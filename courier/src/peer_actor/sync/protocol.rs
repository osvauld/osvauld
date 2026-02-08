//! 3-Step Sync Protocol
//!
//! Handles the SyncOffer → SyncAccept → SyncAck protocol for CRDT synchronization.
//! Also includes asset sync triggering after layer sync completes.
//!
//! ## Bounded Resync Protocol
//!
//! When vectors diverge after SyncOffer/SyncAccept, we resync. But to prevent
//! infinite loops, we track resync attempts:
//!
//! 1. `resync_attempts < MAX_RESYNC_ATTEMPTS`: Send another SyncOffer
//! 2. `resync_attempts >= MAX_RESYNC_ATTEMPTS`: Send SyncReset (request full snapshot)
//! 3. Node responds with SyncSnapshot (authoritative full state)
//! 4. User replaces local layer entirely
//!
//! **Invariant**: Node is source of truth for divergence resolution.

use tracing::{debug, error, info, trace, warn, instrument};

use crate::message::Message;
use transport::Connection;

use super::super::guards::require_auth;
use super::super::{PeerActor, PeerActorState, MAX_RESYNC_ATTEMPTS};

impl<C: Connection> PeerActor<C> {

    /// Handle incoming SyncOffer from peer (Step 1)
    ///
    /// **Context**: Peer has a layer update with their state vector
    /// **Peer sends**: SyncOffer with page_id, layer_name, encrypted data, state_vector, permit
    /// **We verify**: Permit is valid
    /// **We decrypt**: ECDH with ephemeral_public + our encryption key
    /// **We apply**: Via Scribe.ApplyUpdateWithResult (handles merge)
    /// **We respond**: SyncAccept with our state vector after applying (only if successful)
    #[instrument(skip(self, myself, state, data, their_state_vector, ephemeral_public, permit), fields(page_id = %page_id, layer_name = %layer_name))]
    pub(in crate::peer_actor) async fn on_sync_offer(
        &self,
        myself: ractor::ActorRef<super::super::PeerMessage>,
        page_id: &str,
        layer_name: &str,
        data: &[u8],
        their_state_vector: &[u8],
        ephemeral_public: &[u8; 32],
        permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("SyncOffer from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        let peer_device_id = self.node_id.to_string();

        debug!(
            "SyncOffer: page={} layer={} ({} bytes, vector {} bytes) from {}",
            page_id, layer_name, data.len(), their_state_vector.len(), peer_did
        );

        // Decrypt transit-encrypted update using ECDH
        let our_secret = match state.butler.encryption_key().await {
            Ok(key) => key,
            Err(e) => {
                error!("Failed to get encryption key for SyncOffer decryption: {}", e);
                return;
            }
        };
        let decrypted_data = match herald::decrypt_from_transfer(&our_secret, ephemeral_public, data) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to decrypt SyncOffer from {}: {}", peer_did, e);
                return;
            }
        };

        // Auto-subscribe peer if not already subscribed (both Node and User modes)
        // For Node mode: Ensures can_write() can check viewer's write permissions
        // For User mode: Ensures node gets subscribed to viewer's scribe for bidirectional sync
        if !state.page_subscriptions.contains_key(page_id) {
            info!("Auto-subscribing peer {} to page {} with permit from SyncOffer", peer_did, page_id);
            self.subscribe_to_page(myself.clone(), page_id, permit, state).await;
        }

