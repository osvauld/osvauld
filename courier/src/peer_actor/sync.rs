//! Unified sync handlers - permit determines role (owner/node/viewer)
//!
//! Key insight: Sync mechanics are identical regardless of who is syncing.
//! The ONLY difference is the permit - gurkha handles authorization.
//!
//! Flows:
//! 1. Viewer requests space: SpaceRequest → SpaceData → SpaceDataAck → PageData (x N)
//! 2. 3-Step Sync Protocol: SyncOffer → SyncAccept → SyncAck (or resync SyncOffer if diverged)
//!
//! The 3-step protocol is used for ALL sync operations:
//! - Initial sync on subscribe
//! - Broadcast updates
//! - Periodic reconciliation
//!
//! Divergence detection: If state vectors don't match after applying update,
//! sender immediately sends full diff as new SyncOffer.
//!
//! ## Authorization
//!
//! **Identity-based**: `gurkha::can_access_layer()` checks if viewer can access layer
//! **Role-based**: App code checks `permit:role()` for state-dependent authorization
//!
//! Authorization logic lives in app code (signed by owner, trusted).

use serde_json::{json, Value};
use tracing::{debug, error, info, warn, instrument};

use crate::coordinator::CoordinatorMessage;
use crate::message::Message;
use crate::CourierMode;

use super::guards::{
    require_user_mode, require_node_mode, require_auth, parse_permit,
    to_published_space, from_published_space, from_published_page_meta,
};
use super::{PeerActor, PeerActorState, PendingViewerSync};

// ==================== Default Consent Templates ====================
// Used for automatic consent issuance when viewer receives space/pages.
// In production, these could be configurable.

/// Default space consent template - viewer consents to receive space sync + new pages
const DEFAULT_SPACE_CONSENT_TEMPLATE: &str = r#"{
  "consent_template": {
    "token_type": "sync_space_consent",
    "operations": { "receive_pages": "allow", "receive_updates": "allow" },
    "auth_capabilities": { "accept_sync": true, "accept_new_pages": true },
    "relationship": "sync_consent"
  }
}"#;

/// Default page consent template - viewer consents to receive page layer updates
const DEFAULT_PAGE_CONSENT_TEMPLATE: &str = r#"{
  "consent_template": {
    "token_type": "sync_page_consent",
    "operations": { "receive_layer_updates": "allow" },
    "auth_capabilities": { "accept_sync": true },
    "relationship": "sync_consent"
  }
}"#;

impl PeerActor {
    // ==================== Viewer Space Request ====================

    /// Initiate request for space content as viewer (User mode)
    ///
    /// **Context**: Viewer has aud:* permit from shareable link
    /// **We send**: SpaceRequest to node with our identity and the permit
    /// **Next**: Wait for SpaceData with delegated permit and content
    #[instrument(skip(self, state, viewer_permit), fields(space_id = %space_id))]
    pub(super) async fn initiate_request_space_as_viewer(
        &self,
        space_id: &str,
        viewer_permit: &str,
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "initiate_request_space_as_viewer").is_some() {
            return;
        }

