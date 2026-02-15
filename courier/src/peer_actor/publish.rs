//! Publishing handlers for owner <-> node communication
//!
//! Flow:
//! 1. Owner calls initiate_publish_space -> sends PublishSpace
//! 2. Node receives, stores space, sends PublishSpaceAck
//! 3. Owner calls initiate_page_announce (per page) -> sends PageAnnounce (meta + permits only)
//! 4. Node receives, stores page shell, sends PageAnnounceAck
//! 5. Layers arrive later via Scribe subscription / SyncOffer
//!
//! Shareable links:
//! 1. Owner calls initiate_get_shareable_link -> sends GetShareableLinkRequest
//! 2. Node generates aud:* viewer permit -> sends GetShareableLinkResponse

use tracing::{debug, error, info, instrument, warn};

use crate::message::*;

use transport::Connection;

use super::guards::{
    from_published_page_meta, from_published_space, parse_permit, require_auth, require_node_mode,
    require_user_mode, to_published_page_meta, to_published_space,
};
use super::{PeerActor, PeerActorState};

impl<C: Connection> PeerActor<C> {
    /// Initiate publishing a space to the connected node (User mode)
    ///
    /// **Context**: Owner wants to publish a space
    /// **We do**: Get space from Butler, delegate permit, send PublishSpace
    #[instrument(skip(self, state), fields(space_id = %space_id))]
    pub(super) async fn initiate_publish_space(
        &self,
        space_id: &str,
        state: &mut PeerActorState<C>,
    ) {
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
        let delegated_permit = match state
            .butler
            .spaces()
            .delegate_to_node(space_id, &node_pubkey)
            .await
        {
            Ok(permit) => permit,
            Err(e) => {
                error!("Failed to delegate space permit: {}", e);
                return;
            }
        };

        // Get page IDs for this space
        let pages = state
            .butler
            .spaces()
            .list_page_ids(space_id)
            .unwrap_or_default();

        let request_id = uuid::Uuid::new_v4().to_string();
        let msg = Message::PublishSpace(PublishSpaceMsg {
            request_id: request_id.clone(),
            space: to_published_space(&space),
            space_permit: delegated_permit,
        });

        info!(
            "Sending PublishSpace for {} (request: {}, {} pages)",
            space_id,
            request_id,
            pages.len()
        );
        self.send_message(&msg, state).await;
    }

