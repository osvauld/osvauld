//! Sync Consent
//!
//! Handles viewer issuing consent permits to node, and node receiving/storing them.
//! Consent permits authorize the node to send sync updates to the viewer.

use tracing::{error, info, warn, instrument};

use crate::message::*;
use transport::Connection;

use super::guards::{require_user_mode, require_auth, parse_permit};
use super::{PeerActor, PeerActorState};

// Used for automatic consent issuance when viewer receives space/pages.
// In production, these could be configurable.

/// Default space consent template - viewer consents to receive space sync + new pages
pub(in crate::peer_actor) const DEFAULT_SPACE_CONSENT_TEMPLATE: &str = r#"{
  "consent_template": {
    "token_type": "sync_space_consent",
    "operations": { "receive_pages": "allow", "receive_updates": "allow" },
    "auth_capabilities": { "accept_sync": true, "accept_new_pages": true },
    "relationship": "sync_consent"
  }
}"#;

/// Default page consent template - viewer consents to receive page layer updates
pub(in crate::peer_actor) const DEFAULT_PAGE_CONSENT_TEMPLATE: &str = r#"{
  "consent_template": {
    "token_type": "sync_page_consent",
    "operations": { "receive_layer_updates": "allow" },
    "auth_capabilities": { "accept_sync": true },
    "presence": { "visible": true },
    "relationship": "sync_consent"
  }
}"#;

/// Default layer consent template - viewer consents to receive a specific dynamic layer
pub(in crate::peer_actor) const DEFAULT_LAYER_CONSENT_TEMPLATE: &str = r#"{
  "consent_template": {
    "token_type": "sync_layer_consent",
    "operations": { "receive_layer_updates": "allow" },
    "auth_capabilities": { "accept_sync": true },
    "relationship": "sync_consent"
  }
}"#;

impl<C: Connection> PeerActor<C> {

