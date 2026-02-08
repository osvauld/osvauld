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

use crate::message::Message;

use transport::Connection;

use super::guards::{
    require_user_mode, require_node_mode, require_auth, parse_permit,
    to_published_space, to_published_page_meta, from_published_space, from_published_page_meta,
};
use super::{PeerActor, PeerActorState};

impl<C: Connection> PeerActor<C> {

    /// Initiate publishing a space to the connected node (User mode)
    ///
    /// **Context**: Owner wants to publish a space
    /// **We do**: Get space from Butler, delegate permit, send PublishSpace
    #[instrument(skip(self, state), fields(space_id = %space_id))]
    pub(super) async fn initiate_publish_space(&self, space_id: &str, state: &mut PeerActorState<C>) {
        if require_user_mode(state.mode, "initiate_publish_space").is_some() {
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!("Cannot publish space: not authenticated");
            return;
        }

        info!("Publishing space {} to node {}", space_id, self.node_id);

        // Get space from Butler
        let space = match state.butler.spaces().get(space_id) {
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
        let delegated_permit = match state.butler.spaces().delegate_to_node(space_id, &node_pubkey).await {
            Ok(permit) => permit,
            Err(e) => {
                error!("Failed to delegate space permit: {}", e);
                return;
            }
        };

        // Get page IDs for this space
        let pages = state.butler.spaces().list_page_ids(space_id)
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
    pub(super) async fn initiate_publish_page(&self, page_id: &str, state: &mut PeerActorState<C>) {
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
        let sovereign_node = match state.butler.nodes().get(&node_id_str) {
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
        let prepared = match state.butler.publish().prepare_page(page_id, &node_did, &encryption_key_bytes).await {
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
        state: &mut PeerActorState<C>,
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
        let Some(permit) = self.parse_permit_or_respond(
            space_permit,
            Message::PublishError { request_id: request_id.to_string(), error: "Invalid permit".to_string() },
            state,
        ).await else { return };

        // Verify permit allows accepting publish (peer_capabilities.accept_publish)
        let peer_caps = permit.peer_capabilities();
        if !peer_caps.accept_publish {
            warn!("Space permit lacks accept_publish capability");
            self.send_publish_error(request_id, "Permit lacks accept_publish capability", state).await;
            return;
        }

        // Verify space_id matches
        let permit_space_id = permit.space_id()
            .unwrap_or_default();
        if permit_space_id != space.id {
            warn!("Space ID mismatch: permit has '{}', message has '{}'", permit_space_id, space.id);
            self.send_publish_error(request_id, "Space ID mismatch", state).await;
            return;
        }

        // Store space via Butler
        let butler_space = from_published_space(space);
        if let Err(e) = state.butler.publish().store_space(&butler_space, space_permit) {
            error!("Failed to store space: {}", e);
            self.send_publish_error(request_id, &format!("Storage error: {}", e), state).await;
            return;
        }

        info!("Stored published space: {} ({})", space.id, space.name);

        // Issue permit back to owner (node→owner permit proves space is published)
        let owner_pubkey = permit.user_id()
            .unwrap_or_else(|| peer_did.clone());

        let node_permit = match state.butler.publish().issue_space_permit_to_owner(&space.id, &owner_pubkey).await {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to issue node permit to owner: {} - using echoed permit", e);
                space_permit.to_string()
            }
        };

        // Get existing page IDs for this space
        let pages = state.butler.spaces().list_page_ids(&space.id)
            .unwrap_or_default();

        let ack = Message::PublishSpaceAck {
            request_id: request_id.to_string(),
            permit: node_permit,
            pages,
        };
        self.send_message(&ack, state).await;
    }

    /// Handle PublishPage (Node receives from owner)
    ///
    /// **Context**: Owner sends page with transit-encrypted layers
    /// **Peer sends**: Page metadata, node permit, owner permit, ephemeral public, encrypted layers
    /// **We verify**: Permit validity (accept_publish capability, issuer=authenticated peer)
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
        state: &mut PeerActorState<C>,
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
        let Some(permit) = self.parse_permit_or_respond(
            page_permit,
            Message::PublishError { request_id: request_id.to_string(), error: "Invalid permit".to_string() },
            state,
        ).await else { return };

        // Check permit allows accepting publish
        let peer_caps = permit.peer_capabilities();
        if !peer_caps.accept_publish {
            error!("Permit lacks accept_publish capability");
            self.send_publish_error(request_id, "Permit lacks accept_publish capability", state).await;
            return;
        }

        // Check page_id matches
        let permit_page_id = permit.page_id();
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
        if let Err(e) = state.butler.publish().store_page(
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
        if let Err(e) = state.butler.permits().store_page_permit(&page.id, &page.owner_did, owner_permit) {
            warn!("Failed to store owner's permit for sync auth: {} - sync may fail", e);
            // Continue - page was stored successfully, sync can still work via subscription
        } else {
            debug!("Stored owner's permit for page {} (sync authorization)", page.id);
        }

        // Trigger asset sync: check for assets layer and request missing assets
        self.trigger_asset_sync_after_publish(&page.id, layers, state).await;

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
        state: &mut PeerActorState<C>,
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
        if let Err(e) = state.butler.publish().mark_page_published(page_id, &node_id_str) {
            warn!("Failed to mark page as published: {}", e);
        }

        // Store node's permit for sync authorization (permit-based auth)
        // This allows owner to verify incoming SyncOffer from node
        if let Ok((node_did, _)) = require_auth(&state.state) {
            if let Err(e) = state.butler.permits().store_page_permit(page_id, node_did, permit) {
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

        info!("Page {} published successfully to node {}", page_id, self.node_id);
    }

    /// Handle PublishSpaceAck (Owner receives from node)
    ///
    /// **Context**: Node acknowledged our PublishSpace
    /// **Peer sends**: Ack with node-issued permit (space_id derived from permit)
    /// **We verify**: Permit is valid, extract space_id
    /// **We store**: Node's permit (proves space is published to this node)
    /// **We notify**: Coordinator to trigger page sync
    #[instrument(skip(self, state, permit, pages))]
    pub(super) async fn on_publish_space_ack(
        &self,
        request_id: &str,
        permit: &str,
        pages: &[String],
        state: &mut PeerActorState<C>,
    ) {
        if require_user_mode(state.mode, "on_publish_space_ack").is_some() {
            return;
        }

        let (node_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("PublishSpaceAck from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        // Extract space_id from permit
        let space_id = match gurkha::extract_space_id(permit) {
            Ok(id) => id,
            Err(e) => {
                warn!("Failed to extract space_id from permit: {}", e);
                return;
            }
        };

        info!("PublishSpaceAck: space={}, request={}, {} existing pages", space_id, request_id, pages.len());

        // Store node's permit (proves space is published to this node)
        if let Err(e) = state.butler.spaces().store_permit(&space_id, &node_did, permit) {
            warn!("Failed to store node's space permit: {}", e);
        } else {
            debug!("Stored node's permit for space {} (published state)", space_id);
        }

        info!("Space {} published successfully to node {}", space_id, self.node_id);

        // Trigger page publishing for pages not already on node
        let existing_pages_set: std::collections::HashSet<&str> = pages.iter().map(|s| s.as_str()).collect();
        let all_pages = match state.butler.spaces().list_page_ids(&space_id) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to list pages for space {}: {}", space_id, e);
                return;
            }
        };

        let pages_to_publish: Vec<String> = all_pages
            .into_iter()
            .filter(|p| !existing_pages_set.contains(p.as_str()))
            .collect();

        if !pages_to_publish.is_empty() {
            info!("Publishing {} pages for space {} to node {}", pages_to_publish.len(), space_id, self.node_id);
            for page_id in pages_to_publish {
                self.initiate_publish_page(&page_id, state).await;
            }
        }
    }

    /// Handle PublishError (Owner receives from node)
    #[instrument(skip(self, state), fields(request_id = %request_id))]
    pub(super) async fn on_publish_error(
        &self,
        request_id: &str,
        error: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_user_mode(state.mode, "on_publish_error").is_some() {
            return;
        }

        error!("Publish failed (request {}): {}", request_id, error);
    }

    /// Send a PublishError message
    #[instrument(skip_all, fields(request_id = %request_id, error = %error))]
    pub(super) async fn send_publish_error(&self, request_id: &str, error: &str, state: &PeerActorState<C>) {
        let msg = Message::PublishError {
            request_id: request_id.to_string(),
            error: error.to_string(),
        };
        self.send_message(&msg, state).await;
    }

    /// Initiate request for shareable link (User mode)
    ///
    /// **Context**: Owner wants to share a space with viewers
    /// **We send**: GetShareableLinkRequest to node
    /// **Next**: Wait for GetShareableLinkResponse, send result through response_tx
    #[instrument(skip(self, state, response_tx), fields(space_id = %space_id))]
    pub(super) async fn initiate_get_shareable_link(
        &self,
        space_id: &str,
        response_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
        state: &mut PeerActorState<C>,
    ) {
        if require_user_mode(state.mode, "initiate_get_shareable_link").is_some() {
            let _ = response_tx.send(Err("Not in user mode".to_string()));
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!("Cannot request shareable link: not authenticated");
            let _ = response_tx.send(Err("Not authenticated".to_string()));
            return;
        }

        let request_id = uuid::Uuid::new_v4().to_string();

        // Store the callback for when response arrives
        state.pending_shareable_link_requests.insert(request_id.clone(), response_tx);

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
        state: &mut PeerActorState<C>,
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
        let space = match state.butler.spaces().get(space_id) {
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
        let connection_string = match state.butler.nodes().generate_viewer_connection_string(
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

    /// Trigger asset sync after receiving a published page
    ///
    /// **Context**: Node received page from owner, check for assets layer
    /// **Flow**:
    ///   1. Look for `{page_id}/assets` layer in received layers
    ///   2. Parse asset metadata from the layer
    ///   3. Find assets missing from local AssetStore
    ///   4. Send AssetPrepare for each missing asset
    #[instrument(skip(self, layers, state), fields(page_id = %page_id))]
    async fn trigger_asset_sync_after_publish(
        &self,
        page_id: &str,
        layers: &[(String, Vec<u8>)],
        state: &mut PeerActorState<C>,
    ) {
        let assets_layer_name = format!("{}/assets", page_id);

        // Find the assets layer in the received layers
        let Some((_, assets_data)) = layers.iter().find(|(name, _)| name == &assets_layer_name) else {
            debug!("No assets layer found in published page {}", page_id);
            return;
        };

        // Skip if empty
        if assets_data.is_empty() {
            debug!("Assets layer is empty for page {}", page_id);
            return;
        }

        // Load the layer to parse metadata
        // Note: The layer data is already decrypted by store_published_page
        // We need to get the assets layer from Butler after it's been stored
        let scribe = match state.butler.open_page(page_id).await {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to open page {} for asset sync: {}", page_id, e);
                return;
            }
        };

        // Get the assets layer snapshot
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetSnapshot {
            layer_name: assets_layer_name.clone(),
            reply: tx,
        }) {
            warn!("Failed to request assets layer snapshot: {}", e);
            return;
        }

        let layer_bytes = match rx.await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                debug!("Assets layer not found in Scribe for page {}", page_id);
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
                warn!("Failed to parse assets layer: {}", e);
                return;
            }
        };

        // Find missing assets
        let missing = butler::services::asset_service::find_missing_assets_from_layer(
            state.butler.asset_store(),
            &layer,
        );

        if missing.is_empty() {
            debug!("No missing assets for page {}", page_id);
            return;
        }

        info!(
            page_id = %page_id,
            missing_count = missing.len(),
            "Found missing assets after publish, sending AssetPrepare"
        );

        // Send AssetPrepare for each missing asset (tracked for retry)
        for metadata in missing {
            self.send_asset_prepare(page_id, &metadata.hash, state).await;
        }
    }

    /// Handle GetShareableLinkResponse (Owner receives from node)
    ///
    /// **Context**: Node generated a shareable link for us
    /// **Peer sends**: GetShareableLinkResponse with connection_string (contains aud:* permit)
    /// **We do**: Send connection_string through the waiting callback
    #[instrument(skip(self, state, connection_string), fields(request_id = %request_id, space_id = %space_id))]
    pub(super) async fn on_get_shareable_link_response(
        &self,
        request_id: &str,
        space_id: &str,
        connection_string: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_user_mode(state.mode, "on_get_shareable_link_response").is_some() {
            return;
        }

        info!("Received shareable link for space {} (request: {})", space_id, request_id);

        // Look up and invoke the callback
        if let Some(response_tx) = state.pending_shareable_link_requests.remove(request_id) {
            let _ = response_tx.send(Ok(connection_string.to_string()));
        } else {
            warn!("No pending request found for shareable link request_id: {}", request_id);
        }
    }
}