    /// Initiate announcing a page to node (User mode)
    ///
    /// **Context**: Owner wants to announce a page to their node
    /// **We send**: PageAnnounce with meta + delegated permit + owner permit (NO layers)
    /// **Next**: After PageAnnounceAck, owner subscribes to Scribe and layers arrive via SyncOffer
    #[instrument(skip(self, state), fields(page_id = %page_id))]
    pub(super) async fn initiate_page_announce(
        &self,
        page_id: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_user_mode(state.mode, "initiate_page_announce").is_some() {
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!("Cannot announce page: not authenticated");
            return;
        }

        info!("Announcing page {} to node {}", page_id, self.node_id);

        // Get page metadata from Butler
        let page_data = match state.butler.pages().get(page_id) {
            Ok(Some(p)) => p,
            Ok(None) => {
                error!("Page {} not found", page_id);
                return;
            }
            Err(e) => {
                error!("Failed to get page {}: {}", page_id, e);
                return;
            }
        };

        // Get owner's permit from page data
        let owner_permit = match page_data.get_permit() {
            Some(p) => p.clone(),
            None => {
                error!("No permit found for page {}", page_id);
                return;
            }
        };

        // Delegate permit to node
        let node_pubkey = self.node_id.to_string();
        let signing_key = match state.butler.signing_key().await {
            Ok(k) => k,
            Err(e) => {
                error!("Failed to get signing key: {}", e);
                return;
            }
        };

        let (delegated_permit, _cid) =
            match gurkha::delegate_page(&signing_key, &owner_permit, "node", &node_pubkey).await {
                Ok(result) => result,
                Err(e) => {
                    error!("Failed to delegate page permit: {}", e);
                    return;
                }
            };

        let request_id = uuid::Uuid::new_v4().to_string();
        let msg = Message::PageAnnounce(PageAnnounceMsg {
            request_id: request_id.clone(),
            page: to_published_page_meta(&page_data.meta),
            page_permit: delegated_permit,
            owner_permit,
        });

        info!(
            "Sending PageAnnounce for {} (request: {})",
            page_id, request_id
        );
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
        space: &PublishedSpace,
        space_permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_node_mode(state.mode, "on_publish_space").is_some() {
            return;
        }

        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                self.send_publish_error(request_id, "Not authenticated", state)
                    .await;
                return;
            }
        };

        info!(
            "PublishSpace: {} ({}) from {}",
            space.id, space.name, peer_did
        );

        // Validate permit
        let Some(permit) = self
            .parse_permit_or_respond(
                space_permit,
                Message::PublishError(PublishErrorMsg {
                    request_id: request_id.to_string(),
                    error: "Invalid permit".to_string(),
                }),
                state,
            )
            .await
        else {
            return;
        };

        // Verify permit allows accepting publish (peer_capabilities.accept_publish)
        let peer_caps = permit.peer_capabilities();
        if !peer_caps.accept_publish {
            warn!("Space permit lacks accept_publish capability");
            self.send_publish_error(request_id, "Permit lacks accept_publish capability", state)
                .await;
            return;
        }

        // Verify space_id matches
        let permit_space_id = permit.space_id().unwrap_or_default();
        if permit_space_id != space.id {
            warn!(
                "Space ID mismatch: permit has '{}', message has '{}'",
                permit_space_id, space.id
            );
            self.send_publish_error(request_id, "Space ID mismatch", state)
                .await;
            return;
        }

        // Store space via Butler
        let butler_space = from_published_space(space);
        if let Err(e) = state
            .butler
            .publish()
            .store_space(&butler_space, space_permit)
        {
            error!("Failed to store space: {}", e);
            self.send_publish_error(request_id, &format!("Storage error: {}", e), state)
                .await;
            return;
        }

        info!("Stored published space: {} ({})", space.id, space.name);

        // Issue permit back to owner (node->owner permit proves space is published)
        let owner_pubkey = permit.user_id().unwrap_or_else(|| peer_did.clone());

        let node_permit = match state
            .butler
            .publish()
            .issue_space_permit_to_owner(&space.id, &owner_pubkey)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    "Failed to issue node permit to owner: {} - using echoed permit",
                    e
                );
                space_permit.to_string()
            }
        };

        // Get existing page IDs for this space
        let pages = state
            .butler
            .spaces()
            .list_page_ids(&space.id)
            .unwrap_or_default();

        let ack = Message::PublishSpaceAck(PublishSpaceAckMsg {
            request_id: request_id.to_string(),
            permit: node_permit,
            pages,
        });
        self.send_message(&ack, state).await;
    }

    /// Handle PageAnnounce (Node receives from owner)
    ///
    /// **Context**: Owner announces a page (meta + permits only, no layers)
    /// **Peer sends**: Page metadata, delegated page permit, owner permit
    /// **We verify**: Permit validity (accept_publish capability, issuer=authenticated peer)
    /// **We store**: Page metadata + permits via Butler (page shell, no layers)
    /// **We send**: PageAnnounceAck
    /// **Next**: Layers arrive via Scribe subscription / SyncOffer
    #[instrument(skip(self, state, page, page_permit, owner_permit), fields(request_id = %request_id, page_id = %page.id))]
    pub(super) async fn on_page_announce(
        &self,
        request_id: &str,
        page: &PublishedPageMeta,
        page_permit: &str,
        owner_permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_node_mode(state.mode, "on_page_announce").is_some() {
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!("PageAnnounce from unauthenticated peer: {}", self.node_id);
            return;
        }

        info!(
            "PageAnnounce: {} ({}) from {}",
            page.id, page.name, page.owner_did
        );

        // Validate permit
        let Some(permit) = self
            .parse_permit_or_respond(
                page_permit,
                Message::PublishError(PublishErrorMsg {
                    request_id: request_id.to_string(),
                    error: "Invalid permit".to_string(),
                }),
                state,
            )
            .await
        else {
            return;
        };

        // Check permit allows accepting publish
        let peer_caps = permit.peer_capabilities();
        if !peer_caps.accept_publish {
            error!("Permit lacks accept_publish capability");
            self.send_publish_error(request_id, "Permit lacks accept_publish capability", state)
                .await;
            return;
        }

        // Check page_id matches
        let permit_page_id = permit.page_id();
        if permit_page_id.as_deref() != Some(&page.id) {
            error!(
                "Permit page_id {:?} doesn't match page {}",
                permit_page_id, page.id
            );
            self.send_publish_error(request_id, "Permit page_id mismatch", state)
                .await;
            return;
        }

        // Generate AES key for this page on the node
        // (Node always generates its own key — owner's encrypted_key is for the owner)
        let identity = match state.butler.get_identity().await {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to get identity for AES key generation: {}", e);
                self.send_publish_error(request_id, "Internal error", state)
                    .await;
                return;
            }
        };
        let node_public_enc_key: [u8; 32] = identity
            .public_encryption_key()
            .try_into()
            .expect("public_encryption_key should be 32 bytes");

        let aes_key = herald::generate_aes_key();
        let encrypted_key = match herald::encrypt(&node_public_enc_key, &aes_key) {
            Ok(ek) => ek,
            Err(e) => {
                error!("Failed to encrypt AES key for page: {}", e);
                self.send_publish_error(request_id, "Encryption error", state)
                    .await;
                return;
            }
        };

        // Store page shell (metadata + permits + encrypted AES key, no layers)
        let mut page_meta = from_published_page_meta(page);
        page_meta.encrypted_key = encrypted_key;
        let mut page_data = butler::PageData::new(page_meta);
        page_data.set_permit(page_permit.to_string());

        if let Err(e) = state.butler.store().put_page(&page_data) {
            error!("Failed to store announced page: {}", e);
            self.send_publish_error(request_id, &format!("Storage failed: {}", e), state)
                .await;
            return;
        }

        info!("Stored announced page shell: {} ({})", page.id, page.name);

        // Store owner's permit for sync authorization (permit-based auth, no DID whitelist)
        if let Err(e) = state
            .butler
            .permits()
            .page()
            .store(&page.id, &page.owner_did, owner_permit)
        {
            warn!(
                "Failed to store owner's permit for sync auth: {} - sync may fail",
                e
            );
        } else {
            debug!(
                "Stored owner's permit for page {} (sync authorization)",
                page.id
            );
        }

        // Register owner as authorized user for this page
        let permit_cid =
            gurkha::crypto::get_permit_cid(page_permit).unwrap_or_else(|_| "unknown".to_string());
        let sender_did = page.owner_did.clone();
        if let Err(e) = state
            .butler
            .store()
            .put_permit_cid(&page.id, &sender_did, &permit_cid)
        {
            warn!("Failed to store permit CID for page {}: {}", page.id, e);
        }

        let ack = Message::PageAnnounceAck(PageAnnounceAckMsg {
            request_id: request_id.to_string(),
            page_id: page.id.clone(),
            permit: page_permit.to_string(),
        });
        self.send_message(&ack, state).await;
    }

    /// Handle PageAnnounceAck (Owner receives from node)
    ///
    /// **Context**: Node acknowledged our PageAnnounce
    /// **Peer sends**: Ack with page_id and echoed permit
    /// **We verify**: Permit matches what we sent
    /// **We update**: Mark page as published, subscribe to Scribe for layer delivery
    #[instrument(skip(self, myself, state, permit), fields(page_id = %page_id))]
    pub(super) async fn on_page_announce_ack(
        &self,
        myself: ractor::ActorRef<super::PeerMessage>,
        request_id: &str,
        page_id: &str,
        permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_user_mode(state.mode, "on_page_announce_ack").is_some() {
            return;
        }

        if require_auth(&state.state).is_err() {
            warn!(
                "PageAnnounceAck from unauthenticated peer: {}",
                self.node_id
            );
            return;
        }

        info!("PageAnnounceAck: page={}, request={}", page_id, request_id);

        if let Err(e) = parse_permit(permit) {
            warn!("Invalid permit in PageAnnounceAck: {}", e);
            return;
        }

        // Mark page as published on this node
        let node_id_str = self.node_id.to_string();
        if let Err(e) = state
            .butler
            .publish()
            .mark_page_published(page_id, &node_id_str)
        {
            warn!("Failed to mark page as published: {}", e);
        }

        // Store node's permit for sync authorization (permit-based auth)
        // This allows owner to verify incoming SyncOffer from node
        if let Ok((node_did, _)) = require_auth(&state.state) {
            if let Err(e) = state
                .butler
                .permits()
                .page()
                .store(page_id, node_did, permit)
            {
                warn!(
                    "Failed to store node's permit for sync auth: {} - sync may fail",
                    e
                );
            } else {
                debug!(
                    "Stored node's permit for page {} (sync authorization)",
                    page_id
                );
            }

            // Note: Do NOT store peer vectors here. PageAnnounce sends only metadata,
            // not layers. Storing vectors would tell Scribe the node already has the data,
            // causing it to skip sending layers entirely. Scribe subscription handles
            // layer delivery and will set peer vectors after successful sync.
        }

        // Subscribe to the page's Scribe so layers are broadcast to the node
        // This opens the Scribe (loading layers from storage) and starts broadcasting
        info!(
            "Subscribing to Scribe for page {} to deliver layers to node",
            page_id
        );
        self.subscribe_to_page(myself, page_id, permit, state).await;

        info!(
            "Page {} announced successfully to node {}",
            page_id, self.node_id
        );
    }

    /// Handle PublishSpaceAck (Owner receives from node)
    ///
    /// **Context**: Node acknowledged our PublishSpace
    /// **Peer sends**: Ack with node-issued permit (space_id derived from permit)
    /// **We verify**: Permit is valid, extract space_id
    /// **We store**: Node's permit (proves space is published to this node)
    /// **We notify**: Coordinator to trigger page announce
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
                warn!(
                    "PublishSpaceAck from unauthenticated peer: {}",
                    self.node_id
                );
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

        info!(
            "PublishSpaceAck: space={}, request={}, {} existing pages",
            space_id,
            request_id,
            pages.len()
        );

        // Store node's permit (proves space is published to this node)
        if let Err(e) = state
            .butler
            .spaces()
            .store_permit(&space_id, &node_did, permit)
        {
            warn!("Failed to store node's space permit: {}", e);
        } else {
            debug!(
                "Stored node's permit for space {} (published state)",
                space_id
            );
        }

        info!(
            "Space {} published successfully to node {}",
            space_id, self.node_id
        );

        // Trigger page announcing for pages not already on node
        let existing_pages_set: std::collections::HashSet<&str> =
            pages.iter().map(|s| s.as_str()).collect();
        let all_pages = match state.butler.spaces().list_page_ids(&space_id) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to list pages for space {}: {}", space_id, e);
                return;
            }
        };

        let pages_to_announce: Vec<String> = all_pages
            .into_iter()
            .filter(|p| !existing_pages_set.contains(p.as_str()))
            .collect();

        if !pages_to_announce.is_empty() {
            info!(
                "Announcing {} pages for space {} to node {}",
                pages_to_announce.len(),
                space_id,
                self.node_id
            );
            for page_id in pages_to_announce {
                self.initiate_page_announce(&page_id, state).await;
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

    /// Handle PermitUpdate from peer
    ///
    /// **Context**: Peer sends updated permit (e.g., after app update or re-derivation)
    /// **Peer sends**: PermitUpdate with new permit and scope (Space or Page)
    /// **We do**: Validate new permit, update stored permit, notify Scribe
    #[instrument(skip(self, state, permit), fields(scope = ?scope))]
    pub(super) async fn on_permit_update(
        &self,
        permit: &str,
        scope: &PermitScope,
        state: &mut PeerActorState<C>,
    ) {
        if require_auth(&state.state).is_err() {
            warn!("PermitUpdate from unauthenticated peer: {}", self.node_id);
            return;
        }

        let parsed = match parse_permit(permit) {
            Ok(p) => p,
            Err(e) => {
                warn!("Invalid permit in PermitUpdate: {}", e);
                return;
            }
        };

        match scope {
            PermitScope::Space { space_id } => {
                info!("PermitUpdate for space {} from {}", space_id, self.node_id);
                if let Err(e) = state.butler.spaces().store_permit(
                    space_id,
                    &parsed.parsed().issuer().to_string(),
                    permit,
                ) {
                    error!("Failed to store updated space permit: {}", e);
                }
            }
            PermitScope::Page { page_id } => {
                info!("PermitUpdate for page {} from {}", page_id, self.node_id);
                // Store in USER_PAGE_PERMITS table (for peer_resolver lookups on node)
                if let Err(e) = state.butler.permits().page().store(
                    page_id,
                    &parsed.parsed().issuer().to_string(),
                    permit,
                ) {
                    error!("Failed to store updated page permit: {}", e);
                }
                // Also store as PageData.permit ("our" permit) so build_scribe_args
                // can find it as our_permit for Scribe authorization.
                // Without this, viewers receiving PermitUpdate have no our_permit
                // and Scribe rejects all incoming SyncOffers.
                if let Err(e) = state.butler.pages().set_permit(page_id, permit.to_string()) {
                    error!("Failed to set page permit on PageData: {}", e);
                }

                // Layer discovery for viewer happens via LayerSync protocol:
                // Node detects dynamic layers and sends LayerSync with data + permit.
            }
        }
    }

    /// Send a PublishError message
    #[instrument(skip_all, fields(request_id = %request_id, error = %error))]
    pub(super) async fn send_publish_error(
        &self,
        request_id: &str,
        error: &str,
        state: &PeerActorState<C>,
    ) {
        let msg = Message::PublishError(PublishErrorMsg {
            request_id: request_id.to_string(),
            error: error.to_string(),
        });
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
        state
            .pending_shareable_link_requests
            .insert(request_id.clone(), response_tx);

        info!(
            "Requesting shareable link for space {} from node {}",
            space_id, self.node_id
        );

        let msg = Message::GetShareableLinkRequest(GetShareableLinkRequestMsg {
            request_id,
            space_id: space_id.to_string(),
        });

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
                warn!(
                    "GetShareableLinkRequest from unauthenticated peer: {}",
                    self.node_id
                );
                return;
            }
        };

        info!(
            "GetShareableLinkRequest: space={} from {}",
            space_id, peer_did
        );

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
            warn!(
                "Peer {} is not owner of space {} (owner: {})",
                peer_did, space_id, space.owner_did
            );
            return;
        }

        // Generate full connection string with viewer permit
        // Butler has access to all identity keys and can derive node_id
        let connection_string = match state
            .butler
            .nodes()
            .generate_viewer_connection_string(
                space_id, None, // relay_url - could be obtained from transport if needed
            )
            .await
        {
            Ok(cs) => cs,
            Err(e) => {
                error!("Failed to generate viewer connection string: {}", e);
                return;
            }
        };

        info!(
            "Generated shareable connection string for space {} (len: {})",
            space_id,
            connection_string.len()
        );

        let msg = Message::GetShareableLinkResponse(GetShareableLinkResponseMsg {
            request_id: request_id.to_string(),
            space_id: space_id.to_string(),
            permit: connection_string, // Full connection string, not just permit
        });

        self.send_message(&msg, state).await;
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

        info!(
            "Received shareable link for space {} (request: {})",
            space_id, request_id
        );

        // Look up and invoke the callback
        if let Some(response_tx) = state.pending_shareable_link_requests.remove(request_id) {
            let _ = response_tx.send(Ok(connection_string.to_string()));
        } else {
            warn!(
                "No pending request found for shareable link request_id: {}",
                request_id
            );
        }
    }
}
