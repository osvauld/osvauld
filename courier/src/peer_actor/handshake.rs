//! Handshake handlers for peer authentication
//!
//! Flow:
//! 1. User sends Hello → Node
//! 2. Node validates (via decision), sends Welcome → User
//! 3. User validates Welcome (via decision), sends PermitGrant → Node
//! 4. Node validates PermitGrant (via decision), sends Ack → User
//! 5. Both sides authenticated
//!
//! ## Design (Capability-Based)
//!
//! The protocol is completely role-agnostic. All handshake decisions are based on
//! capabilities derived directly from the permit structure:
//!
//! - `accept_publish: true` → Peer can publish spaces to this node
//! - `first_connection: true` → First time connecting (needs OwnerInfo stored)
//!
//! Security model (capability-based decisions):
//! - Node decides how to respond to Hello based on capabilities
//! - User validates Welcome permit audience
//! - Node validates PermitGrant permit from user

use base64::{engine::general_purpose::STANDARD, Engine};
use tracing::{debug, error, info, warn, instrument};

use herald::Identity;
use crate::handshake::{
    HelloDecision, WelcomeDecision, PermitGrantDecision,
    HelloContext, WelcomeContext, PermitGrantContext,
    decide_hello_response, decide_welcome_response, decide_permit_grant_response,
    can_publish,
};

use crate::message::*;
use crate::state::{PeerState, PeerType};
use butler::OwnerInfo;

use transport::Connection;

use super::guards::{require_node_mode, require_user_mode, parse_permit};
use super::{PeerActor, PeerActorState};

impl<C: Connection> PeerActor<C> {

    /// Initiate handshake by sending Hello (User mode only)
    ///
    /// **Context**: User connects to Node and starts handshake
    /// **We send**: Hello with our identity and permit
    /// **Next**: Wait for Welcome
    #[instrument(skip(self, state, permit))]
    pub(super) async fn initiate_handshake(&self, permit: &str, state: &mut PeerActorState<C>) {
        if require_user_mode(state.mode, "initiate_handshake").is_some() {
            return;
        }

        if !matches!(state.state, PeerState::Connected) {
            warn!("Cannot initiate handshake in state {}", state.state.name());
            return;
        }

        // Parse the permit to extract capabilities
        let parsed_permit = match parse_permit(permit) {
            Ok(p) => p,
            Err(e) => {
                error!("Invalid permit for handshake: {}", e);
                state.state = PeerState::fail("Invalid permit");
                return;
            }
        };

        // Extract capability from permit
        let our_can_publish = can_publish(&parsed_permit);
        debug!(can_publish = our_can_publish, "Extracted capabilities from permit");

        // Get our identity from Butler
        let user_info = match state.butler.user_info().await {
            Ok(info) => info,
            Err(e) => {
                error!("Failed to get user info: {}", e);
                state.state = PeerState::fail("No identity");
                return;
            }
        };

        // Get expected node verifying key from sovereign node record
        // We compare against the verifying key (extracted from DID), not the device key
        let node_id_str = self.node_id.to_string();
        let expected_node_pubkey = match state.butler.nodes().get(&node_id_str) {
            Ok(Some(sovereign_node)) => {
                // Extract the 32-byte public key from DID format
                // DID is "did:key:z6Mk..." - we need the raw bytes for comparison
                match Identity::public_key_from_did(&sovereign_node.did) {
                    Ok(bytes) => bytes.to_vec(),
                    Err(e) => {
                        error!("Failed to extract pubkey from node DID: {}", e);
                        state.state = PeerState::fail("Invalid node public key");
                        return;
                    }
                }
            }
            Ok(None) => {
                // Peer flow - might not have sovereign node record
                // Use empty vec, validation will be done differently
                warn!("No sovereign node record, proceeding without node pubkey validation");
                vec![]
            }
            Err(e) => {
                error!("Failed to get sovereign node: {}", e);
                state.state = PeerState::fail("Failed to get node info");
                return;
            }
        };

        let timestamp = chrono::Utc::now().timestamp();

        // TODO: Sign (did + timestamp) with signing key
        let signature = vec![];

        // Convert public keys to [u8; 32] arrays
        let public_key: [u8; 32] = user_info.public_key.clone().try_into()
            .expect("public_key should be 32 bytes");
        let encryption_key: [u8; 32] = user_info.encryption_key.clone().try_into()
            .expect("encryption_key should be 32 bytes");

        let hello = Message::Hello(HelloMsg {
            protocol_version: PROTOCOL_VERSION,
            did: user_info.did.clone(),
            username: user_info.username.clone(),
            public_key,
            encryption_key,
            signature,
            timestamp,
            permit: permit.to_string(),
        });

        info!(can_publish = our_can_publish, "Sending Hello to {} as {}", self.node_id, user_info.username);
        self.send_message(&hello, state).await;

        // Transition to AwaitingWelcome with capability tracking
        state.state = PeerState::AwaitingWelcome {
            our_did: user_info.did,
            our_username: user_info.username,
            sent_at: timestamp,
            can_publish: our_can_publish,
            expected_node_pubkey,
            our_permit: permit.to_string(),
        };
    }

