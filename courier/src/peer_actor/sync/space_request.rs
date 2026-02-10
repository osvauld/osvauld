//! Space Request Flow
//!
//! Handles viewer requesting space content from node:
//! - SpaceRequest → SpaceData → SpaceDataAck → Scribe subscription (layers via SyncOffer)

use tracing::{debug, error, info, warn, instrument};

use crate::message::*;
use transport::Connection;

use super::super::guards::{
    require_user_mode, require_node_mode, require_auth,
    to_published_space, from_published_space, page_to_published_meta,
    from_published_page_meta,
};
use super::super::{PeerActor, PeerActorState, PendingViewerSync};
use super::super::consent::DEFAULT_SPACE_CONSENT_TEMPLATE;

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

        let msg = Message::SpaceRequest(SpaceRequestMsg {
            request_id,
            space_id: space_id.to_string(),
            viewer_did: identity.did().to_string(),
            viewer_public_key,
            viewer_encryption_key,
            viewer_permit: viewer_permit.to_string(),
        });

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
            Message::SpaceRequestError(SpaceRequestErrorMsg { request_id: request_id.to_string(), error: "Invalid permit".to_string() }),
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

        let published_pages: Vec<PublishedPageMeta> = pages.iter().map(|p| page_to_published_meta(p)).collect();
        let page_ids: Vec<String> = published_pages.iter().map(|p| p.id.clone()).collect();
        info!("Space {} has {} pages for viewer {}", space_id, page_ids.len(), viewer_did);

        // Store viewer context for streaming pages after ack
        state.pending_viewer_syncs.insert(
            request_id.to_string(),
            PendingViewerSync {
                viewer_did: viewer_did.to_string(),
                viewer_pubkey_b64: viewer_pubkey_b64.clone(),
                viewer_encryption_key: *viewer_encryption_key,
                page_ids,
            },
        );

        // Send SpaceData with full page metadata
        let msg = Message::SpaceData(SpaceDataMsg {
            request_id: request_id.to_string(),
            space_id: space_id.to_string(),
            delegated_permit: delegated_space_permit,
            space: to_published_space(&space),
            pages: published_pages,
        });

        self.send_message(&msg, state).await;
        info!("Sent SpaceData for space {} to viewer {} (waiting for ack)", space_id, viewer_did);
    }

    /// Handle SpaceDataAck from viewer (Node mode)
    ///
    /// **Context**: Viewer acknowledged SpaceData, now prepare viewer permits and subscribe via Scribe
    /// **Viewer sent**: SpaceDataAck with delegated permit
    /// **We verify**: Permit matches what we delegated
    /// **We prepare**: Viewer permits for each page (delegate + store)
    /// **We subscribe**: Viewer to Scribe for each page (layers delivered via SyncOffer)
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

        // Prepare viewer permits for each page (no PageData messages sent)
        // Scribe's broadcast mechanism will deliver layers via SyncOffer
        let total_pages = pending.page_ids.len();
        for (idx, page_id) in pending.page_ids.iter().enumerate() {
            // Prepare page for viewer (delegates permit, encrypts key)
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

            // Send the viewer's page permit TO the viewer via PermitUpdate
            // Without this, the viewer's Scribe has no permit and rejects all incoming
            // SyncOffers from the node (can_write fails on all 3 paths).
            let permit_msg = Message::PermitUpdate(PermitUpdateMsg {
                permit: prepared.permit.clone(),
                scope: PermitScope::Page { page_id: page_id.clone() },
            });
            self.send_message(&permit_msg, state).await;
            debug!("Sent PermitUpdate for page {} to viewer {}", page_id, pending.viewer_did);

            debug!("Prepared viewer permit {}/{} ({}) for viewer {}", idx + 1, total_pages, page_id, pending.viewer_did);
        }

        info!("Prepared {} page permits for viewer {} in space {} (layers via Scribe SyncOffer)", total_pages, pending.viewer_did, space_id);

        // CRITICAL: Refresh subscriptions now that viewer permits are stored
        // This subscribes the viewer to Scribe for each page, which will deliver
        // layers via SyncOffer. Without this, Scribe broadcasts won't reach the
        // new viewer because PeerActor wasn't subscribed during handshake
        // (viewer had no permits yet).
        self.refresh_subscriptions_after_page_data(myself, state).await;
    }

    /// Handle SpaceData from node (Viewer mode)
    ///
    /// **Context**: Node responded to our SpaceRequest with space metadata + page metas
    /// **Node sent**: SpaceData with delegated permit, space, and page metadata
    /// **We store**: Space with delegated permit + page shells with generated AES keys
    /// **We send**: SpaceDataAck to trigger viewer permit preparation and Scribe subscription
    #[instrument(skip(self, state, space, delegated_permit, pages), fields(request_id = %request_id, space_id = %space_id))]
    pub(in crate::peer_actor) async fn on_space_data(
        &self,
        request_id: &str,
        space_id: &str,
        delegated_permit: &str,
        space: &crate::message::PublishedSpace,
        pages: &[PublishedPageMeta],
        state: &mut PeerActorState<C>,
    ) {
        if require_user_mode(state.mode, "on_space_data").is_some() {
            return;
        }

        info!("SpaceData: space={} pages={}", space_id, pages.len());

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

        info!("Stored space {} with delegated permit and source_did={}, expecting {} pages", space_id, source_did, pages.len());

        // Create page shells for each page (viewer generates own AES key per page)
        let identity = match state.butler.get_identity().await {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to get identity for page creation: {}", e);
                return;
            }
        };
        let viewer_public_enc_key: [u8; 32] = identity.public_encryption_key().try_into()
            .expect("public_encryption_key should be 32 bytes");

        for page in pages {
            // Generate viewer's own AES key for this page
            let aes_key = herald::generate_aes_key();
            let encrypted_key = match herald::encrypt(&viewer_public_enc_key, &aes_key) {
                Ok(ek) => ek,
                Err(e) => {
                    error!("Failed to encrypt AES key for page {}: {}", page.id, e);
                    continue;
                }
            };

            let mut page_meta = from_published_page_meta(page);
            page_meta.encrypted_key = encrypted_key;
            let page_data = butler::PageData::new(page_meta);

            if let Err(e) = state.butler.store().put_page(&page_data) {
                error!("Failed to store page shell {}: {}", page.id, e);
            } else {
                debug!("Created page shell {} ({}) with viewer AES key", page.id, page.name);
            }
        }

        // Issue space consent permit immediately after receiving space
        self.issue_space_consent(space_id, delegated_permit, DEFAULT_SPACE_CONSENT_TEMPLATE, state).await;

        // Send SpaceDataAck to trigger viewer permit preparation and Scribe subscription
        let msg = Message::SpaceDataAck(SpaceDataAckMsg {
            request_id: request_id.to_string(),
            space_id: space_id.to_string(),
            delegated_permit: delegated_permit.to_string(),
        });

        self.send_message(&msg, state).await;
        info!("Sent SpaceDataAck for space {}, waiting for {} pages", space_id, pages.len());
    }

    /// Trigger subscription refresh after viewer permits are prepared
    ///
    /// **Context**: After Node prepares viewer permits, Node must re-subscribe
    /// to its own Scribes so that Node's PeerActor is subscribed for this viewer.
    /// Scribe will then deliver layers to the viewer via SyncOffer broadcasts.
    ///
    /// **Why needed**: On first connection:
    /// 1. Viewer connects -> handshake -> subscribe_to_active_scribes (viewer has no permit yet)
    /// 2. Viewer sends SpaceRequest -> Node prepares permits -> stores them
    /// 3. But Node's PeerActor still isn't subscribed to Node's Scribe for this viewer
    ///
    /// On reconnection, viewer's permit is already on Node, so subscribe_to_active_scribes works.
    #[instrument(skip_all, fields(node = %self.node_id))]
    pub(in crate::peer_actor) async fn refresh_subscriptions_after_page_data(
        &self,
        myself: ractor::ActorRef<super::super::PeerMessage>,
        state: &mut PeerActorState<C>,
    ) {
        info!(node_id = %self.node_id, "Triggering RefreshSubscriptions after viewer permit preparation");
        self.subscribe_to_active_scribes(myself, state).await;
    }

    /// Send a SpaceRequestError message
    #[instrument(skip_all, fields(request_id = %request_id))]
    async fn send_space_request_error(&self, request_id: &str, error: &str, state: &PeerActorState<C>) {
        error!("SpaceRequest error: {}", error);
        let msg = Message::SpaceRequestError(SpaceRequestErrorMsg {
            request_id: request_id.to_string(),
            error: error.to_string(),
        });
        self.send_message(&msg, state).await;
    }
}
