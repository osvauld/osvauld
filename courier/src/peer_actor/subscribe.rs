//! Layer Subscribe Protocol (pull-based via __sync_meta)
//!
//! Handles the pull-based layer discovery and subscription flow:
//! 1. Peer detects new entries in their __sync_meta (via SyncOffer)
//! 2. Peer sends LayerSubscribe for each unsynced layer (with consent permit)
//! 3. Responder validates, exports snapshot, issues permit, responds with LayerSubscribeAck
//! 4. Subscriber decrypts data, stores permit, applies to Scribe, marks synced

use tracing::{debug, error, info, instrument, warn};

use crate::message::*;
use transport::Connection;

use super::guards::require_auth;
use super::{PeerActor, PeerActorState};
use butler::ScribeMessage;

/// Default layer consent template — subscriber consents to receive layer sync updates
const DEFAULT_LAYER_CONSENT_TEMPLATE: &str = r#"{
  "consent_template": {
    "token_type": "sync_layer_consent",
    "operations": { "receive_updates": "allow" },
    "auth_capabilities": { "accept_sync": true },
    "relationship": "sync_consent"
  }
}"#;

impl<C: Connection> PeerActor<C> {
    /// Emit a layer_subscribe_protocol capture event
    ///
    /// **Context**: Traces LayerSubscribe/LayerSubscribeAck outcomes for observability
    fn emit_layer_subscribe_protocol_capture(
        capture_tx: &Option<tokio::sync::broadcast::Sender<String>>,
        direction: &str,
        page_id: &str,
        layer: &str,
        peer_did: &str,
        result: &str,
        error: Option<&str>,
    ) {
        if let Some(ref tx) = capture_tx {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "layer_subscribe_protocol",
                "ts": ts,
                "direction": direction,
                "page_id": page_id,
                "layer": layer,
                "peer_did": peer_did,
                "result": result,
                "error": error,
            })) {
                let _ = tx.send(json);
            }
        }
    }

    /// Issue a layer consent permit for a specific layer
    ///
    /// **Context**: Subscriber is about to send LayerSubscribe, needs consent permit
    /// **Returns**: consent_permit token, or empty string on failure
    async fn issue_layer_consent(
        &self,
        page_id: &str,
        layer_name: &str,
        state: &PeerActorState<C>,
    ) -> String {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => return String::new(),
        };

        let signing_key = match state.butler.signing_key().await {
            Ok(key) => key,
            Err(e) => {
                warn!("Failed to get signing key for layer consent: {}", e);
                return String::new();
            }
        };

        // Get page permit as proof for consent
        let page_permit = match state.butler.pages().get(page_id) {
            Ok(Some(data)) => match data.get_permit() {
                Some(permit) => permit.clone(),
                None => {
                    warn!(
                        "No page permit for page={}, sending consent without proof",
                        page_id
                    );
                    return String::new();
                }
            },
            _ => {
                warn!("Failed to get page data for consent: page={}", page_id);
                return String::new();
            }
        };

        let full_layer_name = format!("{}/{}", page_id, layer_name);
        match gurkha::issue_sync_layer_consent(
            &signing_key,
            &peer_did,
            page_id,
            &full_layer_name,
            &page_permit,
            DEFAULT_LAYER_CONSENT_TEMPLATE,
        )
        .await
        {
            Ok((token, _cid)) => {
                debug!("Issued layer consent for layer={}", layer_name);
                token
            }
            Err(e) => {
                warn!("Failed to issue layer consent: {}", e);
                String::new()
            }
        }
    }

    /// Subscribe to dynamic layers on a peer (triggered by Coordinator)
    ///
    /// **Context**: Scribe emitted SyncEvent::SubscribeLayers, Coordinator forwarded here
    /// **We do**: Issue consent permits, send LayerSubscribe for each layer
    #[instrument(skip(self, state), fields(node = %self.node_id, page_id = %page_id))]
    pub(in crate::peer_actor) async fn subscribe_to_layers(
        &self,
        page_id: &str,
        layers: &[String],
        state: &mut PeerActorState<C>,
    ) {
        if layers.is_empty() {
            return;
        }

        info!(
            page_id = %page_id,
            count = layers.len(),
            "Subscribing to {} layers via LayerSubscribe",
            layers.len()
        );

        for layer_name in layers {
            let consent_permit = self.issue_layer_consent(page_id, layer_name, state).await;
            let request_id = uuid::Uuid::new_v4().to_string();
            let msg = Message::LayerSubscribe(LayerSubscribeMsg {
                request_id,
                page_id: page_id.to_string(),
                layer_name: layer_name.clone(),
                consent_permit,
            });
            self.send_message(&msg, state).await;
            debug!(page_id = %page_id, layer = %layer_name, "Sent LayerSubscribe");
        }
    }

    /// Detect __sync_meta updates and send LayerSubscribe for unsynced entries
    ///
    /// **Context**: PeerActor applied a SyncOffer for a __sync_meta layer
    /// **We do**: Query Scribe for unsynced entries, issue consent, send LayerSubscribe for each
    #[instrument(skip(self, state), fields(node = %self.node_id, page_id = %page_id))]
    pub(in crate::peer_actor) async fn check_sync_meta_and_subscribe(
        &self,
        page_id: &str,
        state: &mut PeerActorState<C>,
    ) {
        let scribe = match state.butler.open_page(page_id).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to open page {} for sync_meta check: {}", page_id, e);
                return;
            }
        };

        // Query Scribe for unsynced __sync_meta entries
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(ScribeMessage::GetUnsyncedSyncMeta { reply: reply_tx }) {
            error!("Failed to query unsynced sync_meta: {}", e);
            return;
        }

        let unsynced = match reply_rx.await {
            Ok(entries) => entries,
            Err(_) => {
                error!("GetUnsyncedSyncMeta reply channel dropped");
                return;
            }
        };

        if unsynced.is_empty() {
            debug!("No unsynced __sync_meta entries for page={}", page_id);
            return;
        }

        // Delegate to subscribe_to_layers which handles consent + sending
        self.subscribe_to_layers(page_id, &unsynced, state).await;
    }

    /// Handle incoming LayerSubscribe from peer
    ///
    /// **Context**: Peer discovered a layer in __sync_meta, requests data + permit
    /// **We do**: Store consent permit, delegate to Scribe for permit + snapshot, encrypt, respond
    #[instrument(skip(self, state, consent_permit), fields(page_id = %page_id, layer = %layer_name))]
    pub(in crate::peer_actor) async fn on_layer_subscribe(
        &self,
        request_id: &str,
        page_id: &str,
        layer_name: &str,
        consent_permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("LayerSubscribe from unauthenticated peer: {}", self.node_id);
                return;
            }
        };

        info!(
            "LayerSubscribe: page={} layer={} from peer={}",
            page_id, layer_name, peer_did
        );

        // Store consent permit if provided
        if !consent_permit.is_empty() {
            let full_layer_name = format!("{}/{}", page_id, layer_name);
            if let Err(e) = state.butler.permits().consent().store(
                &peer_did,
                page_id,
                &full_layer_name,
                consent_permit,
            ) {
                warn!("Failed to store viewer layer consent: {}", e);
                // Non-fatal — continue processing
            } else {
                debug!(
                    "Stored viewer layer consent for peer={} layer={}",
                    peer_did, layer_name
                );
            }
        }

        // Get Scribe for this page
        let scribe = match state.page_subscriptions.get(page_id) {
            Some(sub) => sub.scribe.clone(),
            None => {
                warn!("LayerSubscribe: no page subscription for page={}", page_id);
                self.send_layer_subscribe_reject(
                    request_id,
                    page_id,
                    layer_name,
                    "page not subscribed",
                    state,
                )
                .await;
                return;
            }
        };

        // Delegate to Scribe: issues permit, adds subscriber, exports snapshot
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(ScribeMessage::HandleLayerSubscribe {
            layer_name: layer_name.to_string(),
            peer_did: peer_did.clone(),
            reply: reply_tx,
        }) {
            error!("Failed to send HandleLayerSubscribe to Scribe: {}", e);
            self.send_layer_subscribe_reject(
                request_id,
                page_id,
                layer_name,
                "internal error",
                state,
            )
            .await;
            return;
        }

        let (snapshot_data, state_vector, layer_permit) = match reply_rx.await {
            Ok(Ok(result)) => {
                Self::emit_layer_subscribe_protocol_capture(
                    &state.capture_tx,
                    "incoming",
                    page_id,
                    layer_name,
                    &peer_did,
                    "ok",
                    None,
                );
                result
            }
            Ok(Err(e)) => {
                warn!("HandleLayerSubscribe error for layer={}: {}", layer_name, e);
                Self::emit_layer_subscribe_protocol_capture(
                    &state.capture_tx,
                    "incoming",
                    page_id,
                    layer_name,
                    &peer_did,
                    "err",
                    Some(&e),
                );
                self.send_layer_subscribe_reject(request_id, page_id, layer_name, &e, state)
                    .await;
                return;
            }
            Err(_) => {
                error!(
                    "HandleLayerSubscribe reply dropped for layer={}",
                    layer_name
                );
                Self::emit_layer_subscribe_protocol_capture(
                    &state.capture_tx,
                    "incoming",
                    page_id,
                    layer_name,
                    &peer_did,
                    "err",
                    Some("reply dropped"),
                );
                self.send_layer_subscribe_reject(
                    request_id,
                    page_id,
                    layer_name,
                    "internal error",
                    state,
                )
                .await;
                return;
            }
        };

        // Encrypt snapshot for transfer
        let session_key = match state.session_key {
            Some(key) => key,
            None => {
                warn!("Cannot send LayerSubscribeAck: session key not set");
                self.send_layer_subscribe_reject(
                    request_id,
                    page_id,
                    layer_name,
                    "no session key",
                    state,
                )
                .await;
                return;
            }
        };

        let encrypted_data = match herald::encrypt_symmetric(&session_key, &snapshot_data) {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to encrypt LayerSubscribeAck data: {}", e);
                self.send_layer_subscribe_reject(
                    request_id,
                    page_id,
                    layer_name,
                    "encryption failed",
                    state,
                )
                .await;
                return;
            }
        };

        // Ensure peer is a per-layer subscriber for future broadcasts.
        // HandleLayerSubscribe attempts this internally, but as a belt-and-suspenders
        // we also send an explicit AuthorizeLayerSubscriber from here.
        scribe
            .cast(ScribeMessage::AuthorizeLayerSubscriber {
                layer_name: layer_name.to_string(),
                subscriber_did: peer_did.clone(),
            })
            .ok();

        // Note: Entry marking in __sync_meta happens on the requester side when they
        // receive this ack and apply the data (see handle_layer_permit_ack: MarkSyncMetaSynced).
        // For authority flow, StoreLayerAuthority in scribe already marks the creator's entry.

        let msg = Message::LayerSubscribeAck(LayerSubscribeAckMsg {
            request_id: request_id.to_string(),
            page_id: page_id.to_string(),
            layer_name: layer_name.to_string(),
            accepted: true,
            reason: None,
            data: encrypted_data,
            state_vector,
            layer_permit,
        });

        self.send_message(&msg, state).await;
        info!(
            "Sent LayerSubscribeAck for page={} layer={} to peer={}",
            page_id, layer_name, peer_did
        );
    }

    /// Send a rejected LayerSubscribeAck
    async fn send_layer_subscribe_reject(
        &self,
        request_id: &str,
        page_id: &str,
        layer_name: &str,
        reason: &str,
        state: &PeerActorState<C>,
    ) {
        let msg = Message::LayerSubscribeAck(LayerSubscribeAckMsg {
            request_id: request_id.to_string(),
            page_id: page_id.to_string(),
            layer_name: layer_name.to_string(),
            accepted: false,
            reason: Some(reason.to_string()),
            data: Vec::new(),
            state_vector: Vec::new(),
            layer_permit: String::new(),
        });
        self.send_message(&msg, state).await;
    }

    /// Handle incoming LayerSubscribeAck from peer
    ///
    /// **Context**: Peer processed our LayerSubscribe, sent data + permit
    /// **Permit-driven**: Detects permit type to determine handling:
    ///   - `layer_authority` → We're the node, creator sent authority. Store + fan out.
    ///   - `layer_permit` → We're the user, node sent access permit. Store + apply.
    #[instrument(skip(self, state, data, state_vector, layer_permit), fields(page_id = %page_id, layer = %layer_name))]
    pub(in crate::peer_actor) async fn on_layer_subscribe_ack(
        &self,
        _request_id: &str,
        page_id: &str,
        layer_name: &str,
        accepted: bool,
        reason: Option<&str>,
        data: &[u8],
        state_vector: &[u8],
        layer_permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!(
                    "LayerSubscribeAck from unauthenticated peer: {}",
                    self.node_id
                );
                return;
            }
        };

        if !accepted {
            let reject_reason = reason.unwrap_or("unknown");
            warn!(
                "LayerSubscribeAck rejected for page={} layer={}: {}",
                page_id, layer_name, reject_reason
            );
            Self::emit_layer_subscribe_protocol_capture(
                &state.capture_tx,
                "ack_rejected",
                page_id,
                layer_name,
                &peer_did,
                "err",
                Some(reject_reason),
            );
            return;
        }

        info!(
            "LayerSubscribeAck: page={} layer={} from peer={} data_len={}",
            page_id,
            layer_name,
            peer_did,
            data.len()
        );

        // Decrypt data
        let session_key = match state.session_key {
            Some(key) => key,
            None => {
                error!("Cannot decrypt LayerSubscribeAck: session key not set");
                return;
            }
        };

        let decrypted_data = match herald::decrypt_symmetric(&session_key, data) {
            Ok(plaintext) => plaintext,
            Err(e) => {
                error!("Failed to decrypt LayerSubscribeAck data: {}", e);
                return;
            }
        };

        // Detect permit type to determine handling path
        let is_authority = gurkha::Permit::from_token(layer_permit)
            .ok()
            .and_then(|p| p.get_fact("token_type").cloned())
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .map(|t| t == "layer_authority")
            .unwrap_or(false);

        if is_authority {
            self.handle_authority_ack(
                page_id,
                layer_name,
                &peer_did,
                layer_permit,
                &decrypted_data,
                state_vector,
                state,
            )
            .await;
        } else {
            self.handle_layer_permit_ack(
                page_id,
                layer_name,
                &peer_did,
                layer_permit,
                &decrypted_data,
                state,
            )
            .await;
        }
    }

    /// Handle LayerSubscribeAck containing a layer_authority permit (node received from creator)
    ///
    /// **Context**: We're the node, a creator responded with authority for a dynamic layer
    /// **We do**: Send StoreLayerAuthority to Scribe (stores authority, applies data, fans out)
    #[instrument(skip(self, state, data, state_vector), fields(page_id = %page_id, layer = %layer_name, creator = %creator_did))]
    async fn handle_authority_ack(
        &self,
        page_id: &str,
        layer_name: &str,
        creator_did: &str,
        authority_token: &str,
        data: &[u8],
        state_vector: &[u8],
        state: &mut PeerActorState<C>,
    ) {
        info!(
            "Received layer_authority for layer={} from creator={}",
            layer_name, creator_did
        );

        let scribe = match state.page_subscriptions.get(page_id) {
            Some(sub) => sub.scribe.clone(),
            None => {
                warn!(
                    "No page subscription for page={}, cannot store authority",
                    page_id
                );
                return;
            }
        };

        // Delegate to Scribe: store authority, apply data, fan out to authorized users
        if let Err(e) = scribe.cast(ScribeMessage::StoreLayerAuthority {
            layer_name: layer_name.to_string(),
            creator_did: creator_did.to_string(),
            authority_token: authority_token.to_string(),
            layer_data: data.to_vec(),
            state_vector: state_vector.to_vec(),
        }) {
            error!("Failed to send StoreLayerAuthority to Scribe: {}", e);
            return;
        }

        // Authorize creator as subscriber for bidirectional sync
        scribe
            .cast(ScribeMessage::AuthorizeLayerSubscriber {
                layer_name: layer_name.to_string(),
                subscriber_did: creator_did.to_string(),
            })
            .ok();

        // Note: StoreLayerAuthority in scribe already marks the creator's __sync_meta entry.

        info!(
            "Stored authority and initiated fanout for layer={}",
            layer_name
        );
    }

    /// Handle LayerSubscribeAck containing a layer_permit (user received from node)
    ///
    /// **Context**: We're the user, node responded with access permit for a dynamic layer
    /// **We do**: Store permit, apply data to Scribe, authorize node, mark synced
    #[instrument(skip(self, state, data), fields(page_id = %page_id, layer = %layer_name, node = %node_did))]
    async fn handle_layer_permit_ack(
        &self,
        page_id: &str,
        layer_name: &str,
        node_did: &str,
        layer_permit: &str,
        data: &[u8],
        state: &mut PeerActorState<C>,
    ) {
        info!(
            "Received layer_permit for layer={} from node={}",
            layer_name, node_did
        );

        // Store layer permit
        let full_layer_name = format!("{}/{}", page_id, layer_name);
        let our_did = match state.butler.get_identity().await {
            Ok(identity) => identity.did().to_string(),
            Err(e) => {
                error!("Failed to get our identity: {}", e);
                return;
            }
        };

        if let Err(e) =
            state
                .butler
                .permits()
                .layer()
                .store(page_id, &our_did, &full_layer_name, layer_permit)
        {
            error!("Failed to store layer permit: {}", e);
            return;
        }

        info!("Stored layer permit for layer={}", full_layer_name);

        // Apply data to Scribe
        if let Some(subscription) = state.page_subscriptions.get(page_id) {
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            if let Err(e) = subscription
                .scribe
                .cast(ScribeMessage::ApplyUpdateWithResult {
                    layer_name: layer_name.to_string(),
                    update: data.to_vec(),
                    from_peer: Some((node_did.to_string(), self.node_id.to_string())),
                    permit: Some(layer_permit.to_string()),
                    reply: reply_tx,
                })
            {
                error!("Failed to send ApplyUpdate for LayerSubscribeAck: {}", e);
                return;
            }

            match reply_rx.await {
                Ok(Ok(())) => {
                    info!("Applied LayerSubscribeAck data for layer={}", layer_name);
                }
                Ok(Err(e)) => {
                    warn!(
                        "Scribe rejected LayerSubscribeAck data for layer={}: {}",
                        layer_name, e
                    );
                }
                Err(_) => {
                    error!("ApplyUpdate reply dropped for layer={}", layer_name);
                    return;
                }
            }

            // Authorize node as subscriber for bidirectional sync
            subscription
                .scribe
                .cast(ScribeMessage::AuthorizeLayerSubscriber {
                    layer_name: layer_name.to_string(),
                    subscriber_did: node_did.to_string(),
                })
                .ok();
            info!("Authorized node {} on layer={}", node_did, layer_name);

            // Mark __sync_meta entry as synced
            subscription
                .scribe
                .cast(ScribeMessage::MarkSyncMetaSynced {
                    layer_name: layer_name.to_string(),
                })
                .ok();
        } else {
            warn!(
                "No page subscription for page={}, cannot apply LayerSubscribeAck",
                page_id
            );
        }
    }
}