    /// Issue sync consent permits and send to node (User/Viewer mode)
    ///
    /// **Context**: Viewer received space + pages, now issues consent permits
    /// **We do**: Issue consent permits via gurkha, send SyncConsentGrant to node
    /// **Node stores**: These permits to attach to future SyncOffer messages
    #[instrument(skip(self, state, space_template, page_template), fields(space_id = %space_id))]
    pub(in crate::peer_actor) async fn issue_sync_consent(
        &self,
        space_id: &str,
        space_template: &str,
        page_template: &str,
        state: &mut PeerActorState<C>,
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
        let space_data = match state.butler.spaces().get_data(space_id) {
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
        let pages = match state.butler.pages().list(space_id) {
            Ok(pages) => pages,
            Err(e) => {
                error!("Failed to list pages in space {}: {}", space_id, e);
                return;
            }
        };

        let mut page_consent_permits: Vec<(String, String)> = Vec::with_capacity(pages.len());

        for page in pages {
            // Get page data with permit
            let page_data = match state.butler.pages().get(&page.id) {
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
        let msg = Message::SyncConsentGrant(SyncConsentGrantMsg {
            request_id,
            space_id: space_id.to_string(),
            space_consent_permit,
            page_consent_permits,
        });

        self.send_message(&msg, state).await;
        info!("Sent SyncConsentGrant for space {} to node", space_id);
    }

    /// Issue space consent permit only (when space is received)
    ///
    /// **Context**: Viewer received space, issues consent immediately
    /// **We do**: Issue space consent permit via gurkha, send SyncConsentGrant
    #[instrument(skip(self, state, node_space_permit, space_template), fields(space_id = %space_id))]
    pub(in crate::peer_actor) async fn issue_space_consent(
        &self,
        space_id: &str,
        node_space_permit: &str,
        space_template: &str,
        state: &mut PeerActorState<C>,
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
        let msg = Message::SyncConsentGrant(SyncConsentGrantMsg {
            request_id,
            space_id: space_id.to_string(),
            space_consent_permit,
            page_consent_permits: vec![],
        });

        self.send_message(&msg, state).await;
        info!("Sent space consent for space {} to node", space_id);
    }

    /// Issue page consent permit (when each page is received)
    ///
    /// **Context**: Viewer received a page, issues consent for that page
    /// **We do**: Issue page consent permit via gurkha, send SyncConsentGrant
    #[instrument(skip(self, state, node_page_permit, page_template), fields(page_id = %page_id))]
    pub(in crate::peer_actor) async fn issue_page_consent(
        &self,
        page_id: &str,
        node_page_permit: &str,
        page_template: &str,
        state: &mut PeerActorState<C>,
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
        let space_id = match state.butler.pages().get(page_id) {
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
        let msg = Message::SyncConsentGrant(SyncConsentGrantMsg {
            request_id,
            space_id,
            space_consent_permit: String::new(), // Empty for page-only consent
            page_consent_permits: vec![(page_id.to_string(), page_consent_permit)],
        });

        self.send_message(&msg, state).await;
        info!("Sent page consent for page {} to node", page_id);
    }

    /// Handle incoming SyncConsentGrant from viewer (Node mode)
    ///
    /// **Context**: Viewer issued consent permits after receiving space content
    /// **We store**: Viewer's consent permits keyed by (viewer_did, space_id/page_id)
    /// **We send**: SyncConsentAck to confirm receipt
    #[instrument(skip(self, state, space_consent_permit, page_consent_permits), fields(space_id = %space_id))]
    pub(in crate::peer_actor) async fn on_sync_consent_grant(
        &self,
        request_id: &str,
        space_id: &str,
        space_consent_permit: &str,
        page_consent_permits: &[(String, String)],
        state: &mut PeerActorState<C>,
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
        if let Err(e) = state.butler.contacts().store_viewer_consent(
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
        let msg = Message::SyncConsentAck(SyncConsentAckMsg {
            request_id: request_id.to_string(),
            space_id: space_id.to_string(),
        });
        self.send_message(&msg, state).await;
    }

    /// Handle incoming SyncConsentAck from node (Viewer-side handler)
    ///
    /// **Context**: Node has acknowledged our consent permits
    /// **Node sends**: SyncConsentAck confirming storage
    /// **We know**: Node can now send us sync updates with our consent permits attached
    #[instrument(skip(self, state), fields(space_id = %space_id))]
    pub(in crate::peer_actor) async fn on_sync_consent_ack(
        &self,
        request_id: &str,
        space_id: &str,
        state: &mut PeerActorState<C>,
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

        info!("Sync consent complete for space {}", space_id);
    }

    // --- Layer Permit + Consent ---

    /// Send a LayerPermit to this peer (Node mode)
    ///
    /// **Context**: Coordinator received NewDynamicLayer, forwards to PeerActor
    /// **We do**: Send LayerPermitMsg on the wire
    #[instrument(skip(self, state, permit), fields(page_id = %page_id, layer = %layer_name))]
    pub(in crate::peer_actor) async fn send_layer_permit(
        &self,
        page_id: &str,
        layer_name: &str,
        permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        let request_id = uuid::Uuid::new_v4().to_string();
        let msg = Message::LayerPermit(LayerPermitMsg {
            request_id,
            page_id: page_id.to_string(),
            layer_name: layer_name.to_string(),
            permit: permit.to_string(),
        });

        self.send_message(&msg, state).await;
        info!("Sent LayerPermit for page={} layer={}", page_id, layer_name);
    }

    /// Send a PermitUpdate to this peer (Node mode)
    ///
    /// **Context**: Page permit reissued with new app layers
    /// **We do**: Send PermitUpdate message on the wire
    #[instrument(skip(self, state, permit), fields(page_id = %page_id))]
    pub(in crate::peer_actor) async fn send_page_permit_update(
        &self,
        page_id: &str,
        permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        let msg = Message::PermitUpdate(PermitUpdateMsg {
            permit: permit.to_string(),
            scope: PermitScope::Page { page_id: page_id.to_string() },
        });

        self.send_message(&msg, state).await;
        info!("Sent PermitUpdate for page={}", page_id);
    }

    /// Handle incoming LayerPermit from node (Viewer-side handler)
    ///
    /// **Context**: Node detected a new dynamic layer and issued a layer permit to us
    /// **We store**: Layer permit alongside page permit in butler
    /// **We do**: Auto-issue layer consent back to node
    #[instrument(skip(self, state, permit), fields(page_id = %page_id, layer = %layer_name))]
    pub(in crate::peer_actor) async fn on_layer_permit(
        &self,
        request_id: &str,
        page_id: &str,
        layer_name: &str,
        permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_user_mode(state.mode, "on_layer_permit").is_some() {
            return;
        }

        let (node_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("LayerPermit from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        info!(
            "LayerPermit: page={} layer={} from node={}",
            page_id, layer_name, node_did
        );

        // Get our DID for storage
        let our_did = match state.butler.get_identity().await {
            Ok(identity) => identity.did().to_string(),
            Err(e) => {
                error!("Failed to get our identity: {}", e);
                return;
            }
        };

        // Store layer permit in butler
        if let Err(e) = state.butler.permits().store_layer_permit(
            page_id,
            &our_did,
            layer_name,
            permit,
        ) {
            error!("Failed to store layer permit: {}", e);
            return;
        }

        info!("Stored layer permit for page={} layer={}", page_id, layer_name);

        // Authorize this layer on local Scribe so observer can broadcast to the node
        // The node is a subscriber on our (viewer's) local Scribe
        if let Some(subscription) = state.page_subscriptions.get(page_id) {
            let bare_name = layer_name.strip_prefix(&format!("{}/", page_id))
                .unwrap_or(layer_name);
            subscription.scribe.cast(butler::ScribeMessage::AuthorizeLayerSubscriber {
                layer_name: bare_name.to_string(),
                subscriber_did: node_did.clone(),
            }).ok();
            info!("Sent AuthorizeLayerSubscriber to local Scribe for layer={}", bare_name);
        }

        // Auto-issue layer consent back to node
        self.issue_layer_consent(page_id, layer_name, permit, state).await;
    }

    /// Issue layer consent permit and send to node (Viewer mode)
    ///
    /// **Context**: Viewer received LayerPermit, now consents to sync for that layer
    /// **We do**: Issue consent permit via gurkha, send LayerConsentGrant to node
    /// **Node stores**: Layer consent to authorize future sync for this layer
    #[instrument(skip(self, state, layer_permit_token), fields(page_id = %page_id, layer = %layer_name))]
    pub(in crate::peer_actor) async fn issue_layer_consent(
        &self,
        page_id: &str,
        layer_name: &str,
        layer_permit_token: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_user_mode(state.mode, "issue_layer_consent").is_some() {
            return;
        }

        // Extract node's DID from the layer permit (iss field)
        let node_pubkey = match parse_permit(layer_permit_token) {
            Ok(permit) => permit.parsed().issuer().to_string(),
            Err(e) => {
                error!("Cannot issue layer consent: failed to parse permit: {}", e);
                return;
            }
        };

        let signing_key = match state.butler.signing_key().await {
            Ok(key) => key,
            Err(e) => {
                error!("Failed to get signing key for layer consent: {}", e);
                return;
            }
        };

        let (consent_permit, _cid) = match gurkha::issue_sync_layer_consent(
            &signing_key,
            &node_pubkey,
            page_id,
            layer_name,
            layer_permit_token,
            DEFAULT_LAYER_CONSENT_TEMPLATE,
        ).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to issue layer consent permit: {}", e);
                return;
            }
        };

        info!("Issued layer consent for page={} layer={}", page_id, layer_name);

        // Send LayerConsentGrant to node
        let request_id = uuid::Uuid::new_v4().to_string();
        let msg = Message::LayerConsentGrant(LayerConsentGrantMsg {
            request_id,
            page_id: page_id.to_string(),
            layer_name: layer_name.to_string(),
            consent_permit,
        });

        self.send_message(&msg, state).await;
    }

    /// Handle incoming LayerConsentGrant from viewer (Node-side handler)
    ///
    /// **Context**: Viewer consented to receive sync for a dynamic layer
    /// **We store**: Layer consent in butler
    /// **We send**: LayerConsentAck to confirm
    #[instrument(skip(self, state, consent_permit), fields(page_id = %page_id, layer = %layer_name))]
    pub(in crate::peer_actor) async fn on_layer_consent_grant(
        &self,
        request_id: &str,
        page_id: &str,
        layer_name: &str,
        consent_permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        // Extract viewer's DID from consent permit (iss field)
        let viewer_did = match parse_permit(consent_permit) {
            Ok(permit) => permit.parsed().issuer().to_string(),
            Err(e) => {
                error!("Cannot process LayerConsentGrant: failed to parse permit: {}", e);
                return;
            }
        };

        info!(
            "LayerConsentGrant: page={} layer={} from viewer={}",
            page_id, layer_name, viewer_did
        );

        // Store layer consent in butler
        if let Err(e) = state.butler.permits().store_viewer_layer_consent(
            &viewer_did,
            page_id,
            layer_name,
            consent_permit,
        ) {
            error!("Failed to store viewer layer consent: {}", e);
            return;
        }

        info!("Stored viewer layer consent for page={} layer={} viewer={}", page_id, layer_name, viewer_did);

        // Send acknowledgment
        let msg = Message::LayerConsentAck(LayerConsentAckMsg {
            request_id: request_id.to_string(),
            page_id: page_id.to_string(),
            layer_name: layer_name.to_string(),
        });
        self.send_message(&msg, state).await;
    }

    /// Handle incoming LayerConsentAck from node (Viewer-side handler)
    ///
    /// **Context**: Node acknowledged our layer consent
    /// **We know**: Node can now sync this dynamic layer to us
    #[instrument(skip(self, state), fields(page_id = %page_id, layer = %layer_name))]
    pub(in crate::peer_actor) async fn on_layer_consent_ack(
        &self,
        request_id: &str,
        page_id: &str,
        layer_name: &str,
        state: &mut PeerActorState<C>,
    ) {
        let (node_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("LayerConsentAck from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        info!(
            "LayerConsentAck: page={} layer={} from node={} request={}",
            page_id, layer_name, node_did, request_id
        );

        info!("Layer consent complete for page={} layer={}", page_id, layer_name);
    }
}
