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

use tracing::{debug, error, info, instrument, trace, warn};
use tokio::time::{timeout, Duration};

use crate::message::*;
use transport::Connection;

use super::super::guards::require_auth;
use super::super::{PeerActor, PeerActorState, MAX_RESYNC_ATTEMPTS};
use crate::coordinator::CourierMode;

impl<C: Connection> PeerActor<C> {
    const SCRIBE_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

    fn require_auth_did(&self, state: &PeerActorState<C>, msg_name: &str) -> Option<String> {
        match require_auth(&state.state) {
            Ok((did, _)) => Some(did.to_string()),
            Err(_) => {
                warn!("{} from unauthenticated peer: {}", msg_name, self.node_id);
                None
            }
        }
    }

    async fn open_page_scribe(
        &self,
        page_id: &str,
        context: &str,
        state: &PeerActorState<C>,
    ) -> Option<ractor::ActorRef<butler::ScribeMessage>> {
        match state.butler.open_page(page_id).await {
            Ok(scribe) => Some(scribe),
            Err(e) => {
                error!("Failed to open page {} {}: {}", page_id, context, e);
                None
            }
        }
    }

    async fn get_state_vector(
        &self,
        scribe: &ractor::ActorRef<butler::ScribeMessage>,
        layer_name: &str,
    ) -> Option<Vec<u8>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetStateVector {
            layer_name: layer_name.to_string(),
            reply: tx.into(),
        }) {
            error!("Failed to request state vector from Scribe: {}", e);
            return None;
        }

        match timeout(Self::SCRIBE_REQUEST_TIMEOUT, rx).await {
            Ok(Ok(Ok(vector))) => Some(vector),
            Ok(Ok(Err(e))) => {
                error!("Scribe returned error getting state vector: {}", e);
                None
            }
            Ok(Err(_)) => {
                error!("Scribe dropped state vector reply channel");
                None
            }
            Err(_) => {
                error!("Timed out waiting for state vector from Scribe");
                None
            }
        }
    }

    async fn get_snapshot(
        &self,
        scribe: &ractor::ActorRef<butler::ScribeMessage>,
        layer_name: &str,
    ) -> Option<Vec<u8>> {
        let (snapshot_tx, snapshot_rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetSnapshot {
            layer_name: layer_name.to_string(),
            reply: snapshot_tx,
        }) {
            error!("Failed to request snapshot from Scribe: {}", e);
            return None;
        }

        match timeout(Self::SCRIBE_REQUEST_TIMEOUT, snapshot_rx).await {
            Ok(Ok(Some(data))) => Some(data),
            Ok(Ok(None)) => {
                warn!("Layer {} not found in Scribe for SyncReset", layer_name);
                None
            }
            Ok(Err(_)) => {
                error!("Scribe dropped snapshot reply channel");
                None
            }
            Err(_) => {
                error!("Timed out waiting for snapshot from Scribe");
                None
            }
        }
    }

    async fn decrypt_from_peer(
        &self,
        encrypted_data: &[u8],
        context: &str,
        peer_did: &str,
        state: &PeerActorState<C>,
    ) -> Option<Vec<u8>> {
        let session_key = match state.session_key {
            Some(key) => key,
            None => {
                warn!("Cannot decrypt {}: session key not set", context);
                return None;
            }
        };

        match herald::decrypt_symmetric(&session_key, encrypted_data) {
            Ok(data) => Some(data),
            Err(e) => {
                error!("Failed to decrypt {} from {}: {}", context, peer_did, e);
                None
            }
        }
    }

    fn encrypt_for_peer(
        &self,
        data: &[u8],
        context: &str,
        state: &PeerActorState<C>,
    ) -> Option<Vec<u8>> {
        let session_key = match state.session_key {
            Some(key) => key,
            None => {
                warn!("Cannot send {}: session key not set", context);
                return None;
            }
        };

        match herald::encrypt_symmetric(&session_key, data) {
            Ok(result) => Some(result),
            Err(e) => {
                error!("Failed to encrypt {}: {}", context, e);
                None
            }
        }
    }

    fn update_peer_vector(
        &self,
        page_id: &str,
        peer_did: &str,
        layer_name: &str,
        their_state_vector: &[u8],
        state: &PeerActorState<C>,
    ) {
        let peer_device_id = self.node_id.to_string();
        if let Some(subscription) = state.page_subscriptions.get(page_id) {
            let _ = subscription
                .scribe
                .cast(butler::ScribeMessage::UpdatePeerVector {
                    user_did: peer_did.to_string(),
                    device_id: peer_device_id,
                    layer_name: layer_name.to_string(),
                    state_vector: their_state_vector.to_vec(),
                });
        }
    }

    async fn get_updates_since(
        &self,
        scribe: &ractor::ActorRef<butler::ScribeMessage>,
        layer_name: &str,
        their_state_vector: &[u8],
    ) -> Option<Vec<u8>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetUpdatesSince {
            layer_name: layer_name.to_string(),
            state_vector: their_state_vector.to_vec(),
            reply: tx,
        }) {
            error!("Failed to request updates from Scribe: {}", e);
            return None;
        }

        match timeout(Self::SCRIBE_REQUEST_TIMEOUT, rx).await {
            Ok(Ok(Ok(diff))) => Some(diff),
            Ok(Ok(Err(e))) => {
                error!("Scribe returned error getting updates: {}", e);
                None
            }
            Ok(Err(_)) => {
                error!("Scribe dropped updates reply channel");
                None
            }
            Err(_) => {
                error!("Timed out waiting for updates from Scribe");
                None
            }
        }
    }

    async fn send_sync_ack(
        &self,
        page_id: &str,
        layer_name: &str,
        our_current_vector: Vec<u8>,
        state: &PeerActorState<C>,
    ) {
        let msg = Message::SyncAck(SyncAckMsg {
            page_id: page_id.to_string(),
            layer_name: layer_name.to_string(),
            state_vector: our_current_vector,
        });
        self.send_message(&msg, state).await;
    }

    async fn on_sync_accept_fire_and_forget(
        &self,
        page_id: &str,
        layer_name: &str,
        their_state_vector: &[u8],
        our_current_vector: Vec<u8>,
        peer_did: &str,
        state: &mut PeerActorState<C>,
    ) {
        debug!(
            "SyncAccept: no pending offer (fire-and-forget) for page={} layer={} - sending SyncAck",
            page_id, layer_name
        );

        self.send_sync_ack(page_id, layer_name, our_current_vector, state)
            .await;
        self.update_peer_vector(page_id, peer_did, layer_name, their_state_vector, state);
    }

    async fn on_sync_accept_tracked(
        &self,
        key: (String, String),
        page_id: &str,
        layer_name: &str,
        their_state_vector: &[u8],
        our_current_vector: Vec<u8>,
        peer_did: &str,
        offer: super::super::PendingSyncOffer,
        scribe: &ractor::ActorRef<butler::ScribeMessage>,
        state: &mut PeerActorState<C>,
    ) {
        trace!(
            "SyncAccept comparison: their_vector={} bytes, offered_vector={} bytes, current_vector={} bytes",
            their_state_vector.len(),
            offer.our_state_vector.len(),
            our_current_vector.len()
        );

        if their_state_vector == offer.our_state_vector.as_slice() {
            state.pending_sync_offers.remove(&key);
            trace!(
                "SyncAccept: vectors match for page={} layer={} - sending SyncAck",
                page_id,
                layer_name
            );
            self.send_sync_ack(page_id, layer_name, our_current_vector, state)
                .await;
            self.update_peer_vector(page_id, peer_did, layer_name, their_state_vector, state);
            return;
        }

        let new_attempts = offer.resync_attempts + 1;
        if new_attempts > MAX_RESYNC_ATTEMPTS {
            state.pending_sync_offers.remove(&key);
            warn!(
                "SyncAccept: max resyncs ({}) exceeded for page={} layer={} - sending SyncReset",
                MAX_RESYNC_ATTEMPTS, page_id, layer_name
            );

            let msg = Message::SyncReset(SyncResetMsg {
                page_id: page_id.to_string(),
                layer_name: layer_name.to_string(),
            });
            self.send_message(&msg, state).await;
            return;
        }

        info!(
            "SyncAccept: vectors DIVERGED for page={} layer={} - sending resync SyncOffer (attempt {}/{})",
            page_id, layer_name, new_attempts, MAX_RESYNC_ATTEMPTS
        );

        let Some(diff) = self
            .get_updates_since(scribe, layer_name, their_state_vector)
            .await
        else {
            return;
        };

        let Some(encrypted_data) = self.encrypt_for_peer(&diff, "resync SyncOffer", state) else {
            return;
        };

        state.pending_sync_offers.insert(
            key,
            super::super::PendingSyncOffer {
                our_state_vector: our_current_vector.clone(),
                permit: String::new(),
                resync_attempts: new_attempts,
            },
        );

        let layer_name_owned = layer_name.to_string();
        let msg = Message::SyncOffer(SyncOfferMsg {
            page_id: page_id.to_string(),
            layer_type: LayerType::from_layer_name(&layer_name_owned),
            layer_name: layer_name_owned,
            data: encrypted_data,
            state_vector: our_current_vector,
            authority_permit: None,
        });
        self.send_message(&msg, state).await;
    }

    /// Handle incoming SyncOffer from peer (Step 1)
    ///
    /// **Context**: Peer has a layer update with their state vector
    /// **Peer sends**: SyncOffer with page_id, layer_name, encrypted data, state_vector
    /// **We decrypt**: Using session key
    /// **We apply**: Via Scribe.ApplyUpdateWithResult (handles merge)
    /// **We respond**: SyncAccept with our state vector after applying (only if successful)
    #[instrument(skip(self, myself, state, data, their_state_vector, authority_permit), fields(page_id = %page_id, layer_name = %layer_name))]
    pub(in crate::peer_actor) async fn on_sync_offer(
        &self,
        myself: ractor::ActorRef<super::super::PeerMessage>,
        page_id: &str,
        layer_name: &str,
        data: &[u8],
        their_state_vector: &[u8],
        authority_permit: Option<&str>,
        state: &mut PeerActorState<C>,
    ) {
        let Some(peer_did) = self.require_auth_did(state, "SyncOffer") else {
            return;
        };

        let peer_device_id = self.node_id.to_string();

        info!(
            "SyncOffer received: page={} layer={} ({} bytes, vector {} bytes) from {}",
            page_id,
            layer_name,
            data.len(),
            their_state_vector.len(),
            peer_did
        );

        // Open page early so bundled authority fanout and apply share one Scribe handle.
        let Some(scribe) = self.open_page_scribe(page_id, "", state).await else {
            return;
        };

        if let Some(permit) = authority_permit {
            match gurkha::PolicyPermit::from_token(permit) {
                Ok(parsed) => {
                    if parsed.token_type() == Some("layer_authority") {
                        let audience = parsed.parsed().audience().to_string();
                        let version = parsed
                            .get_fact("version")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let full_layer_name = if layer_name.starts_with(&format!("{}/", page_id)) {
                            layer_name.to_string()
                        } else {
                            format!("{}/{}", page_id, layer_name)
                        };
                        if let Err(e) = state.butler.permits().authority().store(
                            page_id,
                            &full_layer_name,
                            &audience,
                            version,
                            permit,
                        ) {
                            warn!(
                                page_id = %page_id,
                                layer = %full_layer_name,
                                audience = %audience,
                                error = %e,
                                "Failed to store bundled layer authority permit"
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!(page_id = %page_id, layer = %layer_name, error = %e, "Invalid bundled authority permit in SyncOffer");
                }
            }
        }

        // Decrypt update using session key
        let Some(decrypted_data) = self.decrypt_from_peer(data, "SyncOffer", &peer_did, state).await
        else {
            return;
        };

        // Auto-subscribe peer if not already subscribed (both Node and User modes)
        // For Node mode: Ensures can_write() can check viewer's write permissions
        // For User mode: Ensures node gets subscribed to viewer's scribe for bidirectional sync
        if !state.page_subscriptions.contains_key(page_id) {
            info!(
                "Auto-subscribing peer {} to page {} from SyncOffer",
                peer_did, page_id
            );
            self.subscribe_to_page(myself.clone(), page_id, "", state)
                .await;
        }

        // Apply the update with result feedback
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::ApplyUpdateWithResult {
            layer_name: layer_name.to_string(),
            update: decrypted_data,
            from_peer: Some((peer_did.clone(), peer_device_id.clone())),
            permit: None,
            reply: reply_tx,
        }) {
            error!("Failed to send ApplyUpdateWithResult to Scribe: {}", e);
            return;
        }

        // Wait for apply result - only send SyncAccept if successful
        match timeout(Self::SCRIBE_REQUEST_TIMEOUT, reply_rx).await {
            Ok(Ok(Ok(()))) => {
                debug!(
                    "ApplyUpdate succeeded for page={} layer={}",
                    page_id, layer_name
                );
            }
            Ok(Ok(Err(e))) => {
                warn!(
                    "ApplyUpdate rejected for page={} layer={}: {}",
                    page_id, layer_name, e
                );
                // DO NOT send SyncAccept - this stops the infinite retry loop
                return;
            }
            Ok(Err(_)) => {
                error!(
                    "ApplyUpdate reply channel closed for page={} layer={}",
                    page_id, layer_name
                );
                return;
            }
            Err(_) => {
                error!(
                    "Timed out waiting for ApplyUpdate result for page={} layer={}",
                    page_id, layer_name
                );
                return;
            }
        }

        let Some(our_state_vector) = self.get_state_vector(&scribe, layer_name).await else {
            return;
        };

        let msg = Message::SyncAccept(SyncAcceptMsg {
            page_id: page_id.to_string(),
            layer_name: layer_name.to_string(),
            state_vector: our_state_vector.clone(),
        });

        self.send_message(&msg, state).await;
        debug!(
            "Sent SyncAccept for page={} layer={} to {} (our vector {} bytes)",
            page_id,
            layer_name,
            peer_did,
            our_state_vector.len()
        );

        // After applying a __sync_meta update:
        // - User mode: check for unsynced entries and send LayerSubscribe
        // - Node mode: no action needed (Scribe's apply.rs handles process_creator_sync_meta)
        if layer_name.starts_with("__sync_meta:") && state.mode == CourierMode::User {
            self.check_sync_meta_and_subscribe(page_id, state).await;
        }
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
        let Some(peer_did) = self.require_auth_did(state, "SyncAccept") else {
            return;
        };

        debug!(
            "SyncAccept: page={} layer={} from {} (their vector {} bytes)",
            page_id,
            layer_name,
            peer_did,
            their_state_vector.len()
        );

        // Get pending sync context (may not exist for fire-and-forget broadcasts)
        let key = (page_id.to_string(), layer_name.to_string());
        let pending = state.pending_sync_offers.get(&key).cloned();

        // Get our current state vector from Scribe
        let Some(scribe) = self.open_page_scribe(page_id, "", state).await else {
            return;
        };

        let Some(our_current_vector) = self.get_state_vector(&scribe, layer_name).await else {
            return;
        };

        match pending {
            None => {
                self.on_sync_accept_fire_and_forget(
                    page_id,
                    layer_name,
                    their_state_vector,
                    our_current_vector,
                    &peer_did,
                    state,
                )
                .await;
            }
            Some(offer) => {
                self.on_sync_accept_tracked(
                    key,
                    page_id,
                    layer_name,
                    their_state_vector,
                    our_current_vector,
                    &peer_did,
                    offer,
                    &scribe,
                    state,
                )
                .await;
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
        let Some(peer_did) = self.require_auth_did(state, "SyncAck") else {
            return;
        };

        debug!(
            "SyncAck: page={} layer={} from {} - sync complete",
            page_id, layer_name, peer_did
        );

        // Update Scribe's in-memory vector cache (persistence via periodic flush)
        self.update_peer_vector(page_id, &peer_did, layer_name, their_state_vector, state);

        // Clean up any pending sync for this page/layer (shouldn't exist but be safe)
        let key = (page_id.to_string(), layer_name.to_string());
        state.pending_sync_offers.remove(&key);

        // Check if this is an assets layer sync - trigger asset fetch for missing blobs
        if layer_name == "assets" || layer_name.ends_with("/assets") {
            self.trigger_asset_sync_after_layer_sync(page_id, layer_name, state)
                .await;
        }
    }

    /// Handle incoming SyncReset from peer (fallback after max resyncs)
    ///
    /// **Context**: Peer exceeded MAX_RESYNC_ATTEMPTS OR explicitly needs layer snapshot.
    /// **Both modes**: Respond with SyncSnapshot when layer exists locally.
    #[instrument(skip(self, state), fields(page_id = %page_id, layer_name = %layer_name))]
    pub(in crate::peer_actor) async fn on_sync_reset(
        &self,
        page_id: &str,
        layer_name: &str,
        state: &mut PeerActorState<C>,
    ) {
        let Some(peer_did) = self.require_auth_did(state, "SyncReset") else {
            return;
        };

        info!(
            "SyncReset: page={} layer={} from {} - sending full snapshot",
            page_id, layer_name, peer_did
        );

        // Get Scribe and export full snapshot
        let Some(scribe) = self.open_page_scribe(page_id, "for SyncReset", state).await else {
            return;
        };

        // Get full snapshot
        let Some(snapshot) = self.get_snapshot(&scribe, layer_name).await else {
            return;
        };

        // Get state vector
        let Some(state_vector) = self.get_state_vector(&scribe, layer_name).await else {
            return;
        };

        // Encrypt snapshot for transfer
        let Some(encrypted_snapshot) = self.encrypt_for_peer(&snapshot, "SyncSnapshot", state)
        else {
            return;
        };

        // Send SyncSnapshot
        let layer_name_owned = layer_name.to_string();
        let msg = Message::SyncSnapshot(SyncSnapshotMsg {
            page_id: page_id.to_string(),
            layer_type: LayerType::from_layer_name(&layer_name_owned),
            layer_name: layer_name_owned,
            snapshot: encrypted_snapshot,
            state_vector,
        });
        self.send_message(&msg, state).await;

        info!(
            "Sent SyncSnapshot for page={} layer={} to {} ({} bytes)",
            page_id,
            layer_name,
            peer_did,
            snapshot.len()
        );
    }

    /// Handle incoming SyncSnapshot from peer (response to SyncReset)
    ///
    /// **Context**: Peer responded to our SyncReset with a full snapshot.
    /// **Both modes**: Replace local layer with received state.
    #[instrument(skip(self, state, snapshot, their_state_vector), fields(page_id = %page_id, layer_name = %layer_name))]
    pub(in crate::peer_actor) async fn on_sync_snapshot(
        &self,
        page_id: &str,
        layer_name: &str,
        snapshot: &[u8],
        their_state_vector: &[u8],
        state: &mut PeerActorState<C>,
    ) {
        let Some(peer_did) = self.require_auth_did(state, "SyncSnapshot") else {
            return;
        };

        info!(
            "SyncSnapshot: page={} layer={} from {} ({} bytes) - replacing local state",
            page_id,
            layer_name,
            peer_did,
            snapshot.len()
        );

        // Decrypt snapshot
        let Some(decrypted_snapshot) =
            self.decrypt_from_peer(snapshot, "SyncSnapshot", &peer_did, state)
                .await
        else {
            return;
        };

        // Get Scribe and replace layer
        let Some(scribe) = self
            .open_page_scribe(page_id, "for SyncSnapshot", state)
            .await
        else {
            return;
        };

        // Replace layer with snapshot
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::ReplaceLayer {
            layer_name: layer_name.to_string(),
            snapshot: decrypted_snapshot,
            from_peer: Some((
                peer_did.clone(),
                self.node_id.to_string(),
                their_state_vector.to_vec(),
            )),
            reply: reply_tx,
        }) {
            error!("Failed to send ReplaceLayer to Scribe: {}", e);
            return;
        }

        match timeout(Self::SCRIBE_REQUEST_TIMEOUT, reply_rx).await {
            Ok(Ok(Ok(()))) => {
                info!(
                    "SyncSnapshot applied: page={} layer={} - sync recovery complete",
                    page_id, layer_name
                );
            }
            Ok(Ok(Err(e))) => {
                error!(
                    "ReplaceLayer failed for page={} layer={}: {}",
                    page_id, layer_name, e
                );
                return;
            }
            Ok(Err(_)) => {
                error!("Scribe dropped ReplaceLayer reply channel");
                return;
            }
            Err(_) => {
                error!(
                    "Timed out waiting for ReplaceLayer result for page={} layer={}",
                    page_id, layer_name
                );
                return;
            }
        }

        // Update Scribe's in-memory vector cache (persistence via periodic flush)
        self.update_peer_vector(page_id, &peer_did, layer_name, their_state_vector, state);

        // Clean up any pending sync
        let key = (page_id.to_string(), layer_name.to_string());
        state.pending_sync_offers.remove(&key);
    }
}
