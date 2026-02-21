//! Page Subscription
//!
//! Handles subscribing to Scribe for live sync broadcasts and forwarding
//! broadcasts as SyncOffers to peers.

use tracing::{debug, error, info, instrument, warn};

use crate::message::*;
use crate::CourierMode;
use transport::Connection;

use super::super::guards::require_auth;
use super::super::{PageSubscription, PeerActor, PeerActorState, PeerMessage};

impl<C: Connection> PeerActor<C> {
    /// Subscribe to a page's Scribe for live sync broadcasts
    ///
    /// **Context**: After handshake, we subscribe to pages we want updates from
    /// **We do**: Open page, subscribe to Scribe, spawn listener task
    /// **Listener**: Forwards broadcasts as PeerMessage::BroadcastReceived
    #[instrument(skip(self, myself, state, permit), fields(page_id = %page_id))]
    pub(in crate::peer_actor) async fn subscribe_to_page(
        &self,
        myself: ractor::ActorRef<PeerMessage>,
        page_id: &str,
        permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        let (peer_did, _) = match require_auth(&state.state) {
            Ok(info) => (info.0.to_string(), info.1.to_string()),
            Err(_) => {
                warn!("Cannot subscribe to page before authentication");
                return;
            }
        };

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
                match state.butler.permits().page().get(page_id, &peer_did) {
                    Ok(Some(stored_permit)) => {
                        debug!(
                            "Node mode: Using stored page permit for peer {} (permit_len={})",
                            peer_did,
                            stored_permit.len()
                        );
                        stored_permit
                    }
                    _ => {
                        // No stored permit - try passed permit or local
                        if !permit.is_empty() {
                            debug!(
                                "Node mode: Using passed permit for peer {} (permit_len={})",
                                peer_did,
                                permit.len()
                            );
                            permit.to_string()
                        } else {
                            match state.butler.pages().get(page_id) {
                                Ok(Some(page_data)) => {
                                    debug!("Node mode: Using local permit for peer {} (no stored permit)", peer_did);
                                    page_data.permit.clone().unwrap_or_default()
                                }
                                _ => String::new(),
                            }
                        }
                    }
                }
            }
            CourierMode::User => {
                // User mode: Use our local permit - we control what we send
                match state.butler.pages().get(page_id) {
                    Ok(Some(page_data)) => {
                        debug!("User mode: Using local permit for subscription (our sync capabilities)");
                        page_data.permit.clone().unwrap_or_default()
                    }
                    _ => String::new(),
                }
            }
        };

        let (tx, mut rx) = tokio::sync::mpsc::channel::<butler::BroadcastPayload>(4096);
        let (eph_tx, mut eph_rx) = tokio::sync::mpsc::channel::<butler::EphemeralOutbound>(64);

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

        info!(
            "Subscribed to page {} for peer {} (with ephemeral channel)",
            page_id, peer_did
        );

        // Spawn listener task that forwards CRDT broadcasts to this actor
        let actor_ref = myself.clone();
        let page_id_clone = page_id.to_string();
        let listener_handle = tokio::spawn(async move {
            while let Some(payload) = rx.recv().await {
                if let Err(_) = actor_ref.cast(PeerMessage::BroadcastReceived(payload)) {
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
        let ephemeral_listener_handle = tokio::spawn(async move {
            debug!(page_id = %page_id_for_eph, node_id = %node_id_for_eph, "Ephemeral listener started");
            while let Some(outbound) = eph_rx.recv().await {
                debug!(
                    page_id = %outbound.page_id,
                    node_id = %node_id_for_eph,
                    payload_len = outbound.payload.len(),
                    "PeerActor received ephemeral from Scribe, sending datagram"
                );
                // Create protocol-level EphemeralDatagram
                let datagram = crate::message::EphemeralDatagram {
                    page_id: outbound.page_id,
                    payload: outbound.payload,
                };
                if let Ok(data) = datagram.to_bytes() {
                    // send_datagram is sync (Result, not Future)
                    match conn_for_eph.send_datagram(&data) {
                        Ok(()) => {
                            debug!(node_id = %node_id_for_eph, datagram_len = data.len(), "Ephemeral datagram sent");
                        }
                        Err(e) => {
                            warn!(node_id = %node_id_for_eph, error = %e, "Failed to send ephemeral datagram");
                        }
                    }
                } else {
                    warn!(node_id = %node_id_for_eph, "Failed to serialize ephemeral datagram");
                }
            }
            debug!(page_id = %page_id_for_eph, "Ephemeral listener stopped");
        });

        // Store full subscription info for cleanup on disconnect
        let subscription = PageSubscription {
            scribe: scribe.clone(),
            listener_handle,
            ephemeral_listener_handle,
            user_did: peer_did,
            device_id,
        };
        state
            .page_subscriptions
            .insert(page_id.to_string(), subscription);

        // Register direct Scribe connection and flush any buffered messages.
        // This implements the receiver-initiates pattern for reliable message delivery.
        self.flush_buffered_messages(page_id, &scribe, state, "SubscribeToPage");
        state
            .scribe_connections
            .insert(page_id.to_string(), scribe.clone());
    }

    /// Subscribe this peer to all active Scribes they have access to
    ///
    /// **Context**: Called after handshake completes to auto-subscribe to open pages
    /// **We do**: Query Butler for active Scribes, subscribe to each
    #[instrument(skip(self, myself, state))]
    pub(in crate::peer_actor) async fn subscribe_to_active_scribes(
        &self,
        myself: ractor::ActorRef<PeerMessage>,
        state: &mut PeerActorState<C>,
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
            peer_did,
            active_pages.len()
        );

        for (page_id, permit) in active_pages {
            self.subscribe_to_page(myself.clone(), &page_id, &permit, state)
                .await;
        }
    }

    /// Check subscription health and resubscribe to dead listeners
    ///
    /// **Context**: Periodic health check or after reconnection
    /// **We do**: Check if listener tasks are finished, resubscribe if dead
    /// **Design**: Fixes subscription loss after connection issues
    #[instrument(skip(self, myself, state))]
    pub(in crate::peer_actor) async fn check_subscription_health(
        &self,
        myself: ractor::ActorRef<PeerMessage>,
        state: &mut PeerActorState<C>,
    ) {
        let dead_subscriptions: Vec<String> = state
            .page_subscriptions
            .iter()
            .filter_map(|(page_id, sub)| {
                // Check if either listener has died
                if sub.listener_handle.is_finished() || sub.ephemeral_listener_handle.is_finished()
                {
                    warn!(page_id = %page_id, "Subscription listener died, will resubscribe");
                    Some(page_id.clone())
                } else {
                    None
                }
            })
            .collect();

        // Resubscribe to dead subscriptions
        for page_id in dead_subscriptions {
            if let Some(sub) = state.page_subscriptions.remove(&page_id) {
                sub.listener_handle.abort();
                sub.ephemeral_listener_handle.abort();

                let _ = sub.scribe.cast(butler::ScribeMessage::Unsubscribe {
                    user_did: sub.user_did,
                    device_id: sub.device_id,
                });
            }

            state.scribe_connections.remove(&page_id);

            self.subscribe_to_page(myself.clone(), &page_id, "", state)
                .await;
        }
    }

    /// Handle broadcast received from Scribe - encrypt and send as SyncOffer
    ///
    /// **Context**: Scribe sent us an update to forward to this peer
    /// **We do**: Encrypt with session key, send as SyncOffer
    /// **3-Step**: This initiates the sync protocol; we wait for SyncAccept
    #[instrument(skip(self, state, payload), fields(page_id = %payload.page_id, layer_name = %payload.layer_name))]
    pub(in crate::peer_actor) async fn handle_broadcast_received(
        &self,
        payload: butler::BroadcastPayload,
        state: &mut PeerActorState<C>,
    ) {
        let peer_did = match require_auth(&state.state) {
            Ok((did, _)) => did.to_string(),
            Err(_) => {
                warn!("Cannot send SyncOffer: peer not authenticated");
                return;
            }
        };

        // Need session key
        let session_key = match state.session_key {
            Some(key) => key,
            None => {
                warn!("Cannot send SyncOffer: session key not set");
                return;
            }
        };

        // Encrypt update using session key
        let encrypted_data = match herald::encrypt_symmetric(&session_key, &payload.update) {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to encrypt update for SyncOffer: {}", e);
                return;
            }
        };

        // Fire-and-forget: Don't track pending sync offers for real-time broadcasts.
        // Trust CRDTs to converge. This avoids false divergence detection when
        // rapid writes cause state vectors to advance before SyncAccept arrives.

        let full_layer_name = if payload
            .layer_name
            .starts_with(&format!("{}/", payload.page_id))
        {
            payload.layer_name.clone()
        } else {
            format!("{}/{}", payload.page_id, payload.layer_name)
        };

        let cache_key = (
            payload.page_id.clone(),
            full_layer_name.clone(),
            peer_did.clone(),
        );
        let authority_permit = if let Some(cached) = state.authority_permit_cache.get(&cache_key) {
            cached.clone()
        } else {
            let result = match state.butler.permits().authority().get(
                &payload.page_id,
                &full_layer_name,
                &peer_did,
            ) {
                Ok(Some((_version, permit))) => Some(permit),
                Ok(None) => None,
                Err(e) => {
                    warn!(
                        page_id = %payload.page_id,
                        layer_name = %full_layer_name,
                        audience = %peer_did,
                        error = %e,
                        "Failed to look up layer authority permit for SyncOffer"
                    );
                    None
                }
            };
            state
                .authority_permit_cache
                .insert(cache_key, result.clone());
            result
        };

        // Send SyncOffer with our state vector
        let msg = Message::SyncOffer(SyncOfferMsg {
            page_id: payload.page_id,
            layer_name: payload.layer_name.clone(),
            layer_type: LayerType::from_layer_name(&payload.layer_name),
            data: encrypted_data,
            state_vector: payload.state_vector,
            authority_permit,
        });

        self.send_message(&msg, state).await;
        info!(
            "Sent SyncOffer for layer {} to peer {}",
            payload.layer_name, self.node_id
        );
    }
}
