//! Asset Sync Handlers
//!
//! Handles blob transfer via iroh-blobs:
//! - AssetPrepare: Peer requests asset, we prepare blob
//! - AssetReady: Peer has blob ready, we download it
//! - AssetAck: Confirm transfer success/failure with retry
//! - trigger_asset_sync_after_layer_sync: Request missing blobs after metadata sync

use tracing::{debug, info, warn, instrument};

use crate::message::*;
use transport::Connection;

use super::{PeerActor, PeerActorState, MAX_ASSET_TRANSFER_ATTEMPTS};

impl<C: Connection> PeerActor<C> {
    /// Send AssetAck with error
    #[instrument(skip_all, fields(page_id = %page_id, hash = %hash))]
    async fn send_asset_error(&self, page_id: &str, hash: &str, error: &str, state: &PeerActorState<C>) {
        self.send_message(&Message::AssetAck(AssetAckMsg {
            page_id: page_id.to_string(),
            hash: hash.to_string(),
            success: false,
            error: Some(error.to_string()),
        }), state).await;
    }
    /// Handle AssetPrepare request from peer
    ///
    /// **Context**: Peer wants us to prepare an asset for transfer
    /// **We do**: Decrypt from local storage, add to iroh-blobs, send AssetReady
    #[instrument(skip_all, fields(node = %self.node_id, page_id = %page_id, hash = %hash))]
    pub(in crate::peer_actor) async fn on_asset_prepare(
        &self,
        page_id: &str,
        hash: &str,
        state: &mut PeerActorState<C>,
    ) {
        info!(
            page_id = %page_id,
            hash = %hash,
            peer = %self.node_id,
            "AssetPrepare received - preparing blob for transfer"
        );

        // 1. Get page key via butler
        let (_, page_key) = match state.butler.pages().get_decrypted(page_id).await {
            Ok(result) => result,
            Err(e) => {
                warn!(page_id = %page_id, error = ?e, "Failed to get page key for asset");
                self.send_asset_error(page_id, hash, &format!("Page not found: {}", e), state).await;
                return;
            }
        };

        // 2. Get plaintext via asset service
        let plaintext = match butler::services::asset_service::get_for_transfer(
            state.butler.asset_store(),
            &page_key,
            hash,
        ) {
            Ok(data) => data,
            Err(e) => {
                warn!(hash = %hash, error = ?e, "Failed to get asset for transfer");
                self.send_asset_error(page_id, hash, &format!("Asset not found: {}", e), state).await;
                return;
            }
        };

        // 3. Add to blob store
        let iroh_hash_bytes = match state.blob_store.add_blob(&plaintext).await {
            Ok(h) => h,
            Err(e) => {
                warn!(hash = %hash, error = %e, "Failed to add blob");
                self.send_asset_error(page_id, hash, &e, state).await;
                return;
            }
        };

        info!(
            page_id = %page_id,
            hash = %hash,
            iroh_hash = %hex::encode(&iroh_hash_bytes),
            size = plaintext.len(),
            "Asset prepared for transfer"
        );

        // 4. Send AssetReady with iroh_hash
        self.send_message(&Message::AssetReady(AssetReadyMsg {
            page_id: page_id.to_string(),
            hash: hash.to_string(),
            iroh_hash: iroh_hash_bytes,
        }), state).await;
    }

