//! PeerActor - One actor per P2P connection
//!
//! Handles:
//! - Handshake state machine (Hello/Welcome/PermitGrant/Ack)
//! - Protocol message routing
//! - Per-peer state tracking
//!
//! # Handshake Flow (First Connection)
//!
//! ```text
//! Owner (User mode)                    Node (Node mode)
//!     │                                      │
//!     │──── Hello (first_connection) ───────>│
//!     │                                      │ validate permit
//!     │                                      │ store OwnerInfo
//!     │                                      │ issue permit_for_owner
//!     │<──────────── Welcome ────────────────│
//!     │ validate permit                      │
//!     │ store permit                         │
//!     │ issue permit_for_node                │
//!     │──────────── PermitGrant ────────────>│
//!     │                                      │ store permit
//!     │<──────────── Ack ────────────────────│
//!     │                                      │
//!   [Authenticated]                    [Authenticated]
//! ```

mod guards;
mod handshake;
mod publish;
mod sync;

use std::sync::Arc;

use logging_utils::short;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tracing::{debug, error, info, warn, instrument};
use transport::{ConnectionHandle, NodeId};

use crate::coordinator::{CoordinatorMessage, CourierMode};
use crate::message::Message;
use crate::state::{PeerState, PeerType};
use butler::{Butler, BroadcastPayload};

/// Messages received by PeerActor
#[derive(Debug)]
pub enum PeerMessage {
    /// Protocol message from Coordinator (already deserialized)
    Protocol(Message),

    /// Internal: Initiate handshake by sending Hello (User mode)
    InitiateHandshake {
        permit: String,
    },

    /// Internal: Publish a space to this peer (User mode)
    PublishSpace {
        space_id: String,
    },

    /// Internal: Publish a page to this peer (User mode)
    PublishPage {
        page_id: String,
    },

    /// Internal: Request shareable link for a space (User mode)
    ///
    /// **Context**: Owner wants to share a space with a viewer.
    /// **We do**: Send GetShareableLinkRequest to node.
    GetShareableLink {
        space_id: String,
    },

    /// Internal: Request space content as viewer (User mode)
    ///
    /// **Context**: Viewer has aud:* permit from shareable link.
    /// **We do**: Send SpaceRequest to node with permit.
    RequestSpace {
        space_id: String,
        viewer_permit: String,
    },

    /// Internal: Subscribe to a page's Scribe for live updates
    ///
    /// **Context**: After handshake, we want live updates for pages we have access to
    /// **We do**: Subscribe to Scribe, spawn broadcast listener task
    SubscribeToPage {
        page_id: String,
        permit: String,
    },

    /// Internal: Broadcast payload received from Scribe
    ///
    /// **Context**: Scribe sent us an update to forward to peer
    /// **We do**: Encrypt and send as SyncOffer
    BroadcastReceived(BroadcastPayload),

    /// Internal: Issue sync consent permits and send to node (User/Viewer mode)
    ///
    /// **Context**: Viewer received space + pages, now issues consent permits
    /// **We do**: Issue consent permits via gurkha, send SyncConsentGrant to node
    IssueSyncConsent {
        space_id: String,
        space_template: String,
        page_template: String,
    },

    /// Internal: Refresh subscriptions to active Scribes
    ///
    /// **Context**: Page opened after connection established
    /// **We do**: Re-run subscribe_to_active_scribes to pick up new pages
    RefreshSubscriptions,
}

/// Arguments for spawning PeerActor
pub struct PeerActorArgs {
    pub mode: CourierMode,
    pub conn: ConnectionHandle,
    pub coordinator: ActorRef<CoordinatorMessage>,
    pub butler: Arc<Butler>,
}

/// Pending viewer sync context (for streaming pages after SpaceDataAck)
#[derive(Debug, Clone)]
pub struct PendingViewerSync {
    /// Viewer's DID
    pub viewer_did: String,
    /// Viewer's public key (base64)
    pub viewer_pubkey_b64: String,
    /// Viewer's encryption key for transit encryption
    pub viewer_encryption_key: [u8; 32],
    /// Page IDs to stream
    pub page_ids: Vec<String>,
}

