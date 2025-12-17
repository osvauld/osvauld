//! Publishing handlers for owner → node communication
//!
//! Flow:
//! 1. Owner calls initiate_publish_space → sends PublishSpace
//! 2. Node receives, stores space, sends PublishSpaceAck
//! 3. Owner calls initiate_publish_page (per page) → sends PublishPage
//! 4. Node receives, stores page, sends PublishPageAck
//!
//! Shareable links:
//! 1. Owner calls initiate_get_shareable_link → sends GetShareableLinkRequest
//! 2. Node generates aud:* viewer permit → sends GetShareableLinkResponse

use base64::{engine::general_purpose::STANDARD, Engine};
use tracing::{debug, error, info, warn, instrument};

use crate::coordinator::CoordinatorMessage;
use crate::message::Message;

use super::guards::{
    require_user_mode, require_node_mode, require_auth, parse_permit,
    to_published_space, to_published_page_meta, from_published_space, from_published_page_meta,
};
use super::{PeerActor, PeerActorState};

impl PeerActor {
    // ==================== Publishing: Owner initiates ====================

    /// Initiate publishing a space to the connected node (User mode)
    ///
    /// **Context**: Owner wants to publish a space
    /// **We do**: Get space from Butler, delegate permit, send PublishSpace
    #[instrument(skip(self, state), fields(space_id = %space_id))]
    pub(super) async fn initiate_publish_space(&self, space_id: &str, state: &mut PeerActorState) {
        if require_user_mode(state.mode, "initiate_publish_space").is_some() {
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!("Cannot publish space: not authenticated");
            return;
        }

        info!("Publishing space {} to node {}", space_id, self.node_id);

        // Get space from Butler
        let space = match state.butler.get_space(space_id) {
            Ok(Some(s)) => s,
            Ok(None) => {
                error!("Space {} not found", space_id);
                return;
            }
            Err(e) => {
                error!("Failed to get space {}: {}", space_id, e);
                return;
            }
        };

        // Delegate permit to node
        let node_pubkey = self.node_id.to_string();
        let delegated_permit = match state.butler.delegate_space_to_node(space_id, &node_pubkey).await {
            Ok(permit) => permit,
            Err(e) => {
                error!("Failed to delegate space permit: {}", e);
                return;
            }
        };

        // Get page IDs for this space
        let pages = state.butler.list_page_ids_for_space(space_id)
            .unwrap_or_default();

        let request_id = uuid::Uuid::new_v4().to_string();
        let msg = Message::PublishSpace {
            request_id: request_id.clone(),
            space: to_published_space(&space),
            space_permit: delegated_permit,
        };

        info!("Sending PublishSpace for {} (request: {}, {} pages)", space_id, request_id, pages.len());
        self.send_message(&msg, state).await;
    }

