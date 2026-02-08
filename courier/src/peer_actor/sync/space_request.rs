//! Space/Page Request Flow
//!
//! Handles viewer requesting space content from node:
//! - SpaceRequest → SpaceData → SpaceDataAck → PageData (x N)

use tracing::{debug, error, info, warn, instrument};

use crate::message::Message;
use transport::Connection;

use super::super::guards::{
    require_user_mode, require_node_mode, require_auth,
    to_published_space, from_published_space, from_published_page_meta,
};
use super::super::{PeerActor, PeerActorState, PendingViewerSync};
use super::super::consent::{DEFAULT_SPACE_CONSENT_TEMPLATE, DEFAULT_PAGE_CONSENT_TEMPLATE};

impl<C: Connection> PeerActor<C> {

    /// Initiate request for space content as viewer (User mode)
    ///
    /// **Context**: Viewer has aud:* permit from shareable link
    /// **We send**: SpaceRequest to node with our identity and the permit
    /// **Next**: Wait for SpaceData with delegated permit and content
    #[instrument(skip(self, state, viewer_permit), fields(space_id = %space_id))]
    pub(in crate::peer_actor) async fn initiate_request_space_as_viewer(
        &self,
        space_id: &str,
        viewer_permit: &str,
        state: &mut PeerActorState<C>,
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
    pub(in crate::peer_actor) async fn on_space_request(
        &self,
        request_id: &str,
        space_id: &str,
        viewer_did: &str,
        viewer_public_key: &[u8],
        viewer_encryption_key: &[u8; 32],
        viewer_permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_node_mode(state.mode, "on_space_request").is_some() {
            return;
        }

        info!("SpaceRequest: space={} viewer={}", space_id, viewer_did);

        // Validate viewer_permit
        let Some(permit) = self.parse_permit_or_respond(
            viewer_permit,
            Message::SpaceRequestError { request_id: request_id.to_string(), error: "Invalid permit".to_string() },
            state,
        ).await else { return };

        // Verify it's an aud:* token (wildcard audience)
        if permit.parsed().audience() != "*" {
            self.send_space_request_error(request_id, "Viewer permit must have wildcard audience (aud:*)", state).await;
            return;
        }

        // Verify token_type is viewer_auth
        let token_type = permit.token_type();
        if token_type != Some("viewer_auth") {
            self.send_space_request_error(request_id, "Viewer permit must be viewer_auth type", state).await;
            return;
        }

        // Verify space_id in permit matches requested space_id
        let permit_space_id = permit.space_id();
        if permit_space_id.as_deref() != Some(space_id) {
            self.send_space_request_error(request_id, "Viewer permit space_id does not match requested space", state).await;
            return;
        }

        // Verify space exists on this node
        let space = match state.butler.spaces().get(space_id) {
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
        let delegated_space_permit = match state.butler.spaces().delegate_to_viewer(space_id, &viewer_pubkey_b64).await {
            Ok(p) => p,
            Err(e) => {
                self.send_space_request_error(request_id, &format!("Failed to delegate permit: {}", e), state).await;
                return;
            }
        };

        info!("Delegated space permit to viewer {} (permit len: {})", viewer_did, delegated_space_permit.len());

        // Get page IDs in the space
        let pages = match state.butler.pages().list(space_id) {
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
    /// **After streaming**: Refresh subscriptions so Node's PeerActor is subscribed
    ///   to Node's Scribes for this new viewer
    #[instrument(skip(self, myself, state, _delegated_permit), fields(request_id = %request_id, space_id = %space_id))]
    pub(in crate::peer_actor) async fn on_space_data_ack(
        &self,
        myself: ractor::ActorRef<super::super::PeerMessage>,
        request_id: &str,
        space_id: &str,
        _delegated_permit: &str,
        state: &mut PeerActorState<C>,
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
            let prepared = match state.butler.publish().prepare_page_for_viewer(
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
            if let Err(e) = state.butler.permits().put_cid(page_id, &pending.viewer_did, &prepared.permit) {
                warn!("Failed to store permit CID for page {}: {}", page_id, e);
            }

            // Store viewer's page permit for sync authorization
            // This allows the node to authorize incoming SyncOffers from the viewer
            if let Err(e) = state.butler.permits().store_page_permit(page_id, &pending.viewer_did, &prepared.permit) {
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

        // CRITICAL: Refresh subscriptions now that viewer permits are stored
        // This allows Node's PeerActor to receive broadcasts from Node's Scribes
        // for this viewer. Without this, on first connection, Node's Scribe broadcasts
        // (like online_peers) won't reach the new viewer because PeerActor wasn't
        // subscribed during handshake (viewer had no permits yet).
        self.refresh_subscriptions_after_page_data(myself, state).await;
    }

    /// Handle SpaceData from node (Viewer mode)
    ///
    /// **Context**: Node responded to our SpaceRequest with space metadata
    /// **Node sent**: SpaceData with delegated permit and page_ids (no layers)
    /// **We store**: Space with delegated permit
    /// **We send**: SpaceDataAck to trigger page streaming
    #[instrument(skip(self, state, space, delegated_permit), fields(request_id = %request_id, space_id = %space_id))]
    pub(in crate::peer_actor) async fn on_space_data(
        &self,
        request_id: &str,
        space_id: &str,
        delegated_permit: &str,
        space: &crate::message::PublishedSpace,
        page_ids: &[String],
        state: &mut PeerActorState<C>,
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
        if let Err(e) = state.butler.publish().store_space_with_source(
            &butler_space,
            delegated_permit,
            Some(&source_did),
        ) {
            error!("Failed to store space: {}", e);
            return;
        }

        info!("Stored space {} with delegated permit and source_did={}, expecting {} pages", space_id, source_did, page_ids.len());

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
    pub(in crate::peer_actor) async fn on_viewer_page(
        &self,
        myself: ractor::ActorRef<super::super::PeerMessage>,
        request_id: &str,
        space_id: &str,
        meta: &crate::message::PublishedPageMeta,
        permit: &str,
        ephemeral_public: &[u8; 32],
        layers: &[(String, Vec<u8>)],
        is_last: bool,
        state: &mut PeerActorState<C>,
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
        let _butler_page: butler::Page = page_meta.clone().into();
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

        match state.butler.publish().store_page(
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

        // Extract consent_template from permit_template.json layer (app-defined)
        // Falls back to default if not found
        let consent_template = layers.iter()
            .find(|(name, _)| name == "file:permit_template.json")
            .and_then(|(_, data)| serde_json::from_slice::<serde_json::Value>(data).ok())
            .and_then(|template_json| template_json.get("consent_template").cloned())
            .map(|consent| serde_json::to_string(&serde_json::json!({"consent_template": consent})).unwrap_or_else(|_| DEFAULT_PAGE_CONSENT_TEMPLATE.to_string()))
            .unwrap_or_else(|| DEFAULT_PAGE_CONSENT_TEMPLATE.to_string());

        debug!(
            consent_has_layers = consent_template.contains("layers"),
            "Using consent template from app"
        );

        // Issue page consent permit for this page
        self.issue_page_consent(&meta.id, permit, &consent_template, state).await;

        if is_last {
            info!("PageData stream complete for space {}", space_id);
        }
    }

    /// Trigger subscription refresh after PageData stream completes
    ///
    /// **Context**: After Node sends all PageData to viewer, Node must re-subscribe
    /// to its own Scribes so that Node's PeerActor is subscribed for this viewer.
    /// This allows Node to send ephemeral broadcasts (like online_peers) to the viewer.
    ///
    /// **Why needed**: On first connection:
    /// 1. Viewer connects → handshake → subscribe_to_active_scribes (viewer has no permit yet)
    /// 2. Viewer sends RequestSpace → Node sends PageData → consent permit issued
    /// 3. But Node's PeerActor still isn't subscribed to Node's Scribe for this viewer
    ///
    /// On reconnection, viewer's permit is already on Node, so subscribe_to_active_scribes works.
    #[instrument(skip_all, fields(node = %self.node_id))]
    pub(in crate::peer_actor) async fn refresh_subscriptions_after_page_data(
        &self,
        myself: ractor::ActorRef<super::super::PeerMessage>,
        state: &mut PeerActorState<C>,
    ) {
        info!(node_id = %self.node_id, "Triggering RefreshSubscriptions after PageData stream complete");
        self.subscribe_to_active_scribes(myself, state).await;
    }

    /// Send a SpaceRequestError message
    #[instrument(skip_all, fields(request_id = %request_id))]
    async fn send_space_request_error(&self, request_id: &str, error: &str, state: &PeerActorState<C>) {
        error!("SpaceRequest error: {}", error);
        let msg = Message::SpaceRequestError {
            request_id: request_id.to_string(),
            error: error.to_string(),
        };
        self.send_message(&msg, state).await;
    }
}