/// Context for pending SyncOffer operations (waiting for SyncAccept)
///
/// When we send a SyncOffer, we track the context so that when we receive
/// SyncAccept, we can compare state vectors and decide: SyncAck or resync.
#[derive(Debug, Clone)]
pub struct PendingSyncOffer {
    /// Our state vector at time of sending SyncOffer
    pub our_state_vector: Vec<u8>,
    /// The consent permit we used
    pub permit: String,
}

/// Active subscription to a page's Scribe
///
/// Stores all info needed to cleanly unsubscribe on disconnect.
pub struct PageSubscription {
    /// Scribe actor ref for sending Unsubscribe
    pub scribe: ActorRef<butler::ScribeMessage>,
    /// Listener task handle (aborted on cleanup)
    pub listener_handle: tokio::task::JoinHandle<()>,
    /// Subscriber's DID (for Unsubscribe message)
    pub user_did: String,
    /// Subscriber's device ID (for Unsubscribe message)
    pub device_id: String,
}

/// Actor state for PeerActor
pub struct PeerActorState {
    /// Our node's mode (User or Node)
    mode: CourierMode,
    /// Current handshake state
    state: PeerState,
    /// Connection handle for sending messages
    conn: ConnectionHandle,
    /// Reference to coordinator for callbacks
    coordinator: ActorRef<CoordinatorMessage>,
    /// Butler for storage operations
    butler: Arc<Butler>,
    /// Pending viewer syncs by request_id (Node mode only)
    /// Stored when we send SpaceData, used when we receive SpaceDataAck
    pending_viewer_syncs: std::collections::HashMap<String, PendingViewerSync>,
    /// Peer's encryption key (set after handshake)
    /// Used for ECDH when sending SyncOffer messages
    peer_encryption_key: Option<[u8; 32]>,
    /// Active page subscriptions: page_id -> subscription info
    /// Used for explicit cleanup on disconnect (abort task + send Unsubscribe)
    page_subscriptions: std::collections::HashMap<String, PageSubscription>,
    /// Pending SyncOffer operations: (page_id, layer_name) -> context
    /// Tracks outgoing SyncOffers waiting for SyncAccept response
    pending_sync_offers: std::collections::HashMap<(String, String), PendingSyncOffer>,
}

/// PeerActor handles one P2P connection
pub struct PeerActor {
    node_id: NodeId,
}

impl PeerActor {
    pub fn new(node_id: NodeId) -> Self {
        Self { node_id }
    }

    /// Send a message to the peer
    async fn send_message(&self, msg: &Message, state: &PeerActorState) {
        let bytes = match msg.to_bytes() {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to serialize message: {}", e);
                return;
            }
        };

        if let Err(e) = state.conn.send_bytes(&bytes).await {
            error!("Failed to send message to {}: {}", self.node_id, e);
        }
    }

    /// Send rejection and transition to failed state
    async fn reject(&self, reason: &str, state: &mut PeerActorState) {
        warn!("Rejecting peer {}: {}", self.node_id, reason);
        self.send_message(&Message::Rejected { reason: reason.to_string() }, state).await;
        state.state = PeerState::fail(reason);
        self.notify_failed(state, reason);
    }

    /// Notify coordinator of authentication
    fn notify_authenticated(&self, state: &PeerActorState, peer_type: PeerType, did: &str, username: &str) {
        let _ = state.coordinator.cast(CoordinatorMessage::PeerAuthenticated {
            node_id: self.node_id,
            peer_type,
            did: did.to_string(),
            username: username.to_string(),
        });
    }

    /// Notify coordinator of failure
    fn notify_failed(&self, state: &PeerActorState, reason: &str) {
        let _ = state.coordinator.cast(CoordinatorMessage::PeerFailed {
            node_id: self.node_id,
            reason: reason.to_string(),
        });
    }
}