        // Get our identity
        let identity = match state.butler.get_identity().await {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to get identity for SpaceRequest: {}", e);
                return;
            }
        };

        let request_id = uuid::Uuid::new_v4().to_string();

        info!("Requesting space {} as viewer from node {}", space_id, self.node_id);

        let viewer_public_key: [u8; 32] = identity.public_signing_key().try_into()
            .expect("public_signing_key should be 32 bytes");
        let viewer_encryption_key: [u8; 32] = identity.public_encryption_key().try_into()
            .expect("public_encryption_key should be 32 bytes");

        let msg = Message::SpaceRequest {
            request_id,
            space_id: space_id.to_string(),
            viewer_did: identity.did().to_string(),
            viewer_public_key,
            viewer_encryption_key,
            viewer_permit: viewer_permit.to_string(),
        };

        self.send_message(&msg, state).await;
    }

    /// Handle SpaceRequest from viewer (Node mode)
    ///
    /// **Context**: Viewer connects with aud:* permit from shareable link
    /// **Viewer sends**: SpaceRequest with aud:* permit
    /// **We verify**: Permit is valid aud:* viewer_auth token
    /// **We delegate**: Real viewer permit from space permit
    /// **We store**: Viewer permit in VIEWER_PERMITS table
    /// **We send**: SpaceData with delegated permit and content
    #[instrument(skip(self, state, viewer_public_key, viewer_encryption_key, viewer_permit), fields(request_id = %request_id, space_id = %space_id, viewer_did = %viewer_did))]
    pub(super) async fn on_space_request(
        &self,
        request_id: &str,
        space_id: &str,
        viewer_did: &str,
        viewer_public_key: &[u8],
        viewer_encryption_key: &[u8; 32],
        viewer_permit: &str,
        state: &mut PeerActorState,
    ) {
        if require_node_mode(state.mode, "on_space_request").is_some() {
            return;
        }

        info!("SpaceRequest: space={} viewer={}", space_id, viewer_did);

        // Validate viewer_permit
        let permit = match parse_permit(viewer_permit) {
            Ok(p) => p,
            Err(e) => {
                self.send_space_request_error(request_id, &e, state).await;
                return;
            }
        };

        // Verify it's an aud:* token (wildcard audience)
        if permit.parsed().audience() != "*" {
            self.send_space_request_error(request_id, "Viewer permit must have wildcard audience (aud:*)", state).await;
            return;
        }

        // Verify token_type is viewer_auth
        let token_type = permit.get_fact("token_type")
            .and_then(|v| v.as_str());
        if token_type != Some("viewer_auth") {
            self.send_space_request_error(request_id, "Viewer permit must be viewer_auth type", state).await;
            return;
        }

        // Verify space_id in permit matches requested space_id
        let permit_space_id = permit.get_fact("space_id")
            .and_then(|v| v.as_str());
        if permit_space_id != Some(space_id) {
            self.send_space_request_error(request_id, "Viewer permit space_id does not match requested space", state).await;
            return;
        }

        // Verify space exists on this node
        let space = match state.butler.get_space(space_id) {
            Ok(Some(s)) => s,
            Ok(None) => {
                self.send_space_request_error(request_id, &format!("Space not found: {}", space_id), state).await;
                return;
            }
            Err(e) => {
                self.send_space_request_error(request_id, &format!("Failed to get space: {}", e), state).await;
                return;
            }
        };

        // Convert viewer_public_key to base64 for permit audience
        let viewer_pubkey_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            viewer_public_key,
        );

        // Delegate real viewer permit from space permit
        let delegated_space_permit = match state.butler.delegate_space_to_viewer(space_id, &viewer_pubkey_b64).await {
            Ok(p) => p,
            Err(e) => {
                self.send_space_request_error(request_id, &format!("Failed to delegate permit: {}", e), state).await;
                return;
            }
        };

        info!("Delegated space permit to viewer {} (permit len: {})", viewer_did, delegated_space_permit.len());

        // Get page IDs in the space
        let pages = match state.butler.list_pages(space_id) {
            Ok(p) => p,
            Err(e) => {
                self.send_space_request_error(request_id, &format!("Failed to list pages: {}", e), state).await;
                return;
            }
        };

        let page_ids: Vec<String> = pages.iter().map(|p| p.id.clone()).collect();
        info!("Space {} has {} pages for viewer {}", space_id, page_ids.len(), viewer_did);

        // Store viewer context for streaming pages after ack
        state.pending_viewer_syncs.insert(
            request_id.to_string(),
            PendingViewerSync {
                viewer_did: viewer_did.to_string(),
                viewer_pubkey_b64: viewer_pubkey_b64.clone(),
                viewer_encryption_key: *viewer_encryption_key,
                page_ids: page_ids.clone(),
            },
        );

        // Send SpaceData
        let msg = Message::SpaceData {
            request_id: request_id.to_string(),
            space_id: space_id.to_string(),
            delegated_permit: delegated_space_permit,
            space: to_published_space(&space),
            page_ids,
        };

        self.send_message(&msg, state).await;
        info!("Sent SpaceData for space {} to viewer {} (waiting for ack)", space_id, viewer_did);
    }

    /// Handle SpaceDataAck from viewer (Node mode)
    ///
    /// **Context**: Viewer acknowledged SpaceData, now stream pages
    /// **Viewer sent**: SpaceDataAck with delegated permit
    /// **We verify**: Permit matches what we delegated
    /// **We stream**: PageData messages one by one
    #[instrument(skip(self, state, _delegated_permit), fields(request_id = %request_id, space_id = %space_id))]
    pub(super) async fn on_space_data_ack(
        &self,
        request_id: &str,
        space_id: &str,
        _delegated_permit: &str,
        state: &mut PeerActorState,
    ) {
        if require_node_mode(state.mode, "on_space_data_ack").is_some() {
            return;
        }

        info!("SpaceDataAck: space={} request={}", space_id, request_id);

        // Get and remove pending viewer sync context
        let pending = match state.pending_viewer_syncs.remove(request_id) {
            Some(p) => p,
            None => {
                error!("No pending viewer sync for request {}", request_id);
                return;
            }
        };

        // Stream pages one by one
        let total_pages = pending.page_ids.len();
        for (idx, page_id) in pending.page_ids.iter().enumerate() {
            let is_last = idx == total_pages - 1;

            // Prepare page for viewer
            let prepared = match state.butler.prepare_page_for_viewer(
                page_id,
                &pending.viewer_pubkey_b64,
                &pending.viewer_encryption_key,
            ).await {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to prepare page {} for viewer: {}", page_id, e);
                    continue;
                }
            };

            // Store permit CID for revocation tracking
            // Note: Using permit string as CID for now - proper CID computation can be added later
            if let Err(e) = state.butler.put_permit_cid(page_id, &pending.viewer_did, &prepared.permit) {
                warn!("Failed to store permit CID for page {}: {}", page_id, e);
            }

            // Store viewer's page permit for sync authorization
            // This allows the node to authorize incoming SyncOffers from the viewer
            if let Err(e) = state.butler.store_user_page_permit(page_id, &pending.viewer_did, &prepared.permit) {
                warn!("Failed to store viewer's permit for sync auth: {} - sync may fail", e);
            } else {
                debug!("Stored viewer's permit for page {} (sync authorization)", page_id);
            }

            // Send PageData message
            let msg = Message::PageData {
                request_id: request_id.to_string(),
                space_id: space_id.to_string(),
                meta: crate::message::PublishedPageMeta {
                    id: prepared.meta.id,
                    space_id: prepared.meta.space_id,
                    name: prepared.meta.name,
                    owner_did: prepared.meta.owner_did,
                    is_private: prepared.meta.is_private,
                    created_at: prepared.meta.created_at,
                    updated_at: prepared.meta.updated_at,
                },
                permit: prepared.permit,
                ephemeral_public: prepared.ephemeral_public,
                layers: prepared.layers,
                is_last,
            };

            self.send_message(&msg, state).await;
            debug!("Sent PageData {}/{} ({}) to viewer {}", idx + 1, total_pages, page_id, pending.viewer_did);

            // Store viewer's state vectors so we can send incremental updates later
            // The viewer now has what we just sent, so store the current layer state as their vector
            let viewer_device_id = self.node_id.to_string();
            if let Err(e) = state.butler.store_peer_vectors_from_page(
                page_id,
                &pending.viewer_did,
                &viewer_device_id,
            ).await {
                warn!("Failed to store viewer's state vectors for page {}: {}", page_id, e);
            }
        }

        info!("Streamed {} pages to viewer {} for space {}", total_pages, pending.viewer_did, space_id);
    }

    /// Handle SpaceData from node (Viewer mode)
    ///
    /// **Context**: Node responded to our SpaceRequest with space metadata
    /// **Node sent**: SpaceData with delegated permit and page_ids (no layers)
    /// **We store**: Space with delegated permit
    /// **We send**: SpaceDataAck to trigger page streaming
    #[instrument(skip(self, state, space, delegated_permit), fields(request_id = %request_id, space_id = %space_id))]
    pub(super) async fn on_space_data(
        &self,
        request_id: &str,
        space_id: &str,
        delegated_permit: &str,
        space: &crate::message::PublishedSpace,
        page_ids: &[String],
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "on_space_data").is_some() {
            return;
        }

        info!("SpaceData: space={} pages={}", space_id, page_ids.len());

        // Get node's DID for sync target (EnsureSync uses DID, not transport node_id)
        let source_did = match require_auth(&state.state) {
            Ok((did, _)) => did.to_string(),
            Err(_) => {
                warn!("Cannot store space: peer not authenticated");
                return;
            }
        };

        // Store the space with the delegated permit AND source DID
        // The source_did enables viewers to sync edits back to this node via EnsureSync
        let butler_space = from_published_space(space);
        if let Err(e) = state.butler.store_published_space_with_source(
            &butler_space,
            delegated_permit,
            Some(&source_did),
        ) {
            error!("Failed to store space: {}", e);
            return;
        }

        info!("Stored space {} with delegated permit and source_did={}, expecting {} pages", space_id, source_did, page_ids.len());

        // Emit ViewerSpaceReceived to app
        let _ = state.coordinator.cast(CoordinatorMessage::ViewerSpaceReceived {
            node_id: self.node_id,
            space: butler_space.clone(),
            page_count: page_ids.len(),
        });

        // Issue space consent permit immediately after receiving space
        self.issue_space_consent(space_id, delegated_permit, DEFAULT_SPACE_CONSENT_TEMPLATE, state).await;

        // Send SpaceDataAck to trigger page streaming
        let msg = Message::SpaceDataAck {
            request_id: request_id.to_string(),
            space_id: space_id.to_string(),
            delegated_permit: delegated_permit.to_string(),
        };

        self.send_message(&msg, state).await;
        info!("Sent SpaceDataAck for space {}, waiting for {} pages", space_id, page_ids.len());
    }

    /// Handle PageData from node (Viewer mode)
    ///
    /// **Context**: Node is streaming pages after we acknowledged SpaceData
    /// **Node sent**: PageData with page metadata, permit, and encrypted layers
    /// **We store**: Page with source_node_did for future sync (viewer→node reconnection)
    /// **If is_last**: Notify coordinator of successful viewer sync
    #[instrument(skip(self, state, meta, permit, ephemeral_public, layers), fields(request_id = %request_id, space_id = %space_id, page_id = %meta.id))]
    pub(super) async fn on_viewer_page(
        &self,
        request_id: &str,
        space_id: &str,
        meta: &crate::message::PublishedPageMeta,
        permit: &str,
        ephemeral_public: &[u8; 32],
        layers: &[(String, Vec<u8>)],
        is_last: bool,
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "on_viewer_page").is_some() {
            return;
        }

        // Get node DID from authenticated state (viewer authenticated with node during handshake)
        let node_did = match require_auth(&state.state) {
            Ok((did, _)) => Some(did.to_string()),
            Err(_) => {
                warn!("PageData received without authentication - cannot track source node");
                None
            }
        };

        info!("PageData: page={} space={} is_last={} from_node={:?}", meta.id, space_id, is_last, node_did);

        // Convert and store with source node DID for viewer→node reconnection
        // Also store node's state vectors for incremental sync later
        let page_meta = from_published_page_meta(meta);
        let butler_page: butler::Page = page_meta.clone().into();
        let transit_layers: Vec<(String, Vec<u8>)> = layers.to_vec();
        let sender_device_id = self.node_id.to_string();

        // sender_did is the node's DID, sender_device_id is node's Iroh NodeId
        let (sender_did, sender_device_id) = match &node_did {
            Some(did) => (did.as_str(), sender_device_id.as_str()),
            None => {
                error!("Cannot store page without node DID - skipping state vector storage");
                ("unknown", "unknown")
            }
        };

        match state.butler.store_published_page(
            page_meta,
            permit,
            ephemeral_public,
            transit_layers,
            node_did.as_deref(),
            sender_did,
            sender_device_id,
        ).await {
            Ok(_) => {
                info!("Stored page {} from space {} (source_node={:?})", meta.id, space_id, node_did);
            }
            Err(e) => {
                error!("Failed to store page {}: {}", meta.id, e);
            }
        }

        // Emit PageReceived to app
        let _ = state.coordinator.cast(CoordinatorMessage::PageReceived {
            node_id: self.node_id,
            page: butler_page,
            is_last,
        });

        // Extract consent_template from permit_template.json layer (app-defined)
        // Falls back to default if not found
        let consent_template = layers.iter()
            .find(|(name, _)| name == "file:permit_template.json")
            .and_then(|(_, data)| serde_json::from_slice::<Value>(data).ok())
            .and_then(|template_json| template_json.get("consent_template").cloned())
            .map(|consent| serde_json::to_string(&json!({"consent_template": consent})).unwrap_or_else(|_| DEFAULT_PAGE_CONSENT_TEMPLATE.to_string()))
            .unwrap_or_else(|| DEFAULT_PAGE_CONSENT_TEMPLATE.to_string());

        debug!(
            consent_has_layers = consent_template.contains("layers"),
            "Using consent template from app"
        );

        // Issue page consent permit for this page
        self.issue_page_consent(&meta.id, permit, &consent_template, state).await;

        // If this is the last page, notify coordinator
        if is_last {
            info!("PageData stream complete for space {}", space_id);
            let _ = state.coordinator.cast(CoordinatorMessage::ViewerSyncComplete {
                node_id: self.node_id,
                space_id: space_id.to_string(),
                pages_synced: 1, // TODO: track total pages received
            });
        }
    }

    // ==================== 3-Step Sync Protocol ====================

    /// Handle incoming SyncOffer from peer (Step 1)
    ///
    /// **Context**: Peer has a layer update with their state vector
    /// **Peer sends**: SyncOffer with page_id, layer_name, encrypted data, state_vector, permit
    /// **We verify**: Permit is valid
    /// **We decrypt**: ECDH with ephemeral_public + our encryption key
    /// **We apply**: Via Scribe.ApplyUpdateWithResult (handles merge)
    /// **We respond**: SyncAccept with our state vector after applying (only if successful)
    #[instrument(skip(self, myself, state, data, their_state_vector, ephemeral_public, permit), fields(page_id = %page_id, layer_name = %layer_name))]
    pub(super) async fn on_sync_offer(
        &self,
        myself: ractor::ActorRef<super::PeerMessage>,
        page_id: &str,
        layer_name: &str,
        data: &[u8],
        their_state_vector: &[u8],
        ephemeral_public: &[u8; 32],
        permit: &str,
        state: &mut PeerActorState,
    ) {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("SyncOffer from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        let peer_device_id = self.node_id.to_string();

        info!(
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
    pub(super) async fn on_sync_accept(
        &self,
        page_id: &str,
        layer_name: &str,
        their_state_vector: &[u8],
        state: &mut PeerActorState,
    ) {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("SyncAccept from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        info!(
            "SyncAccept: page={} layer={} from {} (their vector {} bytes)",
            page_id, layer_name, peer_did, their_state_vector.len()
        );

        // Get pending sync context
        let key = (page_id.to_string(), layer_name.to_string());
        let pending = match state.pending_sync_offers.remove(&key) {
            Some(p) => p,
            None => {
                warn!(
                    "SyncAccept received but no pending SyncOffer for page={} layer={}",
                    page_id, layer_name
                );
                return;
            }
        };

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

        // Compare state vectors
        debug!(
            "SyncAccept comparison: their_vector={} bytes {:02x?}, our_vector={} bytes {:02x?}",
            their_state_vector.len(),
            &their_state_vector[..their_state_vector.len().min(32)],
            our_current_vector.len(),
            &our_current_vector[..our_current_vector.len().min(32)]
        );
        if their_state_vector == our_current_vector.as_slice() {
            // In sync! Send SyncAck
            info!(
                "SyncAccept: vectors match for page={} layer={} - sending SyncAck",
                page_id, layer_name
            );

            let msg = Message::SyncAck {
                page_id: page_id.to_string(),
                layer_name: layer_name.to_string(),
                state_vector: our_current_vector.clone(),
            };
            self.send_message(&msg, state).await;

            // Update cached peer vector (persistent storage)
            let peer_device_id = self.node_id.to_string();
            if let Err(e) = state.butler.store_peer_state_vector(
                &peer_did,
                &peer_device_id,
                page_id,
                layer_name,
                their_state_vector,
            ).await {
                warn!("Failed to update cached peer vector: {}", e);
            }

            // Also update Scribe's in-memory vector cache (for faster sync decisions)
            if let Some(subscription) = state.page_subscriptions.get(page_id) {
                let _ = subscription.scribe.cast(butler::ScribeMessage::UpdatePeerVector {
                    user_did: peer_did.clone(),
                    device_id: peer_device_id.clone(),
                    layer_name: layer_name.to_string(),
                    state_vector: their_state_vector.to_vec(),
                });
            }
        } else {
            // Diverged! Send resync SyncOffer with full diff from their vector
            info!(
                "SyncAccept: vectors DIVERGED for page={} layer={} - sending resync SyncOffer",
                page_id, layer_name
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

            // Track this new pending sync
            state.pending_sync_offers.insert(
                key.clone(),
                super::PendingSyncOffer {
                    our_state_vector: our_current_vector.clone(),
                    permit: pending.permit.clone(),
                },
            );

            let msg = Message::SyncOffer {
                page_id: page_id.to_string(),
                layer_name: layer_name.to_string(),
                data: encrypted_data,
                state_vector: our_current_vector,
                ephemeral_public,
                permit: pending.permit,
            };
            self.send_message(&msg, state).await;
        }
    }

    /// Handle incoming SyncAck from peer (Step 3)
    ///
    /// **Context**: Peer confirms sync complete after comparing vectors
    /// **Peer sends**: SyncAck with their final state vector
    /// **We do**: Update cached peer vector, log sync complete
    #[instrument(skip(self, state, their_state_vector), fields(page_id = %page_id, layer_name = %layer_name))]
    pub(super) async fn on_sync_ack(
        &self,
        page_id: &str,
        layer_name: &str,
        their_state_vector: &[u8],
        state: &mut PeerActorState,
    ) {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("SyncAck from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        info!(
            "SyncAck: page={} layer={} from {} - sync complete",
            page_id, layer_name, peer_did
        );

        // Update cached peer vector (persistent storage)
        let peer_device_id = self.node_id.to_string();
        if let Err(e) = state.butler.store_peer_state_vector(
            &peer_did,
            &peer_device_id,
            page_id,
            layer_name,
            their_state_vector,
        ).await {
            warn!("Failed to update cached peer vector: {}", e);
        }

        // Also update Scribe's in-memory vector cache (for faster sync decisions)
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

    // ==================== Page Subscription ====================

    /// Subscribe to a page's Scribe for live sync broadcasts
    ///
    /// **Context**: After handshake, we subscribe to pages we want updates from
    /// **We do**: Open page, subscribe to Scribe, spawn listener task
    /// **Listener**: Forwards broadcasts as PeerMessage::BroadcastReceived
    #[instrument(skip(self, myself, state, permit), fields(page_id = %page_id))]
    pub(super) async fn subscribe_to_page(
        &self,
        myself: ractor::ActorRef<super::PeerMessage>,
        page_id: &str,
        permit: &str,
        state: &mut PeerActorState,
    ) {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("Cannot subscribe to page before authentication");
                return;
            }
        };

        // Check if already subscribed
        if state.page_subscriptions.contains_key(page_id) {
            debug!("Already subscribed to page {}", page_id);
            return;
        }

        // Open the page (activates Scribe)
        let scribe = match state.butler.open_page(page_id).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to open page {} for subscription: {}", page_id, e);
                return;
            }
        };

        // Determine subscription permit based on mode.
        // The subscription controls what layers the peer can RECEIVE from broadcasts.
        //
        // **Node mode**: Look up the peer's stored page permit. The peer's permit (stored
        // when they first received the page) defines what they can access. Consent permits
        // from SyncOffer don't have layer definitions, so stored permit is preferred.
        //
        // **User mode**: Use our local permit. We sync to a single target (the node) which
        // has owner-level permissions. What matters is what WE can SEND (sync=true layers).
        // The local permit defines our sync capabilities.
        let subscription_permit = match state.mode {
            CourierMode::Node => {
                // Node mode: Look up peer's stored page permit
                match state.butler.get_user_page_permit(page_id, &peer_did) {
                    Ok(Some(stored_permit)) => {
                        debug!("Node mode: Using stored page permit for peer {} (permit_len={})", peer_did, stored_permit.len());
                        stored_permit
                    }
                    _ => {
                        // No stored permit - try passed permit or local
                        if !permit.is_empty() {
                            debug!("Node mode: Using passed permit for peer {} (permit_len={})", peer_did, permit.len());
                            permit.to_string()
                        } else {
                            match state.butler.get_page(page_id) {
                                Ok(Some(page_data)) => {
                                    debug!("Node mode: Using local permit for peer {} (no stored permit)", peer_did);
                                    page_data.permit.clone().unwrap_or_default()
                                }
                                _ => String::new()
                            }
                        }
                    }
                }
            }
            CourierMode::User => {
                // User mode: Use our local permit - we control what we send
                match state.butler.get_page(page_id) {
                    Ok(Some(page_data)) => {
                        debug!("User mode: Using local permit for subscription (our sync capabilities)");
                        page_data.permit.clone().unwrap_or_default()
                    }
                    _ => String::new()
                }
            }
        };

        // Create CRDT broadcast channel
        let (tx, mut rx) = tokio::sync::mpsc::channel::<butler::BroadcastPayload>(32);

        // Create ephemeral broadcast channel (for cursor/typing/presence)
        let (eph_tx, mut eph_rx) = tokio::sync::mpsc::channel::<butler::EphemeralOutbound>(64);

        // Subscribe to Scribe with both CRDT and ephemeral channels
        let device_id = self.node_id.to_string();
        if let Err(e) = scribe.cast(butler::ScribeMessage::Subscribe {
            user_did: peer_did.clone(),
            device_id: device_id.clone(),
            broadcast_tx: tx,
            ephemeral_tx: Some(eph_tx),
            permit: subscription_permit,
        }) {
            error!("Failed to subscribe to Scribe for page {}: {}", page_id, e);
            return;
        }

        info!("Subscribed to page {} for peer {} (with ephemeral channel)", page_id, peer_did);

        // Spawn listener task that forwards CRDT broadcasts to this actor
        let actor_ref = myself.clone();
        let page_id_clone = page_id.to_string();
        let listener_handle = tokio::spawn(async move {
            while let Some(payload) = rx.recv().await {
                if let Err(_) = actor_ref.cast(super::PeerMessage::BroadcastReceived(payload)) {
                    // Actor stopped, exit loop
                    break;
                }
            }
            debug!("Broadcast listener stopped for page {}", page_id_clone);
        });

        // Spawn ephemeral listener task that sends datagrams directly to peer
        let conn_for_eph = state.conn.clone();
        let page_id_for_eph = page_id.to_string();
        let node_id_for_eph = self.node_id;
        tokio::spawn(async move {
            while let Some(outbound) = eph_rx.recv().await {
                // Create protocol-level EphemeralDatagram
                let datagram = crate::message::EphemeralDatagram {
                    page_id: outbound.page_id,
                    payload: outbound.payload,
                };
                if let Ok(data) = datagram.to_bytes() {
                    // send_datagram is sync (Result, not Future)
                    if let Err(e) = conn_for_eph.send_datagram(&data) {
                        debug!("Failed to send ephemeral datagram to {}: {}", node_id_for_eph, e);
                    }
                }
            }
            debug!("Ephemeral listener stopped for page {}", page_id_for_eph);
        });

        // Store full subscription info for cleanup on disconnect
        let subscription = super::PageSubscription {
            scribe,
            listener_handle,
            user_did: peer_did,
            device_id,
        };
        state.page_subscriptions.insert(page_id.to_string(), subscription);
    }

    /// Subscribe this peer to all active Scribes they have access to
    ///
    /// **Context**: Called after handshake completes to auto-subscribe to open pages
    /// **We do**: Query Butler for active Scribes, subscribe to each
    #[instrument(skip(self, myself, state))]
    pub(super) async fn subscribe_to_active_scribes(
        &self,
        myself: ractor::ActorRef<super::PeerMessage>,
        state: &mut PeerActorState,
    ) {
        let peer_did = match require_auth(&state.state) {
            Ok((did, _)) => did.to_string(),
            Err(_) => {
                warn!("Cannot subscribe to active scribes: peer not authenticated");
                return;
            }
        };

        let active_pages = match state.butler.list_active_scribes_for_peer(&peer_did).await {
            Ok(pages) => pages,
            Err(e) => {
                warn!("Failed to list active scribes for peer {}: {}", peer_did, e);
                return;
            }
        };

        if active_pages.is_empty() {
            debug!("No active scribes to subscribe peer {} to", peer_did);
            return;
        }

        info!(
            "Auto-subscribing peer {} to {} active scribes",
            peer_did, active_pages.len()
        );

        for (page_id, permit) in active_pages {
            self.subscribe_to_page(myself.clone(), &page_id, &permit, state).await;
        }
    }

    /// Handle broadcast received from Scribe - encrypt and send as SyncOffer
    ///
    /// **Context**: Scribe sent us an update to forward to this peer
    /// **We do**: Look up consent permit, ECDH encrypt with peer's key, send as SyncOffer
    /// **3-Step**: This initiates the sync protocol; we wait for SyncAccept
    #[instrument(skip(self, state, payload), fields(page_id = %payload.page_id, layer_name = %payload.layer_name))]
    pub(super) async fn handle_broadcast_received(
        &self,
        payload: butler::BroadcastPayload,
        state: &mut PeerActorState,
    ) {
        // Need peer's encryption key
        let peer_encryption_key = match state.peer_encryption_key {
            Some(key) => key,
            None => {
                warn!("Cannot send SyncOffer: peer encryption key not set");
                return;
            }
        };

        // Get peer DID for consent permit lookup
        let peer_did = match require_auth(&state.state) {
            Ok((did, _)) => did.to_string(),
            Err(_) => {
                warn!("Cannot send SyncOffer: peer not authenticated");
                return;
            }
        };

        // Look up consent permit: viewer consent first, then owner's own page permit
        // - For node→viewer sync: use viewer's consent permit (issued by viewer)
        // - For owner→node sync: use owner's page permit (issued when page was created)
        let permit = state.butler.get_viewer_page_consent(&peer_did, &payload.page_id)
            .ok()
            .flatten()
            .or_else(|| {
                // For owner→node: use owner's own page permit
                state.butler.get_page(&payload.page_id)
                    .ok()
                    .flatten()
                    .and_then(|pd| pd.permit.clone())
            });

        let permit = match permit {
            Some(p) => p,
            None => {
                warn!("Cannot send SyncOffer: no permit for peer {} page {}", peer_did, payload.page_id);
                return;
            }
        };

        // Encrypt update using ECDH
        let (ephemeral_public, encrypted_data) = match herald::encrypt_for_transfer(
            &peer_encryption_key,
            &payload.update,
        ) {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to encrypt update for SyncOffer: {}", e);
                return;
            }
        };

        // Track this pending sync offer
        let key = (payload.page_id.clone(), payload.layer_name.clone());
        state.pending_sync_offers.insert(
            key,
            super::PendingSyncOffer {
                our_state_vector: payload.state_vector.clone(),
                permit: permit.clone(),
            },
        );

        // Send SyncOffer with our state vector
        let msg = Message::SyncOffer {
            page_id: payload.page_id,
            layer_name: payload.layer_name,
            data: encrypted_data,
            state_vector: payload.state_vector,
            ephemeral_public,
            permit,
        };

        self.send_message(&msg, state).await;
        debug!("Sent SyncOffer to peer {}", self.node_id);
    }

    // ==================== Helpers ====================

    /// Send a SpaceRequestError message
    async fn send_space_request_error(&self, request_id: &str, error: &str, state: &PeerActorState) {
        error!("SpaceRequest error: {}", error);
        let msg = Message::SpaceRequestError {
            request_id: request_id.to_string(),
            error: error.to_string(),
        };
        self.send_message(&msg, state).await;
    }

    // ==================== Issue Sync Consent (Viewer → Node) ====================

    /// Issue sync consent permits and send to node (User/Viewer mode)
    ///
    /// **Context**: Viewer received space + pages, now issues consent permits
    /// **We do**: Issue consent permits via gurkha, send SyncConsentGrant to node
    /// **Node stores**: These permits to attach to future SyncOffer messages
    #[instrument(skip(self, state, space_template, page_template), fields(space_id = %space_id))]
    pub(super) async fn issue_sync_consent(
        &self,
        space_id: &str,
        space_template: &str,
        page_template: &str,
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "issue_sync_consent").is_some() {
            return;
        }

        // Get node's public key from authenticated state (this is who we're issuing permits to)
        let node_pubkey = match require_auth(&state.state) {
            Ok((did, _)) => did.to_string(),
            Err(_) => {
                warn!("Cannot issue sync consent: not authenticated with node");
                return;
            }
        };

        // Get viewer's signing key
        let signing_key = match state.butler.signing_key().await {
            Ok(key) => key,
            Err(e) => {
                error!("Failed to get signing key for consent permits: {}", e);
                return;
            }
        };

        // Get space data with permit (node's viewer permit for space)
        let space_data = match state.butler.get_space_data(space_id) {
            Ok(Some(space)) => space,
            Ok(None) => {
                error!("Space {} not found for consent issuance", space_id);
                return;
            }
            Err(e) => {
                error!("Failed to get space {}: {}", space_id, e);
                return;
            }
        };

        let node_space_permit = match space_data.get_permit() {
            Some(permit) => permit.clone(),
            None => {
                error!("No permit found for space {} - cannot issue consent", space_id);
                return;
            }
        };

        // Issue space consent permit
        let (space_consent_permit, _cid) = match gurkha::issue_sync_space_consent(
            &signing_key,
            &node_pubkey,
            space_id,
            &node_space_permit,
            space_template,
        ).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to issue space consent permit: {}", e);
                return;
            }
        };

        info!("Issued space consent permit for space {}", space_id);

        // Get all pages in space and issue page consent permits
        let pages = match state.butler.list_pages(space_id) {
            Ok(pages) => pages,
            Err(e) => {
                error!("Failed to list pages in space {}: {}", space_id, e);
                return;
            }
        };

        let mut page_consent_permits: Vec<(String, String)> = Vec::with_capacity(pages.len());

        for page in pages {
            // Get page data with permit
            let page_data = match state.butler.get_page(&page.id) {
                Ok(Some(data)) => data,
                Ok(None) => {
                    warn!("Page {} not found - skipping consent", page.id);
                    continue;
                }
                Err(e) => {
                    warn!("Failed to get page {}: {} - skipping consent", page.id, e);
                    continue;
                }
            };

            let node_page_permit = match page_data.get_permit() {
                Some(permit) => permit.clone(),
                None => {
                    warn!("No permit found for page {} - skipping consent", page.id);
                    continue;
                }
            };

            // Issue page consent permit
            let (page_consent_permit, _cid) = match gurkha::issue_sync_page_consent(
                &signing_key,
                &node_pubkey,
                &page.id,
                &node_page_permit,
                page_template,
            ).await {
                Ok(result) => result,
                Err(e) => {
                    warn!("Failed to issue page consent permit for {}: {}", page.id, e);
                    continue;
                }
            };

            page_consent_permits.push((page.id.clone(), page_consent_permit));
        }

        info!(
            "Issued {} page consent permits for space {}",
            page_consent_permits.len(),
            space_id
        );

        // Send SyncConsentGrant to node
        let request_id = uuid::Uuid::new_v4().to_string();
        let msg = Message::SyncConsentGrant {
            request_id,
            space_id: space_id.to_string(),
            space_consent_permit,
            page_consent_permits,
        };

        self.send_message(&msg, state).await;
        info!("Sent SyncConsentGrant for space {} to node", space_id);
    }

    /// Issue space consent permit only (when space is received)
    ///
    /// **Context**: Viewer received space, issues consent immediately
    /// **We do**: Issue space consent permit via gurkha, send SyncConsentGrant
    #[instrument(skip(self, state, node_space_permit, space_template), fields(space_id = %space_id))]
    async fn issue_space_consent(
        &self,
        space_id: &str,
        node_space_permit: &str,
        space_template: &str,
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "issue_space_consent").is_some() {
            return;
        }

        // Extract node's DID from the permit (iss field)
        // This works for both handshake and SpaceRequest flows
        let node_pubkey = match parse_permit(node_space_permit) {
            Ok(permit) => permit.parsed().issuer().to_string(),
            Err(e) => {
                error!("Cannot issue space consent: failed to parse permit: {}", e);
                return;
            }
        };

        let signing_key = match state.butler.signing_key().await {
            Ok(key) => key,
            Err(e) => {
                error!("Failed to get signing key for space consent: {}", e);
                return;
            }
        };

        let (space_consent_permit, _cid) = match gurkha::issue_sync_space_consent(
            &signing_key,
            &node_pubkey,
            space_id,
            node_space_permit,
            space_template,
        ).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to issue space consent permit: {}", e);
                return;
            }
        };

        info!("Issued space consent permit for space {}", space_id);

        // Send SyncConsentGrant with only space consent (no page consents yet)
        let request_id = uuid::Uuid::new_v4().to_string();
        let msg = Message::SyncConsentGrant {
            request_id,
            space_id: space_id.to_string(),
            space_consent_permit,
            page_consent_permits: vec![],
        };

        self.send_message(&msg, state).await;
        info!("Sent space consent for space {} to node", space_id);
    }

    /// Issue page consent permit (when each page is received)
    ///
    /// **Context**: Viewer received a page, issues consent for that page
    /// **We do**: Issue page consent permit via gurkha, send SyncConsentGrant
    #[instrument(skip(self, state, node_page_permit, page_template), fields(page_id = %page_id))]
    async fn issue_page_consent(
        &self,
        page_id: &str,
        node_page_permit: &str,
        page_template: &str,
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "issue_page_consent").is_some() {
            return;
        }

        // Extract node's DID from the permit (iss field)
        // This works for both handshake and SpaceRequest flows
        let node_pubkey = match parse_permit(node_page_permit) {
            Ok(permit) => permit.parsed().issuer().to_string(),
            Err(e) => {
                error!("Cannot issue page consent: failed to parse permit: {}", e);
                return;
            }
        };

        let signing_key = match state.butler.signing_key().await {
            Ok(key) => key,
            Err(e) => {
                error!("Failed to get signing key for page consent: {}", e);
                return;
            }
        };

        // Get space_id from page for the message
        let space_id = match state.butler.get_page(page_id) {
            Ok(Some(page_data)) => page_data.meta.space_id,
            Ok(None) => {
                error!("Page {} not found - cannot issue consent", page_id);
                return;
            }
            Err(e) => {
                error!("Failed to get page {}: {}", page_id, e);
                return;
            }
        };

        let (page_consent_permit, _cid) = match gurkha::issue_sync_page_consent(
            &signing_key,
            &node_pubkey,
            page_id,
            node_page_permit,
            page_template,
        ).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to issue page consent permit for {}: {}", page_id, e);
                return;
            }
        };

        info!("Issued page consent permit for page {}", page_id);

        // Send SyncConsentGrant with only this page consent (empty space consent)
        let request_id = uuid::Uuid::new_v4().to_string();
        let msg = Message::SyncConsentGrant {
            request_id,
            space_id,
            space_consent_permit: String::new(), // Empty for page-only consent
            page_consent_permits: vec![(page_id.to_string(), page_consent_permit)],
        };

        self.send_message(&msg, state).await;
        info!("Sent page consent for page {} to node", page_id);
    }

    // ==================== Receive Sync Consent (Node receives from Viewer) ====================

    /// Handle incoming SyncConsentGrant from viewer (Node mode)
    ///
    /// **Context**: Viewer issued consent permits after receiving space content
    /// **We store**: Viewer's consent permits keyed by (viewer_did, space_id/page_id)
    /// **We send**: SyncConsentAck to confirm receipt
    #[instrument(skip(self, state, space_consent_permit, page_consent_permits), fields(space_id = %space_id))]
    pub(super) async fn on_sync_consent_grant(
        &self,
        request_id: &str,
        space_id: &str,
        space_consent_permit: &str,
        page_consent_permits: &[(String, String)],
        state: &mut PeerActorState,
    ) {
        // Extract viewer's DID from the consent permit (iss field)
        // The viewer issues consent permits, so `iss` contains viewer's DID
        // This works for SpaceRequest flow where viewer isn't handshake-authenticated
        let viewer_did = if !space_consent_permit.is_empty() {
            match parse_permit(space_consent_permit) {
                Ok(permit) => permit.parsed().issuer().to_string(),
                Err(e) => {
                    error!("Cannot process SyncConsentGrant: failed to parse space consent permit: {}", e);
                    return;
                }
            }
        } else if let Some((_, permit)) = page_consent_permits.first() {
            match parse_permit(permit) {
                Ok(permit) => permit.parsed().issuer().to_string(),
                Err(e) => {
                    error!("Cannot process SyncConsentGrant: failed to parse page consent permit: {}", e);
                    return;
                }
            }
        } else {
            error!("SyncConsentGrant has no permits to extract viewer DID from");
            return;
        };

        info!(
            "SyncConsentGrant: space={} from viewer={} ({} page consents)",
            space_id, viewer_did, page_consent_permits.len()
        );

        // Store viewer consent permits via Butler
        if let Err(e) = state.butler.store_viewer_consent(
            &viewer_did,
            space_id,
            space_consent_permit,
            page_consent_permits,
        ).await {
            error!("Failed to store viewer consent permits: {}", e);
            return;
        }

        info!("Stored viewer consent permits for {} space={}", viewer_did, space_id);

        // Send acknowledgment
        let msg = Message::SyncConsentAck {
            request_id: request_id.to_string(),
            space_id: space_id.to_string(),
        };
        self.send_message(&msg, state).await;
    }

    /// Handle incoming SyncConsentAck from node (Viewer-side handler)
    ///
    /// **Context**: Node has acknowledged our consent permits
    /// **Node sends**: SyncConsentAck confirming storage
    /// **We know**: Node can now send us sync updates with our consent permits attached
    #[instrument(skip(self, state), fields(space_id = %space_id))]
    pub(super) async fn on_sync_consent_ack(
        &self,
        request_id: &str,
        space_id: &str,
        state: &mut PeerActorState,
    ) {
        let (node_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("SyncConsentAck from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        info!(
            "SyncConsentAck: space={} from node={} request={}",
            space_id, node_did, request_id
        );

        // Notify coordinator that consent handshake is complete
        let _ = state.coordinator.cast(CoordinatorMessage::SyncConsentComplete {
            node_id: self.node_id,
            space_id: space_id.to_string(),
        });
    }

    // ==================== Asset Sync Helpers ====================

    /// Trigger asset sync after receiving an assets layer update via Loro sync
    ///
    /// **Context**: We received a SyncAck for an assets layer, meaning new metadata was synced
    /// **Flow**:
    ///   1. Get the assets layer from Scribe
    ///   2. Parse asset metadata
    ///   3. Find assets missing from local AssetStore
    ///   4. Send AssetPrepare for each missing asset
    ///
    /// **Note**: Spawns a task to avoid blocking sync pipeline
    #[instrument(skip(self, state), fields(page_id = %page_id, layer = %layer_name))]
    async fn trigger_asset_sync_after_layer_sync(
        &self,
        page_id: &str,
        layer_name: &str,
        state: &mut PeerActorState,
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

// ==================== Mutation Validation (Node-side enforcement) ====================

/// Validate viewer mutation using permit's layer patterns
///
/// **Context**: Node receives SyncOffer from viewer, wants to validate before applying
///
/// **Implementation**: Identity-based check via can_access_layer
/// Authorization logic is in the app code (checked by permit:role())
#[allow(dead_code)]
pub fn validate_viewer_mutation(
    permit: &gurkha::Permit,
    layer_name: &str,
) -> Result<(), String> {
    // Identity-based check: can viewer access this layer?
    if !gurkha::can_access_layer(permit, layer_name, "sync") {
        return Err(format!("Viewer cannot sync layer '{}'", layer_name));
    }

    // State-based authorization is handled in app code via permit:role()
    Ok(())
}