    /// Handle AssetReady notification from peer
    ///
    /// **Context**: Peer has prepared asset, blob is ready for download
    /// **We do**: Download via iroh-blobs, verify, encrypt, store locally
    #[instrument(skip_all, fields(node = %self.node_id, page_id = %page_id, hash = %hash))]
    pub(in crate::peer_actor) async fn on_asset_ready(
        &self,
        page_id: &str,
        hash: &str,
        iroh_hash_bytes: &[u8; 32],
        state: &mut PeerActorState<C>,
    ) {
        info!(
            page_id = %page_id,
            hash = %hash,
            peer = %self.node_id,
            "AssetReady received - downloading blob"
        );

        // 1. Download blob from peer
        let plaintext = match state.blob_store.download_blob(iroh_hash_bytes, self.node_id).await {
            Ok(data) => data,
            Err(e) => {
                warn!(hash = %hash, error = %e, "Failed to download blob");
                self.send_asset_error(page_id, hash, &e, state).await;
                return;
            }
        };

        // 2. Get page key for storage
        let (_, page_key) = match state.butler.pages().get_decrypted(page_id).await {
            Ok(result) => result,
            Err(e) => {
                warn!(page_id = %page_id, error = ?e, "Failed to get page key for storing asset");
                self.send_asset_error(page_id, hash, &format!("Page not found: {}", e), state).await;
                return;
            }
        };

        // 3. Get metadata from Loro-synced assets layer (for signature verification)
        let assets_layer_name = "assets".to_string();
        let metadata = match self.get_asset_metadata_from_layer(page_id, &assets_layer_name, hash, state).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                warn!(hash = %hash, "Asset metadata not found in layer - cannot verify");
                self.send_asset_error(page_id, hash, "Metadata not synced yet", state).await;
                return;
            }
            Err(e) => {
                warn!(hash = %hash, error = %e, "Failed to get asset metadata from layer");
                self.send_asset_error(page_id, hash, &format!("Metadata lookup failed: {}", e), state).await;
                return;
            }
        };

        // 4. Verify and store the received asset
        if let Err(e) = butler::services::asset_service::store_received(
            state.butler.asset_store(),
            &page_key,
            &metadata,
            &plaintext,
        ) {
            warn!(hash = %hash, error = ?e, "Failed to store received asset");
            self.send_asset_error(page_id, hash, &format!("Storage failed: {}", e), state).await;
            return;
        }

        info!(
            page_id = %page_id,
            hash = %hash,
            size = plaintext.len(),
            "Asset received and stored successfully"
        );

        // 5. Send AssetAck
        self.send_message(&Message::AssetAck(AssetAckMsg {
            page_id: page_id.to_string(),
            hash: hash.to_string(),
            success: true,
            error: None,
        }), state).await;
    }

    /// Handle AssetAck from peer
    ///
    /// **Context**: Peer received and processed our AssetReady message
    /// **Success**: Remove from pending transfers, log completion
    /// **Failure**: Retry with exponential backoff (max 3 attempts)
    #[instrument(skip_all, fields(node = %self.node_id, page_id = %page_id, hash = %hash, success = success))]
    pub(in crate::peer_actor) async fn on_asset_ack(
        &self,
        page_id: &str,
        hash: &str,
        success: bool,
        error: Option<&str>,
        state: &mut PeerActorState<C>,
    ) {
        if success {
            // Success - remove from pending transfers
            state.pending_asset_transfers.remove(hash);
            info!(
                page_id = %page_id,
                hash = %hash,
                peer = %self.node_id,
                "Asset transfer acknowledged"
            );
            // MemStore auto-cleans via reference counting, no manual cleanup needed
        } else {
            // Failure - check retry logic
            let should_retry = if let Some(pending) = state.pending_asset_transfers.get_mut(hash) {
                pending.attempts += 1;
                if pending.attempts < MAX_ASSET_TRANSFER_ATTEMPTS {
                    let backoff_ms = 100 * (1 << pending.attempts); // Exponential: 200ms, 400ms, 800ms
                    info!(
                        page_id = %page_id,
                        hash = %hash,
                        attempt = pending.attempts,
                        backoff_ms = backoff_ms,
                        error = ?error,
                        "Asset transfer failed, scheduling retry"
                    );
                    // Return true to retry
                    true
                } else {
                    warn!(
                        page_id = %page_id,
                        hash = %hash,
                        attempts = pending.attempts,
                        error = ?error,
                        "Asset transfer failed after max retries, giving up (will sync on next session)"
                    );
                    // Give up - remove from pending
                    false
                }
            } else {
                // Not tracked - this is a response to an externally triggered AssetPrepare
                // or the transfer completed before we tracked it
                warn!(
                    page_id = %page_id,
                    hash = %hash,
                    error = ?error,
                    "Asset transfer failed (not tracked for retry)"
                );
                false
            };

            if should_retry {
                // Re-send AssetPrepare for retry
                self.send_message(&Message::AssetPrepare(AssetPrepareMsg {
                    page_id: page_id.to_string(),
                    hash: hash.to_string(),
                }), state).await;
            } else {
                // Clean up
                state.pending_asset_transfers.remove(hash);
            }
        }
    }

    /// Get asset metadata from the Loro-synced assets layer
    ///
    /// **Context**: When receiving an asset via iroh-blobs, we need the full metadata
    /// (including signature) from the Loro layer for verification
    ///
    /// **Flow**:
    ///   1. Get Scribe for the page
    ///   2. Get layer snapshot
    ///   3. Parse and find the specific asset metadata by hash
    #[instrument(skip_all, fields(page_id = %page_id, layer = %layer_name, hash = %hash))]
    async fn get_asset_metadata_from_layer(
        &self,
        page_id: &str,
        layer_name: &str,
        hash: &str,
        state: &mut PeerActorState<C>,
    ) -> Result<Option<butler::AssetMetadata>, String> {
        // Get Scribe
        let scribe = match state.page_subscriptions.get(page_id) {
            Some(sub) => sub.scribe.clone(),
            None => {
                match state.butler.open_page(page_id).await {
                    Ok(s) => s,
                    Err(e) => return Err(format!("Failed to open page: {}", e)),
                }
            }
        };

        // Get layer snapshot
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetSnapshot {
            layer_name: layer_name.to_string(),
            reply: tx,
        }) {
            return Err(format!("Failed to request layer snapshot: {}", e));
        }

        let layer_bytes = match rx.await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ok(None), // Layer doesn't exist
            Err(_) => return Err("Scribe dropped reply channel".to_string()),
        };

        // Parse layer
        let layer = butler::models::Layer::from_snapshot(&layer_bytes)
            .map_err(|e| format!("Failed to parse layer: {}", e))?;

        // Get all assets from layer and find the one we need
        let assets = butler::services::asset_service::get_assets_from_layer(&layer);
        Ok(assets.get(hash).cloned())
    }

    /// Trigger asset sync after receiving an assets layer update via Loro sync
    ///
    /// **Context**: We received a SyncAck for an assets layer, meaning new metadata was synced
    /// **Flow**:
    ///   1. Get the assets layer from Scribe
    ///   2. Parse asset metadata
    ///   3. Find assets missing from local AssetStore
    ///   4. Send AssetPrepare for each missing asset
    ///
    /// **Note**: Called from sync/protocol.rs on_sync_ack
    #[instrument(skip(self, state), fields(page_id = %page_id, layer = %layer_name))]
    pub(in crate::peer_actor) async fn trigger_asset_sync_after_layer_sync(
        &self,
        page_id: &str,
        layer_name: &str,
        state: &mut PeerActorState<C>,
    ) {
        // Get the assets layer from Scribe
        let scribe = match state.page_subscriptions.get(page_id) {
            Some(sub) => sub.scribe.clone(),
            None => {
                // Try to open the page if not subscribed
                match state.butler.open_page(page_id).await {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("Cannot trigger asset sync - page {} not open: {}", page_id, e);
                        return;
                    }
                }
            }
        };

        // Get the layer snapshot
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetSnapshot {
            layer_name: layer_name.to_string(),
            reply: tx,
        }) {
            warn!("Failed to request assets layer snapshot: {}", e);
            return;
        }

        let layer_bytes = match rx.await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                debug!("Assets layer {} not found in Scribe", layer_name);
                return;
            }
            Err(_) => {
                warn!("Scribe dropped assets layer reply channel");
                return;
            }
        };

        // Parse the layer
        let layer = match butler::models::Layer::from_snapshot(&layer_bytes) {
            Ok(l) => l,
            Err(e) => {
                warn!("Failed to parse assets layer {}: {}", layer_name, e);
                return;
            }
        };

        // Find missing assets
        let missing = butler::services::asset_service::find_missing_assets_from_layer(
            state.butler.asset_store(),
            &layer,
        );

        if missing.is_empty() {
            debug!("No missing assets after sync of layer {}", layer_name);
            return;
        }

        info!(
            page_id = %page_id,
            layer = %layer_name,
            missing_count = missing.len(),
            "Found missing assets after layer sync, sending AssetPrepare"
        );

        // Send AssetPrepare for each missing asset (tracked for retry)
        for metadata in missing {
            self.send_asset_prepare(page_id, &metadata.hash, state).await;
        }
    }
}