    /// Handle Hello message (Node mode receives this)
    ///
    /// **Context**: Peer connects to us (the Node)
    /// **Peer sends**: Hello with identity and permit
    /// **We call**: decide_hello_response() for decision
    /// **We execute**: Accept (store info, issue permit) or Reject
    /// **We send**: Welcome with permit_for_peer
    #[instrument(skip(self, state, public_key, encryption_key, permit), fields(peer_did = %did, peer_username = %username))]
    pub(super) async fn on_hello(
        &self,
        did: &str,
        username: &str,
        public_key: &[u8; 32],
        encryption_key: &[u8; 32],
        permit: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_node_mode(state.mode, "on_hello").is_some() {
            return;
        }

        if !matches!(state.state, PeerState::Connected) {
            warn!("Unexpected Hello in state {}", state.state.name());
            return;
        }

        // Store peer's encryption key for ECDH during SyncOffer
        state.peer_encryption_key = Some(*encryption_key);

        info!("Received Hello from {} ({})", username, did);

        // Parse and validate permit, cache for later use
        let parsed_permit = match parse_permit(permit) {
            Ok(p) => p,
            Err(e) => {
                self.reject(&e, state).await;
                return;
            }
        };
        state.cached_peer_permit = Some(parsed_permit.clone());

        let peer_can_publish = can_publish(&parsed_permit);
        let is_first_connection = parsed_permit.is_first_connection();
        debug!(can_publish = peer_can_publish, is_first_connection, "Permit capabilities parsed");

        // Check if we already have an owner
        let existing_owner = match state.butler.nodes().get_owner() {
            Ok(owner) => owner,
            Err(e) => {
                error!("Failed to check owner: {}", e);
                self.reject("Internal error", state).await;
                return;
            }
        };

        // Build context for decision
        let ctx = HelloContext {
            incoming_permit: &parsed_permit,
            incoming_did: did,
            existing_owner_did: existing_owner.as_ref().map(|o| o.did.as_str()),
            existing_owner_permit: existing_owner.as_ref().and_then(|o| o.permit.as_deref()),
        };

        // Get decision
        let decision = decide_hello_response(&ctx);
        debug!(?decision, "Hello decision");

        // Execute decision
        match decision {
            HelloDecision::AcceptFirstConnection { can_publish } => {
                self.handle_first_connection(did, username, public_key, encryption_key, can_publish, state).await;
            }
            HelloDecision::AcceptReconnection { stored_permit, can_publish } => {
                self.handle_reconnection(did, username, &stored_permit, can_publish, state).await;
            }
            HelloDecision::AcceptPeer => {
                self.handle_peer_connection(did, username, public_key, encryption_key, state).await;
            }
            HelloDecision::RejectAlreadyHasOwner => {
                self.reject("Node already has an owner", state).await;
            }
            HelloDecision::RejectOwnerMismatch => {
                self.reject("Owner DID mismatch", state).await;
            }
            HelloDecision::RejectNoOwnerForReconnection => {
                self.reject("No owner registered for reconnection", state).await;
            }
        }
    }