    /// Initiate publishing a page to node (User mode)
    ///
    /// **Context**: Owner wants to publish a page to their node
    /// **We prepare**: Decrypt layers, re-encrypt with ephemeral ECDH for transit
    /// **We send**: PublishPage message with encrypted layers
    #[instrument(skip(self, state), fields(page_id = %page_id))]
    pub(super) async fn initiate_publish_page(&self, page_id: &str, state: &mut PeerActorState) {
        if require_user_mode(state.mode, "initiate_publish_page").is_some() {
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!("Cannot publish page: not authenticated");
            return;
        }

        info!("Publishing page {} to node {}", page_id, self.node_id);

        // Get node info to obtain encryption key
        let node_id_str = self.node_id.to_string();
        let sovereign_node = match state.butler.get_sovereign_node(&node_id_str) {
            Ok(Some(n)) => n,
            Ok(None) => {
                error!("No sovereign node found for {}", node_id_str);
                return;
            }
            Err(e) => {
                error!("Failed to get sovereign node: {}", e);
                return;
            }
        };

        // Decode the encryption key (X25519) from base64
        let encryption_key_bytes = match STANDARD.decode(&sovereign_node.encryption_key) {
            Ok(b) if b.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&b);
                arr
            }
            Ok(b) => {
                error!("Invalid encryption key length: {} (expected 32)", b.len());
                return;
            }
            Err(e) => {
                error!("Failed to decode encryption key: {}", e);
                return;
            }
        };

        // Prepare page for publishing via Butler (node's DID is used for permit audience)
        let node_did = sovereign_node.did.clone();
        let prepared = match state.butler.prepare_page_for_publish(page_id, &node_did, &encryption_key_bytes).await {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to prepare page for publish: {}", e);
                return;
            }
        };

        let request_id = uuid::Uuid::new_v4().to_string();
        let layer_count = prepared.layers.len();
        let msg = Message::PublishPage {
            request_id: request_id.clone(),
            page: to_published_page_meta(&prepared.meta),
            page_permit: prepared.permit,
            owner_permit: prepared.owner_permit,
            ephemeral_public: prepared.ephemeral_public,
            layers: prepared.layers,
        };

        info!("Sending PublishPage for {} (request: {}, {} layers)", page_id, request_id, layer_count);
        self.send_message(&msg, state).await;
    }

    // ==================== Publishing Handlers (Node-side) ====================

    /// Handle PublishSpace (Node receives from owner)
    ///
    /// **Context**: Owner manually published a space to us
    /// **Peer sends**: PublishSpace with space metadata and delegated permit
    /// **We verify**: Permit is valid, signed by authenticated owner
    /// **We store**: Space metadata + permit via Butler
    /// **We send**: PublishSpaceAck with permit echoed back, or PublishError
    #[instrument(skip(self, state, space, space_permit), fields(space_id = %space.id))]
    pub(super) async fn on_publish_space(
        &self,
        request_id: &str,
        space: &crate::message::PublishedSpace,
        space_permit: &str,
        state: &mut PeerActorState,
    ) {
        if require_node_mode(state.mode, "on_publish_space").is_some() {
            return;
        }

        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                self.send_publish_error(request_id, "Not authenticated", state).await;
                return;
            }
        };

        info!("PublishSpace: {} ({}) from {}", space.id, space.name, peer_did);

        // Validate permit
        let permit = match parse_permit(space_permit) {
            Ok(p) => p,
            Err(e) => {
                self.send_publish_error(request_id, &e, state).await;
                return;
            }
        };

        // Verify permit relationship is "node"
        let relationship = permit.get_fact("relationship")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if relationship != "node" {
            warn!("Space permit relationship is '{}', expected 'node'", relationship);
            self.send_publish_error(request_id, "Permit not delegated to node", state).await;
            return;
        }

        // Verify space_id matches
        let permit_space_id = permit.get_fact("space_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if permit_space_id != space.id {
            warn!("Space ID mismatch: permit has '{}', message has '{}'", permit_space_id, space.id);
            self.send_publish_error(request_id, "Space ID mismatch", state).await;
            return;
        }

        // Store space via Butler
        let butler_space = from_published_space(space);
        if let Err(e) = state.butler.store_published_space(&butler_space, space_permit) {
            error!("Failed to store space: {}", e);
            self.send_publish_error(request_id, &format!("Storage error: {}", e), state).await;
            return;
        }

        info!("Stored published space: {} ({})", space.id, space.name);

        // Get existing page IDs for this space
        let pages = state.butler.list_page_ids_for_space(&space.id)
            .unwrap_or_default();

        let ack = Message::PublishSpaceAck {
            request_id: request_id.to_string(),
            permit: space_permit.to_string(),
            pages,
        };
        self.send_message(&ack, state).await;
    }

    /// Handle PublishPage (Node receives from owner)
    ///
    /// **Context**: Owner sends page with transit-encrypted layers
    /// **Peer sends**: Page metadata, node permit, owner permit, ephemeral public, encrypted layers
    /// **We verify**: Permit validity (relationship="node", issuer=authenticated peer)
    /// **We decrypt**: Layers using ECDH transit key
    /// **We re-encrypt**: Layers with our own AES key
    /// **We store**: Page + node permit + owner permit via Butler
    /// **We send**: PublishPageAck
    #[instrument(skip(self, state, page, page_permit, owner_permit, ephemeral_public, layers), fields(request_id = %request_id, page_id = %page.id))]
    pub(super) async fn on_publish_page(
        &self,
        request_id: &str,
        page: &crate::message::PublishedPageMeta,
        page_permit: &str,
        owner_permit: &str,
        ephemeral_public: &[u8; 32],
        layers: &[(String, Vec<u8>)],
        state: &mut PeerActorState,
    ) {
        if require_node_mode(state.mode, "on_publish_page").is_some() {
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!("PublishPage from unauthenticated peer: {}", self.node_id);
            return;
        }

        info!("PublishPage: {} ({}) from {} - {} layers", page.id, page.name, page.owner_did, layers.len());

        // Validate permit
        let permit = match parse_permit(page_permit) {
            Ok(p) => p,
            Err(e) => {
                self.send_publish_error(request_id, &e, state).await;
                return;
            }
        };

        // Check relationship is "node"
        let relationship = permit.get_fact("relationship")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if relationship.as_deref() != Some("node") {
            error!("Permit relationship is {:?}, expected 'node'", relationship);
            self.send_publish_error(request_id, "Invalid permit relationship", state).await;
            return;
        }

        // Check page_id matches
        let permit_page_id = permit.get_fact("page_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if permit_page_id.as_deref() != Some(&page.id) {
            error!("Permit page_id {:?} doesn't match page {}", permit_page_id, page.id);
            self.send_publish_error(request_id, "Permit page_id mismatch", state).await;
            return;
        }

        // Convert and store (source_node_did is None - owner publishing to node)
        // Store sender's (owner's) state vectors for incremental sync later
        let page_meta = from_published_page_meta(page);
        let transit_layers: Vec<(String, Vec<u8>)> = layers.to_vec();
        let sender_device_id = self.node_id.to_string();
        if let Err(e) = state.butler.store_published_page(
            page_meta,
            page_permit,
            ephemeral_public,
            transit_layers,
            None,
            &page.owner_did,
            &sender_device_id,
        ).await {
            error!("Failed to store published page: {}", e);
            self.send_publish_error(request_id, &format!("Storage failed: {}", e), state).await;
            return;
        }

        info!("Stored published page: {} ({})", page.id, page.name);

        // Store owner's permit for sync authorization (permit-based auth, no DID whitelist)
        if let Err(e) = state.butler.store_user_page_permit(&page.id, &page.owner_did, owner_permit) {
            warn!("Failed to store owner's permit for sync auth: {} - sync may fail", e);
            // Continue - page was stored successfully, sync can still work via subscription
        } else {
            debug!("Stored owner's permit for page {} (sync authorization)", page.id);
        }

        let ack = Message::PublishPageAck {
            request_id: request_id.to_string(),
            page_id: page.id.clone(),
            permit: page_permit.to_string(),
        };
        self.send_message(&ack, state).await;
    }

    /// Handle PublishPageAck (Owner receives from node)
    ///
    /// **Context**: Node acknowledged our PublishPage
    /// **Peer sends**: Ack with page_id and echoed permit
    /// **We verify**: Permit matches what we sent
    /// **We update**: Mark page as published
    #[instrument(skip(self, state, permit), fields(page_id = %page_id))]
    pub(super) async fn on_publish_page_ack(
        &self,
        request_id: &str,
        page_id: &str,
        permit: &str,
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "on_publish_page_ack").is_some() {
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!("PublishPageAck from unauthenticated peer: {}", self.node_id);
            return;
        }

        info!("PublishPageAck: page={}, request={}", page_id, request_id);

        if let Err(e) = parse_permit(permit) {
            warn!("Invalid permit in PublishPageAck: {}", e);
            return;
        }

        // Mark page as published on this node
        let node_id_str = self.node_id.to_string();
        if let Err(e) = state.butler.mark_page_published(page_id, &node_id_str) {
            warn!("Failed to mark page as published: {}", e);
        }

        // Store node's permit for sync authorization (permit-based auth)
        // This allows owner to verify incoming SyncOffer from node
        if let Ok((node_did, _)) = require_auth(&state.state) {
            if let Err(e) = state.butler.store_user_page_permit(page_id, node_did, permit) {
                warn!("Failed to store node's permit for sync auth: {} - sync may fail", e);
            } else {
                debug!("Stored node's permit for page {} (sync authorization)", page_id);
            }

            // Store node's state vectors so we can send incremental updates later
            // The node now has what we just sent, so store our current layer state as their vector
            if let Err(e) = state.butler.store_peer_vectors_from_page(
                page_id,
                node_did,
                &node_id_str,
            ).await {
                warn!("Failed to store node's state vectors: {}", e);
            }
        }

        let _ = state.coordinator.cast(CoordinatorMessage::PagePublished {
            node_id: self.node_id,
            page_id: page_id.to_string(),
        });

        info!("Page {} published successfully to node {}", page_id, self.node_id);
    }

    // ==================== Publishing Handlers (Owner-side) ====================

    /// Handle PublishSpaceAck (Owner receives from node)
    ///
    /// **Context**: Node acknowledged our PublishSpace
    /// **Peer sends**: Ack with echoed permit (space_id derived from permit)
    /// **We verify**: Permit is valid, extract space_id
    /// **We update**: Mark space as published, trigger page sync
    #[instrument(skip(self, state, permit, pages))]
    pub(super) async fn on_publish_space_ack(
        &self,
        request_id: &str,
        permit: &str,
        pages: &[String],
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "on_publish_space_ack").is_some() {
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!("PublishSpaceAck from unauthenticated peer: {}", self.node_id);
            return;
        }

        // Extract space_id from permit
        let space_id = match gurkha::extract_space_id(permit) {
            Ok(id) => id,
            Err(e) => {
                warn!("Failed to extract space_id from permit: {}", e);
                return;
            }
        };

        info!("PublishSpaceAck: space={}, request={}, {} existing pages", space_id, request_id, pages.len());

        // Mark space as published on this node
        let node_id = self.node_id.to_string();
        if let Err(e) = state.butler.mark_space_published(&space_id, &node_id) {
            warn!("Failed to mark space as published: {}", e);
        }

        let _ = state.coordinator.cast(CoordinatorMessage::SpacePublished {
            node_id: self.node_id,
            space_id,
            existing_pages: pages.to_vec(),
        });

        info!("Space published successfully to node {}", self.node_id);
    }

    /// Handle PublishError (Owner receives from node)
    #[instrument(skip(self, state), fields(request_id = %request_id))]
    pub(super) async fn on_publish_error(
        &self,
        request_id: &str,
        error: &str,
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "on_publish_error").is_some() {
            return;
        }

        error!("Publish failed (request {}): {}", request_id, error);

        let _ = state.coordinator.cast(CoordinatorMessage::PublishFailed {
            node_id: self.node_id,
            request_id: request_id.to_string(),
            error: error.to_string(),
        });
    }

    /// Send a PublishError message
    pub(super) async fn send_publish_error(&self, request_id: &str, error: &str, state: &PeerActorState) {
        let msg = Message::PublishError {
            request_id: request_id.to_string(),
            error: error.to_string(),
        };
        self.send_message(&msg, state).await;
    }

    // ==================== Shareable Links ====================

    /// Initiate request for shareable link (User mode)
    ///
    /// **Context**: Owner wants to share a space with viewers
    /// **We send**: GetShareableLinkRequest to node
    /// **Next**: Wait for GetShareableLinkResponse
    #[instrument(skip(self, state), fields(space_id = %space_id))]
    pub(super) async fn initiate_get_shareable_link(&self, space_id: &str, state: &mut PeerActorState) {
        if require_user_mode(state.mode, "initiate_get_shareable_link").is_some() {
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!("Cannot request shareable link: not authenticated");
            return;
        }

        let request_id = uuid::Uuid::new_v4().to_string();

        info!("Requesting shareable link for space {} from node {}", space_id, self.node_id);

        let msg = Message::GetShareableLinkRequest {
            request_id,
            space_id: space_id.to_string(),
        };

        self.send_message(&msg, state).await;
    }

    /// Handle GetShareableLinkRequest (Node receives from owner)
    ///
    /// **Context**: Owner wants to generate a shareable link for viewers
    /// **Peer sends**: GetShareableLinkRequest with space_id
    /// **We verify**: Peer is authenticated owner with access to this space
    /// **We generate**: Full connection string with viewer permit (aud:*)
    /// **We send**: GetShareableLinkResponse with base64-encoded connection string
    #[instrument(skip(self, state), fields(request_id = %request_id, space_id = %space_id))]
    pub(super) async fn on_get_shareable_link_request(
        &self,
        request_id: &str,
        space_id: &str,
        state: &mut PeerActorState,
    ) {
        if require_node_mode(state.mode, "on_get_shareable_link_request").is_some() {
            return;
        }

        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("GetShareableLinkRequest from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        info!("GetShareableLinkRequest: space={} from {}", space_id, peer_did);

        // Verify this space exists and belongs to the owner
        let space = match state.butler.get_space(space_id) {
            Ok(Some(s)) => s,
            Ok(None) => {
                warn!("Space {} not found", space_id);
                return;
            }
            Err(e) => {
                error!("Failed to get space {}: {}", space_id, e);
                return;
            }
        };

        if space.owner_did != peer_did {
            warn!("Peer {} is not owner of space {} (owner: {})", peer_did, space_id, space.owner_did);
            return;
        }

        // Generate full connection string with viewer permit
        // Butler has access to all identity keys and can derive node_id
        let connection_string = match state.butler.generate_viewer_connection_string(
            space_id,
            None, // relay_url - could be obtained from transport if needed
        ).await {
            Ok(cs) => cs,
            Err(e) => {
                error!("Failed to generate viewer connection string: {}", e);
                return;
            }
        };

        info!("Generated shareable connection string for space {} (len: {})", space_id, connection_string.len());

        let msg = Message::GetShareableLinkResponse {
            request_id: request_id.to_string(),
            space_id: space_id.to_string(),
            permit: connection_string, // Full connection string, not just permit
        };

        self.send_message(&msg, state).await;
    }

    /// Handle GetShareableLinkResponse (Owner receives from node)
    ///
    /// **Context**: Node generated a shareable link for us
    /// **Peer sends**: GetShareableLinkResponse with aud:* permit
    /// **We store**: The permit for sharing out-of-band with viewers
    /// **We notify**: Coordinator of successful link generation
    #[instrument(skip(self, state, permit), fields(request_id = %request_id, space_id = %space_id))]
    pub(super) async fn on_get_shareable_link_response(
        &self,
        request_id: &str,
        space_id: &str,
        permit: &str,
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "on_get_shareable_link_response").is_some() {
            return;
        }

        info!("Received shareable link for space {} (request: {})", space_id, request_id);

        let _ = state.coordinator.cast(CoordinatorMessage::ShareableLinkReceived {
            node_id: self.node_id,
            space_id: space_id.to_string(),
            permit: permit.to_string(),
        });
    }
}