        // Get Scribe via Butler and apply update
        let scribe = match state.butler.open_page(page_id).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to open page {}: {}", page_id, e);
                return;
            }
        };

        // Apply the update with result feedback
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::ApplyUpdateWithResult {
            layer_name: layer_name.to_string(),
            update: decrypted_data,
            from_peer: Some((peer_did.clone(), peer_device_id.clone())),
            permit: Some(permit.to_string()),
            reply: reply_tx,
        }) {
            error!("Failed to send ApplyUpdateWithResult to Scribe: {}", e);
            return;
        }

        // Wait for apply result - only send SyncAccept if successful
        match reply_rx.await {
            Ok(Ok(())) => {
                debug!("ApplyUpdate succeeded for page={} layer={}", page_id, layer_name);
            }
            Ok(Err(e)) => {
                warn!("ApplyUpdate rejected for page={} layer={}: {}", page_id, layer_name, e);
                // DO NOT send SyncAccept - this stops the infinite retry loop
                return;
            }
            Err(_) => {
                error!("ApplyUpdate reply channel closed for page={} layer={}", page_id, layer_name);
                return;
            }
        }

        // Get our state vector after applying
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetStateVector {
            layer_name: layer_name.to_string(),
            reply: tx.into(),
        }) {
            error!("Failed to request state vector from Scribe: {}", e);
            return;
        }

        let our_state_vector = match rx.await {
            Ok(Ok(vector)) => vector,
            Ok(Err(e)) => {
                error!("Scribe returned error getting state vector: {}", e);
                return;
            }
            Err(_) => {
                error!("Scribe dropped state vector reply channel");
                return;
            }
        };

        // Send SyncAccept with our state vector
        let msg = Message::SyncAccept {
            page_id: page_id.to_string(),
            layer_name: layer_name.to_string(),
            state_vector: our_state_vector.clone(),
        };

        self.send_message(&msg, state).await;
        debug!(
            "Sent SyncAccept for page={} layer={} to {} (our vector {} bytes)",
            page_id, layer_name, peer_did, our_state_vector.len()
        );
    }

    /// Handle incoming SyncAccept from peer (Step 2)
    ///
    /// **Context**: Peer applied our update and reports their state vector
    /// **Peer sends**: SyncAccept with their state vector after applying
    /// **We compare**: Their vector with our current vector
    /// **If match**: Send SyncAck (sync complete)
    /// **If diverged**: Send new SyncOffer with full diff from their vector (resync)
    #[instrument(skip(self, state, their_state_vector), fields(page_id = %page_id, layer_name = %layer_name))]
    pub(in crate::peer_actor) async fn on_sync_accept(
        &self,
        page_id: &str,
        layer_name: &str,
        their_state_vector: &[u8],
        state: &mut PeerActorState<C>,
    ) {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("SyncAccept from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        debug!(
            "SyncAccept: page={} layer={} from {} (their vector {} bytes)",
            page_id, layer_name, peer_did, their_state_vector.len()
        );

        // Get pending sync context (may not exist for fire-and-forget broadcasts)
        let key = (page_id.to_string(), layer_name.to_string());
        let pending = state.pending_sync_offers.remove(&key);

        // Get our current state vector from Scribe
        let scribe = match state.butler.open_page(page_id).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to open page {}: {}", page_id, e);
                return;
            }
        };

        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetStateVector {
            layer_name: layer_name.to_string(),
            reply: tx.into(),
        }) {
            error!("Failed to request state vector from Scribe: {}", e);
            return;
        }

        let our_current_vector = match rx.await {
            Ok(Ok(vector)) => vector,
            Ok(Err(e)) => {
                error!("Scribe returned error getting state vector: {}", e);
                return;
            }
            Err(_) => {
                error!("Scribe dropped state vector reply channel");
                return;
            }
        };

        // Handle based on whether we have a pending offer
        match pending {
            None => {
                // Fire-and-forget broadcast: No pending offer tracked.
                // Just update peer vector cache and send SyncAck. Trust CRDT to converge.
                debug!(
                    "SyncAccept: no pending offer (fire-and-forget) for page={} layer={} - sending SyncAck",
                    page_id, layer_name
                );

                let msg = Message::SyncAck {
                    page_id: page_id.to_string(),
                    layer_name: layer_name.to_string(),
                    state_vector: our_current_vector,
                };
                self.send_message(&msg, state).await;

                // Update Scribe's in-memory vector cache (persistence via periodic flush)
                let peer_device_id = self.node_id.to_string();
                if let Some(subscription) = state.page_subscriptions.get(page_id) {
                    let _ = subscription.scribe.cast(butler::ScribeMessage::UpdatePeerVector {
                        user_did: peer_did,
                        device_id: peer_device_id,
                        layer_name: layer_name.to_string(),
                        state_vector: their_state_vector.to_vec(),
                    });
                }
            }
            Some(offer) => {
                // Tracked sync (initial sync / reconciliation): Compare vectors
                trace!(
                    "SyncAccept comparison: their_vector={} bytes, offered_vector={} bytes, current_vector={} bytes",
                    their_state_vector.len(),
                    offer.our_state_vector.len(),
                    our_current_vector.len()
                );

                if their_state_vector == offer.our_state_vector.as_slice() {
                    // In sync! Send SyncAck
                    trace!(
                        "SyncAccept: vectors match for page={} layer={} - sending SyncAck",
                        page_id, layer_name
                    );

                    let msg = Message::SyncAck {
                        page_id: page_id.to_string(),
                        layer_name: layer_name.to_string(),
                        state_vector: our_current_vector,
                    };
                    self.send_message(&msg, state).await;

                    // Update Scribe's in-memory vector cache (persistence via periodic flush)
                    let peer_device_id = self.node_id.to_string();
                    if let Some(subscription) = state.page_subscriptions.get(page_id) {
                        let _ = subscription.scribe.cast(butler::ScribeMessage::UpdatePeerVector {
                            user_did: peer_did,
                            device_id: peer_device_id,
                            layer_name: layer_name.to_string(),
                            state_vector: their_state_vector.to_vec(),
                        });
                    }
                } else {
                    // Diverged! Check if we should resync or fall back to SyncReset
                    let new_attempts = offer.resync_attempts + 1;

                    if new_attempts > MAX_RESYNC_ATTEMPTS {
                        // Max resyncs exceeded - fall back to SyncReset (request full snapshot)
                        warn!(
                            "SyncAccept: max resyncs ({}) exceeded for page={} layer={} - sending SyncReset",
                            MAX_RESYNC_ATTEMPTS, page_id, layer_name
                        );

                        let msg = Message::SyncReset {
                            page_id: page_id.to_string(),
                            layer_name: layer_name.to_string(),
                        };
                        self.send_message(&msg, state).await;
                        return;
                    }

                    // Still have retries - send resync SyncOffer with full diff from their vector
                    info!(
                        "SyncAccept: vectors DIVERGED for page={} layer={} - sending resync SyncOffer (attempt {}/{})",
                        page_id, layer_name, new_attempts, MAX_RESYNC_ATTEMPTS
                    );

                    // Get diff from their state vector
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    if let Err(e) = scribe.cast(butler::ScribeMessage::GetUpdatesSince {
                        layer_name: layer_name.to_string(),
                        state_vector: their_state_vector.to_vec(),
                        reply: tx,
                    }) {
                        error!("Failed to request updates from Scribe: {}", e);
                        return;
                    }

                    let diff = match rx.await {
                        Ok(Ok(d)) => d,
                        Ok(Err(e)) => {
                            error!("Scribe returned error getting updates: {}", e);
                            return;
                        }
                        Err(_) => {
                            error!("Scribe dropped updates reply channel");
                            return;
                        }
                    };

                    // Encrypt and send new SyncOffer
                    let peer_encryption_key = match state.peer_encryption_key {
                        Some(key) => key,
                        None => {
                            warn!("Cannot send resync SyncOffer: peer encryption key not set");
                            return;
                        }
                    };

                    let (ephemeral_public, encrypted_data) = match herald::encrypt_for_transfer(
                        &peer_encryption_key,
                        &diff,
                    ) {
                        Ok(result) => result,
                        Err(e) => {
                            error!("Failed to encrypt resync data: {}", e);
                            return;
                        }
                    };

                    // Track this new pending sync with incremented attempt counter
                    state.pending_sync_offers.insert(
                        key,
                        super::super::PendingSyncOffer {
                            our_state_vector: our_current_vector.clone(),
                            permit: offer.permit.clone(),
                            resync_attempts: new_attempts,
                        },
                    );

                    let msg = Message::SyncOffer {
                        page_id: page_id.to_string(),
                        layer_name: layer_name.to_string(),
                        data: encrypted_data,
                        state_vector: our_current_vector,
                        ephemeral_public,
                        permit: offer.permit,
                    };
                    self.send_message(&msg, state).await;
                }
            }
        }
    }

    /// Handle incoming SyncAck from peer (Step 3)
    ///
    /// **Context**: Peer confirms sync complete after comparing vectors
    /// **Peer sends**: SyncAck with their final state vector
    /// **We do**: Update cached peer vector, log sync complete
    #[instrument(skip(self, state, their_state_vector), fields(page_id = %page_id, layer_name = %layer_name))]
    pub(in crate::peer_actor) async fn on_sync_ack(
        &self,
        page_id: &str,
        layer_name: &str,
        their_state_vector: &[u8],
        state: &mut PeerActorState<C>,
    ) {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("SyncAck from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        debug!(
            "SyncAck: page={} layer={} from {} - sync complete",
            page_id, layer_name, peer_did
        );

        // Update Scribe's in-memory vector cache (persistence via periodic flush)
        let peer_device_id = self.node_id.to_string();
        if let Some(subscription) = state.page_subscriptions.get(page_id) {
            let _ = subscription.scribe.cast(butler::ScribeMessage::UpdatePeerVector {
                user_did: peer_did.clone(),
                device_id: peer_device_id.clone(),
                layer_name: layer_name.to_string(),
                state_vector: their_state_vector.to_vec(),
            });
        }

        // Clean up any pending sync for this page/layer (shouldn't exist but be safe)
        let key = (page_id.to_string(), layer_name.to_string());
        state.pending_sync_offers.remove(&key);

        // Check if this is an assets layer sync - trigger asset fetch for missing blobs
        if layer_name.ends_with("/assets") {
            self.trigger_asset_sync_after_layer_sync(page_id, layer_name, state).await;
        }
    }

    /// Handle incoming SyncReset from peer (fallback after max resyncs)
    ///
    /// **Context**: User mode peer exceeded MAX_RESYNC_ATTEMPTS and requests full snapshot
    /// **Node mode only**: Responds with authoritative SyncSnapshot
    /// **User mode**: Should not receive this - log warning
    #[instrument(skip(self, state), fields(page_id = %page_id, layer_name = %layer_name))]
    pub(in crate::peer_actor) async fn on_sync_reset(
        &self,
        page_id: &str,
        layer_name: &str,
        state: &mut PeerActorState<C>,
    ) {
        let peer_did = match require_auth(&state.state) {
            Ok((did, _)) => did.to_string(),
            Err(_) => {
                warn!("SyncReset from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        // Only Node mode should handle SyncReset
        if state.mode != crate::CourierMode::Node {
            warn!("SyncReset received in User mode - ignoring (only Node should respond)");
            return;
        }

        info!(
            "SyncReset: page={} layer={} from {} - sending full snapshot",
            page_id, layer_name, peer_did
        );

        // Get Scribe and export full snapshot
        let scribe = match state.butler.open_page(page_id).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to open page {} for SyncReset: {}", page_id, e);
                return;
            }
        };

        // Get full snapshot
        let (snapshot_tx, snapshot_rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetSnapshot {
            layer_name: layer_name.to_string(),
            reply: snapshot_tx,
        }) {
            error!("Failed to request snapshot from Scribe: {}", e);
            return;
        }

        let snapshot = match snapshot_rx.await {
            Ok(Some(data)) => data,
            Ok(None) => {
                warn!("Layer {} not found in Scribe for SyncReset", layer_name);
                return;
            }
            Err(_) => {
                error!("Scribe dropped snapshot reply channel");
                return;
            }
        };

        // Get state vector
        let (vector_tx, vector_rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetStateVector {
            layer_name: layer_name.to_string(),
            reply: vector_tx.into(),
        }) {
            error!("Failed to request state vector from Scribe: {}", e);
            return;
        }

        let state_vector = match vector_rx.await {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                error!("Scribe returned error getting state vector: {}", e);
                return;
            }
            Err(_) => {
                error!("Scribe dropped state vector reply channel");
                return;
            }
        };

        // Encrypt snapshot for transfer
        let peer_encryption_key = match state.peer_encryption_key {
            Some(key) => key,
            None => {
                warn!("Cannot send SyncSnapshot: peer encryption key not set");
                return;
            }
        };

        let (ephemeral_public, encrypted_snapshot) = match herald::encrypt_for_transfer(
            &peer_encryption_key,
            &snapshot,
        ) {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to encrypt snapshot: {}", e);
                return;
            }
        };

        // Send SyncSnapshot
        let msg = Message::SyncSnapshot {
            page_id: page_id.to_string(),
            layer_name: layer_name.to_string(),
            snapshot: encrypted_snapshot,
            state_vector,
            ephemeral_public,
        };
        self.send_message(&msg, state).await;

        info!(
            "Sent SyncSnapshot for page={} layer={} to {} ({} bytes)",
            page_id, layer_name, peer_did, snapshot.len()
        );
    }

    /// Handle incoming SyncSnapshot from peer (response to SyncReset)
    ///
    /// **Context**: Node responded to our SyncReset with authoritative snapshot
    /// **User mode only**: Replace local layer entirely with node's state
    /// **Node mode**: Should not receive this - log warning
    #[instrument(skip(self, state, snapshot, their_state_vector, ephemeral_public), fields(page_id = %page_id, layer_name = %layer_name))]
    pub(in crate::peer_actor) async fn on_sync_snapshot(
        &self,
        page_id: &str,
        layer_name: &str,
        snapshot: &[u8],
        their_state_vector: &[u8],
        ephemeral_public: &[u8; 32],
        state: &mut PeerActorState<C>,
    ) {
        let peer_did = match require_auth(&state.state) {
            Ok((did, _)) => did.to_string(),
            Err(_) => {
                warn!("SyncSnapshot from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        // Only User mode should receive SyncSnapshot
        if state.mode != crate::CourierMode::User {
            warn!("SyncSnapshot received in Node mode - ignoring (only User should apply)");
            return;
        }

        info!(
            "SyncSnapshot: page={} layer={} from {} ({} bytes) - replacing local state",
            page_id, layer_name, peer_did, snapshot.len()
        );

        // Decrypt snapshot
        let our_secret = match state.butler.encryption_key().await {
            Ok(key) => key,
            Err(e) => {
                error!("Failed to get encryption key for SyncSnapshot decryption: {}", e);
                return;
            }
        };

        let decrypted_snapshot = match herald::decrypt_from_transfer(&our_secret, ephemeral_public, snapshot) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to decrypt SyncSnapshot from {}: {}", peer_did, e);
                return;
            }
        };

        // Get Scribe and replace layer
        let scribe = match state.butler.open_page(page_id).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to open page {} for SyncSnapshot: {}", page_id, e);
                return;
            }
        };

        // Replace layer with snapshot
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::ReplaceLayer {
            layer_name: layer_name.to_string(),
            snapshot: decrypted_snapshot,
            reply: reply_tx,
        }) {
            error!("Failed to send ReplaceLayer to Scribe: {}", e);
            return;
        }

        match reply_rx.await {
            Ok(Ok(())) => {
                info!(
                    "SyncSnapshot applied: page={} layer={} - sync recovery complete",
                    page_id, layer_name
                );
            }
            Ok(Err(e)) => {
                error!("ReplaceLayer failed for page={} layer={}: {}", page_id, layer_name, e);
                return;
            }
            Err(_) => {
                error!("Scribe dropped ReplaceLayer reply channel");
                return;
            }
        }

        // Update Scribe's in-memory vector cache (persistence via periodic flush)
        let peer_device_id = self.node_id.to_string();
        if let Some(subscription) = state.page_subscriptions.get(page_id) {
            let _ = subscription.scribe.cast(butler::ScribeMessage::UpdatePeerVector {
                user_did: peer_did.clone(),
                device_id: peer_device_id,
                layer_name: layer_name.to_string(),
                state_vector: their_state_vector.to_vec(),
            });
        }

        // Clean up any pending sync
        let key = (page_id.to_string(), layer_name.to_string());
        state.pending_sync_offers.remove(&key);
    }
}