    /// Handle first connection from peer who can publish (Node mode)
    ///
    /// **Context**: New peer connecting for the first time with publish capability
    /// **We store**: OwnerInfo (if can_publish)
    /// **We issue**: peer_connection permit
    /// **We send**: Welcome
    #[instrument(skip_all, fields(peer_did = %did, peer_username = %username, can_publish = peer_can_publish))]
    async fn handle_first_connection(
        &self,
        did: &str,
        username: &str,
        public_key: &[u8; 32],
        encryption_key: &[u8; 32],
        peer_can_publish: bool,
        state: &mut PeerActorState<C>,
    ) {
        // Store owner info if peer can publish
        if peer_can_publish {
            let encryption_key_base64 = STANDARD.encode(encryption_key);
            let owner_info = OwnerInfo::new(
                did.to_string(),
                encryption_key_base64,
                username.to_string(),
            );

            if let Err(e) = state.butler.nodes().set_owner(&owner_info) {
                error!("Failed to store owner info: {}", e);
                self.reject("Failed to store owner", state).await;
                return;
            }

            info!("Stored owner info for first connection");
        }

        // Issue new permit for the peer
        // Relationship determines permit type - gurkha expects node_owner or node_viewer
        let relationship = if peer_can_publish { "node_owner" } else { "node_viewer" };
        let peer_pubkey = STANDARD.encode(public_key);
        let (permit_for_peer, _our_pubkey) = match state.butler.issue_peer_connection_permit(&peer_pubkey, relationship).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to issue permit: {}", e);
                self.reject("Failed to issue permit", state).await;
                return;
            }
        };

        // Store the permit we issued (for reconnection)
        if peer_can_publish {
            if let Err(e) = state.butler.nodes().set_owner_permit(permit_for_peer.clone()) {
                warn!("Failed to store owner permit: {}", e);
            }
        }

        // Send Welcome
        self.send_welcome_message(&permit_for_peer, state).await;

        // Transition to AwaitingPermitGrant
        state.state = PeerState::AwaitingPermitGrant {
            their_did: did.to_string(),
            their_username: username.to_string(),
            is_first_connection: true,
            can_publish: peer_can_publish,
        };
    }

    /// Handle reconnection from known peer (Node mode)
    ///
    /// **Context**: Known peer reconnecting
    /// **We send**: Welcome with stored permit
    #[instrument(skip_all, fields(peer_did = %did, peer_username = %username, can_publish = peer_can_publish))]
    async fn handle_reconnection(
        &self,
        did: &str,
        username: &str,
        stored_permit: &str,
        peer_can_publish: bool,
        state: &mut PeerActorState<C>,
    ) {
        debug!(can_publish = peer_can_publish, "Reconnection from known peer");
        if peer_can_publish {
            let _ = state.butler.nodes().update_owner_last_connected();
        }

        // Send Welcome with stored permit
        self.send_welcome_message(stored_permit, state).await;

        // Transition to AwaitingPermitGrant (waiting for Ack)
        state.state = PeerState::AwaitingPermitGrant {
            their_did: did.to_string(),
            their_username: username.to_string(),
            is_first_connection: false,
            can_publish: peer_can_publish,
        };
    }

    /// Handle peer connection (Node mode)
    ///
    /// **Context**: Peer connecting without publish capability
    /// **We store**: Contact record with peer's device (NodeId)
    /// **We issue**: peer_connection permit
    /// **We send**: Welcome
    #[instrument(skip_all, fields(peer_did = %did, peer_username = %username))]
    async fn handle_peer_connection(
        &self,
        did: &str,
        username: &str,
        public_key: &[u8; 32],
        encryption_key: &[u8; 32],
        state: &mut PeerActorState<C>,
    ) {
        info!("Accepting peer connection from {} ({})", username, did);

        // Issue permit for peer - gurkha expects node_viewer for non-publishing peers
        let peer_pubkey = STANDARD.encode(public_key);
        let (permit_for_peer, _our_pubkey) = match state.butler.issue_peer_connection_permit(&peer_pubkey, "node_viewer").await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to issue peer permit: {}", e);
                self.reject("Failed to issue permit", state).await;
                return;
            }
        };

        // Store peer as User contact with their device (NodeId)
        let encryption_key_base64 = STANDARD.encode(encryption_key);
        let mut contact = butler::ContactData::new(
            did.to_string(),
            encryption_key_base64,
            username.to_string(),
        );
        contact.add_device(self.node_id.to_string(), "default".to_string());

        if let Err(e) = state.butler.contacts().upsert(&contact) {
            warn!("Failed to store peer contact: {}", e);
        } else {
            debug!(did = %did, device_id = %self.node_id, "Stored peer contact with device");
        }

        // Send Welcome
        self.send_welcome_message(&permit_for_peer, state).await;

        // Transition to AwaitingPermitGrant
        state.state = PeerState::AwaitingPermitGrant {
            their_did: did.to_string(),
            their_username: username.to_string(),
            is_first_connection: true,
            can_publish: false,
        };
    }

    /// Send Welcome message with our identity and permit for peer
    #[instrument(skip_all, fields(node = %self.node_id))]
    async fn send_welcome_message(&self, permit_for_peer: &str, state: &mut PeerActorState<C>) {
        let our_identity = match state.butler.get_identity().await {
            Ok(identity) => identity,
            Err(e) => {
                error!("Failed to get identity for Welcome: {}", e);
                state.state = PeerState::fail("Internal error");
                return;
            }
        };

        let node_public_key: [u8; 32] = our_identity.public_signing_key().try_into()
            .expect("public_signing_key should be 32 bytes");
        let node_encryption_key: [u8; 32] = our_identity.public_encryption_key().try_into()
            .expect("public_encryption_key should be 32 bytes");

        let timestamp = chrono::Utc::now().timestamp();
        let signature = vec![]; // TODO: Sign

        let welcome = Message::Welcome(WelcomeMsg {
            protocol_version: PROTOCOL_VERSION,
            node_id: self.node_id.to_string(),
            node_public_key,
            node_encryption_key,
            signature,
            timestamp,
            permit_for_peer: permit_for_peer.to_string(),
        });

        info!("Sending Welcome");
        self.send_message(&welcome, state).await;
    }

    /// Handle Welcome message (User mode receives this)
    ///
    /// **Context**: Node responded to our Hello
    /// **We call**: decide_welcome_response() for decision
    /// **We validate**: Node identity, permit audience
    /// **We issue**: Reciprocal permit
    /// **We send**: PermitGrant
    #[instrument(skip(self, myself, state, permit_for_us))]
    pub(super) async fn on_welcome(
        &self,
        myself: ractor::ActorRef<super::PeerMessage>,
        permit_for_us: &str,
        node_public_key: &[u8; 32],
        node_encryption_key: &[u8; 32],
        state: &mut PeerActorState<C>,
    ) {
        if require_user_mode(state.mode, "on_welcome").is_some() {
            return;
        }

        // Extract state data
        let (our_did, our_username, _our_can_publish, expected_node_pubkey, our_permit) = match &state.state {
            PeerState::AwaitingWelcome { our_did, our_username, can_publish, expected_node_pubkey, our_permit, .. } => {
                (our_did.clone(), our_username.clone(), *can_publish, expected_node_pubkey.clone(), our_permit.clone())
            }
            _ => {
                warn!("Unexpected Welcome in state {}", state.state.name());
                return;
            }
        };

        info!("Received Welcome from {}", self.node_id);

        // Store peer's encryption key for ECDH
        state.peer_encryption_key = Some(*node_encryption_key);

        // Parse the permit we received, cache for later use
        let parsed_permit = match parse_permit(permit_for_us) {
            Ok(p) => p,
            Err(e) => {
                warn!("Invalid permit in Welcome: {}", e);
                state.state = PeerState::fail("Invalid permit received");
                return;
            }
        };
        state.cached_peer_permit = Some(parsed_permit.clone());

        // Parse our original permit for WelcomeContext
        let our_parsed_permit = match parse_permit(&our_permit) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to parse our stored permit: {}", e);
                state.state = PeerState::fail("Internal error");
                return;
            }
        };

        // Get our public key for audience validation
        // Permits use base64-encoded pubkey as audience, not DID
        let our_pubkey_b64 = match state.butler.user_info().await {
            Ok(info) => STANDARD.encode(&info.public_key),
            Err(e) => {
                error!("Failed to get user info for audience validation: {}", e);
                state.state = PeerState::fail("Internal error");
                return;
            }
        };

        // Build context for decision
        let ctx = WelcomeContext {
            our_permit: &our_parsed_permit,
            our_did: &our_did,
            our_pubkey_b64: &our_pubkey_b64,
            expected_node_pubkey: &expected_node_pubkey,
            received_node_pubkey: node_public_key,
            received_permit: &parsed_permit,
        };

        // Get decision
        let decision = decide_welcome_response(&ctx);
        debug!(?decision, "Welcome decision");

        // Execute decision
        match decision {
            WelcomeDecision::Accept { can_publish } => {
                self.complete_welcome_flow(
                    myself,
                    permit_for_us,
                    &our_did,
                    &our_username,
                    can_publish,
                    node_public_key,
                    state,
                ).await;
            }
            WelcomeDecision::RejectNodeMismatch => {
                warn!("Node public key mismatch - possible MITM attack");
                state.state = PeerState::fail("Node identity mismatch");
            }
            WelcomeDecision::RejectAudienceMismatch => {
                warn!("Permit audience mismatch");
                state.state = PeerState::fail("Permit audience mismatch");
            }
        }
    }

    /// Complete Welcome flow after successful validation
    ///
    /// **Context**: Decision approved the Welcome
    /// **We store**: Node's permit
    /// **We issue**: Reciprocal permit
    /// **We send**: PermitGrant
    /// **On reconnection**: Also subscribe to active Scribes for live sync
    #[instrument(skip_all, fields(node = %self.node_id, can_publish = our_can_publish))]
    async fn complete_welcome_flow(
        &self,
        myself: ractor::ActorRef<super::PeerMessage>,
        permit_for_us: &str,
        _our_did: &str,
        _our_username: &str,
        our_can_publish: bool,
        node_public_key: &[u8],
        state: &mut PeerActorState<C>,
    ) {
        let node_id_str = self.node_id.to_string();

        // Derive node's DID from their public signing key
        // This is used for consistent identity tracking (e.g., state vector storage)
        let node_public_key_32: [u8; 32] = match node_public_key.try_into() {
            Ok(key) => key,
            Err(_) => {
                error!("Invalid node public key length: expected 32 bytes");
                state.state = PeerState::fail("Invalid node public key");
                return;
            }
        };
        let node_did = Identity::did_from_public_key(&node_public_key_32);

        // Check if this is a reconnection
        let stored_permit = state.butler
            .nodes().get(&node_id_str)
            .ok()
            .flatten()
            .and_then(|n| n.permit);

        if let Some(existing_permit) = stored_permit {
            // Reconnection: Verify the permit matches
            if permit_for_us == existing_permit {
                info!("Reconnection: Node returned our stored permit, sending Ack");
                self.send_message(&Message::Ack, state).await;

                // Transition directly to Authenticated
                let peer_type = PeerType::MyNode;

                state.state = PeerState::Authenticated {
                    peer_type,
                    did: node_did.clone(),
                    username: "node".to_string(),
                };

                self.notify_authenticated(state, peer_type, &node_did, "node");
                info!(can_publish = our_can_publish, "Reconnection handshake complete with node {}", self.node_id);

                // Subscribe peer to our active Scribes for live sync
                self.subscribe_to_active_scribes(myself, state).await;
                return;
            } else {
                warn!("Reconnection: Permit mismatch - falling through to first connection flow");
            }
        }

        // First connection flow
        debug!(can_publish = our_can_publish, "First connection: issuing permit for node");

        // Store the permit from node
        if let Err(e) = state.butler.nodes().set_permit(&node_id_str, permit_for_us.to_string()) {
            warn!("Failed to store permit: {}", e);
        }

        // Issue reciprocal permit for the node using its base64-encoded public key as audience
        // Relationship based on our capability - gurkha expects owner_node or viewer_node
        let relationship = if our_can_publish { "owner_node" } else { "viewer_node" };
        let node_pubkey_b64 = STANDARD.encode(node_public_key);
        let (permit_for_node, _our_pubkey) = match state.butler.issue_peer_connection_permit(&node_pubkey_b64, relationship).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to issue permit for node: {}", e);
                state.state = PeerState::fail("Failed to issue permit");
                return;
            }
        };

        // Send PermitGrant
        let permit_grant = Message::PermitGrant(PermitGrantMsg {
            permit_for_node: permit_for_node.clone(),
        });

        info!(can_publish = our_can_publish, "Sending PermitGrant to {}", self.node_id);
        self.send_message(&permit_grant, state).await;

        // Transition to AwaitingPermitGrant (waiting for Ack)
        // Use the derived DID for consistent identity tracking
        state.state = PeerState::AwaitingPermitGrant {
            their_did: node_did,
            their_username: "node".to_string(),
            is_first_connection: true,
            can_publish: our_can_publish,
        };
    }

    /// Handle PermitGrant message (Node mode receives this)
    ///
    /// **Context**: User sent us their permit after Welcome
    /// **We call**: decide_permit_grant_response() for decision
    /// **We store**: The permit they gave us
    /// **We send**: Ack
    /// **Result**: Both sides authenticated
    /// **Note**: Subscription happens later via refresh_subscriptions_after_page_data
    #[instrument(skip(self, _myself, state, permit_for_node))]
    pub(super) async fn on_permit_grant(
        &self,
        _myself: ractor::ActorRef<super::PeerMessage>,
        permit_for_node: &str,
        state: &mut PeerActorState<C>,
    ) {
        if require_node_mode(state.mode, "on_permit_grant").is_some() {
            return;
        }

        // Extract state data
        let (their_did, their_username, peer_can_publish) = match &state.state {
            PeerState::AwaitingPermitGrant { their_did, their_username, can_publish, .. } => {
                (their_did.clone(), their_username.clone(), *can_publish)
            }
            _ => {
                warn!("Unexpected PermitGrant in state {}", state.state.name());
                return;
            }
        };

        info!("Received PermitGrant from {}", their_username);

        // Parse the permit, cache for later use
        let parsed_permit = match parse_permit(permit_for_node) {
            Ok(p) => p,
            Err(e) => {
                warn!("Invalid permit in PermitGrant: {}", e);
                self.reject("Invalid permit", state).await;
                return;
            }
        };
        state.cached_peer_permit = Some(parsed_permit.clone());

        // Get our pubkey for validation (base64-encoded)
        let our_pubkey_b64 = match state.butler.get_identity().await {
            Ok(identity) => STANDARD.encode(identity.public_signing_key()),
            Err(e) => {
                error!("Failed to get identity: {}", e);
                self.reject("Internal error", state).await;
                return;
            }
        };

        // Build context for decision
        let ctx = PermitGrantContext {
            peer_can_publish,
            our_pubkey_b64: &our_pubkey_b64,
            their_did: &their_did,
            received_permit: &parsed_permit,
        };

        // Get decision
        let decision = decide_permit_grant_response(&ctx);
        debug!(?decision, "PermitGrant decision");

        // Execute decision
        match decision {
            PermitGrantDecision::Accept { can_publish } => {
                // Store connection permit for non-owner peers (viewers)
                // This allows node to reconnect to them later if needed
                if can_publish {
                    debug!("Owner handshake complete - received reciprocal permit");
                } else {
                    if let Err(e) = state.butler.store().put_connection_permit(&their_did, permit_for_node) {
                        warn!("Failed to store connection permit: {}", e);
                    } else {
                        debug!(their_did = %their_did, "Stored viewer connection permit");
                    }
                }

                // Transition to Authenticated
                let peer_type = PeerType::from_can_publish(can_publish);
                state.state = PeerState::Authenticated {
                    peer_type,
                    did: their_did.clone(),
                    username: their_username.clone(),
                };

                // Notify coordinator
                self.notify_authenticated(state, peer_type, &their_did, &their_username);

                // Send Ack
                self.send_message(&Message::Ack, state).await;

                info!(can_publish = can_publish, "Handshake complete with {} ({})", their_username, self.node_id);

                // NOTE: Don't subscribe here for first connections - viewer has no pages yet.
                // Subscription happens in refresh_subscriptions_after_page_data after PageData is sent.
            }
            PermitGrantDecision::RejectAudienceMismatch => {
                warn!("PermitGrant audience mismatch");
                self.reject("Permit audience mismatch", state).await;
            }
            PermitGrantDecision::RejectIssuerMismatch => {
                warn!("PermitGrant issuer mismatch");
                self.reject("Permit issuer mismatch", state).await;
            }
        }
    }

    /// Handle Ack message
    ///
    /// **User mode**: Ack after PermitGrant completes first connection handshake
    /// **Node mode (reconnection)**: Ack after Welcome completes reconnection handshake
    /// **Post-auth**: Subscribe peer to our active Scribes for live sync
    #[instrument(skip(self, myself, state))]
    pub(super) async fn on_ack(
        &self,
        myself: ractor::ActorRef<super::PeerMessage>,
        state: &mut PeerActorState<C>,
    ) {
        // Extract state data
        let (their_did, their_username, is_first_connection, peer_can_publish) = match &state.state {
            PeerState::AwaitingPermitGrant { their_did, their_username, is_first_connection, can_publish } => {
                (their_did.clone(), their_username.clone(), *is_first_connection, *can_publish)
            }
            _ => {
                warn!("Unexpected Ack in state {}", state.state.name());
                return;
            }
        };

        if is_first_connection {
            // User mode: First connection complete after PermitGrant
            let peer_type = PeerType::MyNode;
            state.state = PeerState::Authenticated {
                peer_type,
                did: their_did.clone(),
                username: their_username.clone(),
            };
            self.notify_authenticated(state, peer_type, &their_did, &their_username);
            info!(can_publish = peer_can_publish, "First connection handshake complete with node {}", self.node_id);
        } else {
            // Node mode: Reconnection complete - peer verified our stored permit
            let peer_type = PeerType::from_can_publish(peer_can_publish);
            state.state = PeerState::Authenticated {
                peer_type,
                did: their_did.clone(),
                username: their_username.clone(),
            };
            self.notify_authenticated(state, peer_type, &their_did, &their_username);
            info!(can_publish = peer_can_publish, "Reconnection handshake complete with {}", their_username);

            // Subscribe peer to our active Scribes for live sync (reconnection only)
            // For reconnection, viewer already has pages and permits stored.
            self.subscribe_to_active_scribes(myself, state).await;
        }

        // NOTE: For first connection, subscription happens after PageData is sent
        // via refresh_subscriptions_after_page_data when viewer's permit is stored.

        // Drain pending space request queued before handshake completed
        // (viewer sent SpaceRequest before Ack arrived)
        if let Some((space_id, viewer_permit)) = state.pending_space_request.take() {
            info!("Draining queued SpaceRequest for {} after handshake", space_id);
            self.initiate_request_space_as_viewer(&space_id, &viewer_permit, state).await;
        }
    }
}
