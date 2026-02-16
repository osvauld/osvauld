//! Sync Consent
//!
//! Handles viewer issuing consent permits to node, and node receiving/storing them.
//! Consent permits authorize the node to send sync updates to the viewer.

use tracing::{error, info, instrument, warn};

use crate::message::*;
use transport::Connection;

use super::guards::{parse_permit, require_auth, require_user_mode};
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
                error!(
                    "No permit found for space {} - cannot issue consent",
                    space_id
                );
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
        )
        .await
        {
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
            )
            .await
            {
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
        )
        .await
        {
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
                    error!(
                        "Cannot process SyncConsentGrant: failed to parse space consent permit: {}",
                        e
                    );
                    return;
                }
            }
        } else if let Some((_, permit)) = page_consent_permits.first() {
            match parse_permit(permit) {
                Ok(permit) => permit.parsed().issuer().to_string(),
                Err(e) => {
                    error!(
                        "Cannot process SyncConsentGrant: failed to parse page consent permit: {}",
                        e
                    );
                    return;
                }
            }
        } else {
            error!("SyncConsentGrant has no permits to extract viewer DID from");
            return;
        };

        info!(
            "SyncConsentGrant: space={} from viewer={} ({} page consents)",
            space_id,
            viewer_did,
            page_consent_permits.len()
        );

        // Store viewer consent permits via Butler
        if let Err(e) = state
            .butler
            .contacts()
            .store_viewer_consent(
                &viewer_did,
                space_id,
                space_consent_permit,
                page_consent_permits,
            )
            .await
        {
            error!("Failed to store viewer consent permits: {}", e);
            return;
        }

        info!(
            "Stored viewer consent permits for {} space={}",
            viewer_did, space_id
        );

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
            scope: PermitScope::Page {
                page_id: page_id.to_string(),
            },
        });

        self.send_message(&msg, state).await;
        info!("Sent PermitUpdate for page={}", page_id);
    }
}