#[cfg_attr(feature = "async-trait", ractor::async_trait)]
impl Actor for PeerActor {
    type Msg = PeerMessage;
    type State = PeerActorState;
    type Arguments = PeerActorArgs;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        info!("PeerActor started for {} in {:?} mode", self.node_id, args.mode);

        Ok(PeerActorState {
            mode: args.mode,
            state: PeerState::Connected,
            conn: args.conn,
            coordinator: args.coordinator,
            butler: args.butler,
            pending_viewer_syncs: std::collections::HashMap::new(),
            peer_encryption_key: None,
            page_subscriptions: std::collections::HashMap::new(),
            pending_sync_offers: std::collections::HashMap::new(),
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            PeerMessage::Protocol(msg) => {
                self.handle_protocol_message(myself, msg, state).await;
            }

            PeerMessage::InitiateHandshake { permit } => {
                self.initiate_handshake(&permit, state).await;
            }

            PeerMessage::PublishSpace { space_id } => {
                self.initiate_publish_space(&space_id, state).await;
            }

            PeerMessage::PublishPage { page_id } => {
                self.initiate_publish_page(&page_id, state).await;
            }

            PeerMessage::GetShareableLink { space_id } => {
                self.initiate_get_shareable_link(&space_id, state).await;
            }

            PeerMessage::RequestSpace { space_id, viewer_permit } => {
                self.initiate_request_space_as_viewer(&space_id, &viewer_permit, state).await;
            }

            PeerMessage::SubscribeToPage { page_id, permit } => {
                self.subscribe_to_page(myself, &page_id, &permit, state).await;
            }

            PeerMessage::BroadcastReceived(payload) => {
                self.handle_broadcast_received(payload, state).await;
            }

            PeerMessage::IssueSyncConsent { space_id, space_template, page_template } => {
                self.issue_sync_consent(&space_id, &space_template, &page_template, state).await;
            }

            PeerMessage::RefreshSubscriptions => {
                self.subscribe_to_active_scribes(myself, state).await;
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let subscription_count = state.page_subscriptions.len();

        // Clean up all page subscriptions
        for (page_id, subscription) in state.page_subscriptions.drain() {
            // Abort the listener task
            subscription.listener_handle.abort();

            // Send Unsubscribe to Scribe
            if let Err(e) = subscription.scribe.cast(butler::ScribeMessage::Unsubscribe {
                user_did: subscription.user_did.clone(),
                device_id: subscription.device_id.clone(),
            }) {
                warn!("Failed to send Unsubscribe for page {}: {}", page_id, e);
            } else {
                debug!("Unsubscribed from page {} (user: {}, device: {})",
                    page_id, subscription.user_did, subscription.device_id);
            }
        }

        info!("PeerActor stopped for {} ({} subscriptions cleaned up)",
            self.node_id, subscription_count);
        Ok(())
    }
}

impl PeerActor {
    /// Handle protocol message
    #[instrument(skip(self, myself, message, state), fields(node = %short(&self.node_id), peer_state = %state.state.name(), msg = %message.name()))]
    async fn handle_protocol_message(
        &self,
        myself: ActorRef<PeerMessage>,
        message: Message,
        state: &mut PeerActorState,
    ) {
        // Message name logged in instrument fields above

        match message {
            Message::Hello {
                did,
                username,
                public_key,
                encryption_key,
                signature: _,
                timestamp: _,
                permit,
            } => {
                self.on_hello(&did, &username, &public_key, &encryption_key, &permit, state).await;
            }

            Message::Welcome {
                node_id: _,
                node_public_key,
                node_encryption_key,
                signature: _,
                timestamp: _,
                permit_for_peer,
            } => {
                self.on_welcome(myself.clone(), &permit_for_peer, &node_public_key, &node_encryption_key, state).await;
            }

            Message::PermitGrant { permit_for_node } => {
                self.on_permit_grant(myself.clone(), &permit_for_node, state).await;
            }

            Message::Ack => {
                debug!("Received Ack from {}", self.node_id);
                self.on_ack(myself.clone(), state).await;
            }

            Message::Rejected { reason } => {
                warn!("Connection rejected by {}: {}", self.node_id, reason);
                state.state = PeerState::fail(&reason);
                self.notify_failed(state, &reason);
            }

            // Publishing messages - require authentication
            Message::PublishSpace { request_id, space, space_permit } => {
                self.on_publish_space(&request_id, &space, &space_permit, state).await;
            }

            Message::PublishPage { request_id, page, page_permit, owner_permit, ephemeral_public, layers } => {
                self.on_publish_page(&request_id, &page, &page_permit, &owner_permit, &ephemeral_public, &layers, state).await;
            }

            Message::PublishPageAck { request_id, page_id, permit } => {
                self.on_publish_page_ack(&request_id, &page_id, &permit, state).await;
            }

            Message::PublishSpaceAck { request_id, permit, pages } => {
                self.on_publish_space_ack(&request_id, &permit, &pages, state).await;
            }

            Message::PublishError { request_id, error } => {
                self.on_publish_error(&request_id, &error, state).await;
            }

            // 3-Step Sync Protocol messages
            Message::SyncOffer { page_id, layer_name, data, state_vector, ephemeral_public, permit } => {
                self.on_sync_offer(myself.clone(), &page_id, &layer_name, &data, &state_vector, &ephemeral_public, &permit, state).await;
            }

            Message::SyncAccept { page_id, layer_name, state_vector } => {
                self.on_sync_accept(&page_id, &layer_name, &state_vector, state).await;
            }

            Message::SyncAck { page_id, layer_name, state_vector } => {
                self.on_sync_ack(&page_id, &layer_name, &state_vector, state).await;
            }

            // Shareable link messages
            Message::GetShareableLinkRequest { request_id, space_id } => {
                self.on_get_shareable_link_request(&request_id, &space_id, state).await;
            }

            Message::GetShareableLinkResponse { request_id, space_id, permit } => {
                self.on_get_shareable_link_response(&request_id, &space_id, &permit, state).await;
            }

            // Viewer space request messages
            Message::SpaceRequest { request_id, space_id, viewer_did, viewer_public_key, viewer_encryption_key, viewer_permit } => {
                self.on_space_request(&request_id, &space_id, &viewer_did, &viewer_public_key, &viewer_encryption_key, &viewer_permit, state).await;
            }

            Message::SpaceData { request_id, space_id, delegated_permit, space, page_ids } => {
                self.on_space_data(&request_id, &space_id, &delegated_permit, &space, &page_ids, state).await;
            }

            Message::SpaceDataAck { request_id, space_id, delegated_permit } => {
                self.on_space_data_ack(&request_id, &space_id, &delegated_permit, state).await;
            }

            Message::PageData { request_id, space_id, meta, permit, ephemeral_public, layers, is_last } => {
                self.on_viewer_page(&request_id, &space_id, &meta, &permit, &ephemeral_public, &layers, is_last, state).await;
            }

            Message::SpaceRequestError { request_id, error } => {
                error!("SpaceRequestError: request={} error={}", request_id, error);
            }

            // Sync consent messages (Viewer → Node → Viewer)
            Message::SyncConsentGrant { request_id, space_id, space_consent_permit, page_consent_permits } => {
                self.on_sync_consent_grant(&request_id, &space_id, &space_consent_permit, &page_consent_permits, state).await;
            }

            Message::SyncConsentAck { request_id, space_id } => {
                self.on_sync_consent_ack(&request_id, &space_id, state).await;
            }

            Message::Error { id, code, message } => {
                error!(
                    "Error from {}: {:?} - {} (id: {:?})",
                    self.node_id, code, message, id
                );
            }
        }
    }
}
