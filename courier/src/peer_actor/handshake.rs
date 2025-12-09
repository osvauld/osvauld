//! Handshake handlers for peer authentication
//!
//! Flow:
//! 1. User sends Hello → Node
//! 2. Node validates (via gurkha decision), sends Welcome → User
//! 3. User validates Welcome (via gurkha decision), sends PermitGrant → Node
//! 4. Node validates PermitGrant (via gurkha decision), sends Ack → User
//! 5. Both sides authenticated
//!
//! Identity model:
//! - User/Node have verifying_key (Ed25519, 32 bytes) + encryption_key (X25519, 32 bytes)
//! - All keys are [u8; 32]
//! - Single permit per owner<->node relationship
//!
//! Security model (gurkha decisions):
//! - Node decides how to respond to Hello (owner/viewer, first/reconnect)
//! - User validates Welcome permit matches expected role (prevents escalation)
//! - Node validates PermitGrant permit from user

use base64::{engine::general_purpose::STANDARD, Engine};
use tracing::{debug, error, info, warn, instrument};

use gurkha::{
    HandshakeRole, HelloDecision, WelcomeDecision, PermitGrantDecision,
    HelloContext, WelcomeContext, PermitGrantContext,
    decide_hello_response, decide_welcome_response, decide_permit_grant_response,
};

use crate::message::Message;
use crate::state::{PeerState, PeerType};
use butler::OwnerInfo;

use super::guards::{require_node_mode, require_user_mode, parse_permit};
use super::{PeerActor, PeerActorState};

impl PeerActor {
    // ==================== User Mode: Initiate Handshake ====================

    /// Initiate handshake by sending Hello (User mode only)
    ///
    /// **Context**: User connects to Node and starts handshake
    /// **We send**: Hello with our identity and permit
    /// **Next**: Wait for Welcome
    #[instrument(skip(self, state, permit))]
    pub(super) async fn initiate_handshake(&self, permit: &str, state: &mut PeerActorState) {
        if require_user_mode(state.mode, "initiate_handshake").is_some() {
            return;
        }

        if !matches!(state.state, PeerState::Connected) {
            warn!("Cannot initiate handshake in state {}", state.state.name());
            return;
        }

        // Parse the permit to determine our role
        let parsed_permit = match parse_permit(permit) {
            Ok(p) => p,
            Err(e) => {
                error!("Invalid permit for handshake: {}", e);
                state.state = PeerState::fail("Invalid permit");
                return;
            }
        };

        // Determine our role from the permit
        let our_role = match HandshakeRole::from_permit(parsed_permit.core()) {
            Some(role) => role,
            None => {
                error!("Cannot determine role from permit");
                state.state = PeerState::fail("Unknown role in permit");
                return;
            }
        };

        debug!(?our_role, "Determined role from permit");

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
        // We compare against the verifying key (did), not the device key
        let node_id_str = self.node_id.to_string();
        let expected_node_pubkey = match state.butler.get_sovereign_node(&node_id_str) {
            Ok(Some(sovereign_node)) => {
                // Decode the verifying key (did) from base64
                // This is the node's Ed25519 public key, same as what Welcome sends
                match STANDARD.decode(&sovereign_node.did) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        error!("Failed to decode node verifying key: {}", e);
                        state.state = PeerState::fail("Invalid node public key");
                        return;
                    }
                }
            }
            Ok(None) => {
                // Viewer flow - might not have sovereign node record
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
        let signature = vec![]; // Placeholder

        // Convert public keys to [u8; 32] arrays
        let public_key: [u8; 32] = user_info.public_key.clone().try_into()
            .expect("public_key should be 32 bytes");
        let encryption_key: [u8; 32] = user_info.encryption_key.clone().try_into()
            .expect("encryption_key should be 32 bytes");

        let hello = Message::Hello {
            did: user_info.did.clone(),
            username: user_info.username.clone(),
            public_key,
            encryption_key,
            signature,
            timestamp,
            permit: permit.to_string(),
        };

        info!(?our_role, "Sending Hello to {} as {}", self.node_id, user_info.username);
        self.send_message(&hello, state).await;

        // Transition to AwaitingWelcome with role tracking
        state.state = PeerState::AwaitingWelcome {
            our_did: user_info.did,
            our_username: user_info.username,
            sent_at: timestamp,
            our_role,
            expected_node_pubkey,
        };
    }

    // ==================== Node Mode: Handle Hello ====================

    /// Handle Hello message (Node mode receives this)
    ///
    /// **Context**: Owner/User/Viewer connects to us (the Node)
    /// **Peer sends**: Hello with identity and permit
    /// **We call**: gurkha::decide_hello_response() for decision
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
        state: &mut PeerActorState,
    ) {
        if require_node_mode(state.mode, "on_hello").is_some() {
            return;
        }

        if !matches!(state.state, PeerState::Connected) {
            warn!("Unexpected Hello in state {}", state.state.name());
            return;
        }

        // Store peer's encryption key for ECDH during SyncPush
        state.peer_encryption_key = Some(*encryption_key);

        info!("Received Hello from {} ({})", username, did);

        // Parse and validate permit
        let parsed_permit = match parse_permit(permit) {
            Ok(p) => p,
            Err(e) => {
                self.reject(&e, state).await;
                return;
            }
        };

        let relationship = parsed_permit.relationship().unwrap_or("unknown");
        let is_first_connection = parsed_permit.is_first_connection();
        debug!(relationship, is_first_connection, "Permit parsed");

        // Check if we already have an owner
        let existing_owner = match state.butler.get_owner() {
            Ok(owner) => owner,
            Err(e) => {
                error!("Failed to check owner: {}", e);
                self.reject("Internal error", state).await;
                return;
            }
        };

        // Build context for gurkha decision
        let ctx = HelloContext {
            incoming_permit: parsed_permit.core(),
            incoming_did: did,
            existing_owner_did: existing_owner.as_ref().map(|o| o.did.as_str()),
            existing_owner_permit: existing_owner.as_ref().and_then(|o| o.permit.as_deref()),
        };

        // Get decision from gurkha
        let decision = decide_hello_response(&ctx);
        debug!(?decision, "Hello decision from gurkha");

        // Execute decision
        match decision {
            HelloDecision::AcceptOwnerFirstConnection => {
                self.handle_owner_first_connection(did, username, public_key, encryption_key, state).await;
            }
            HelloDecision::AcceptOwnerReconnection { stored_permit } => {
                self.handle_owner_reconnection(did, username, &stored_permit, state).await;
            }
            HelloDecision::AcceptViewer => {
                self.handle_viewer_connection(did, username, public_key, encryption_key, state).await;
            }
            HelloDecision::RejectAlreadyHasOwner => {
                self.reject("Node already has an owner", state).await;
            }
            HelloDecision::RejectOwnerMismatch => {
                self.reject("Owner DID mismatch", state).await;
            }
            HelloDecision::RejectUnknownRole { relationship } => {
                self.reject(&format!("Unknown relationship: {}", relationship), state).await;
            }
            HelloDecision::RejectNoOwnerForReconnection => {
                self.reject("No owner registered for reconnection", state).await;
            }
        }
    }

    /// Handle owner first connection (Node mode)
    ///
    /// **Context**: New owner connecting for the first time
    /// **We store**: OwnerInfo
    /// **We issue**: node_to_owner permit
    /// **We send**: Welcome
    async fn handle_owner_first_connection(
        &self,
        did: &str,
        username: &str,
        public_key: &[u8; 32],
        encryption_key: &[u8; 32],
        state: &mut PeerActorState,
    ) {
        // Store owner info
        let encryption_key_base64 = STANDARD.encode(encryption_key);
        let owner_info = OwnerInfo::new(
            did.to_string(),
            encryption_key_base64,
            username.to_string(),
        );

        if let Err(e) = state.butler.set_owner(&owner_info) {
            error!("Failed to store owner info: {}", e);
            self.reject("Failed to store owner", state).await;
            return;
        }

        info!("Stored owner info for first connection");

        // Issue new permit for the owner
        let peer_pubkey = STANDARD.encode(public_key);
        let (permit_for_owner, _our_pubkey) = match state.butler.issue_peer_connection_permit(&peer_pubkey, "node_owner").await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to issue permit: {}", e);
                self.reject("Failed to issue permit", state).await;
                return;
            }
        };

        // Store the permit we issued
        if let Err(e) = state.butler.set_owner_permit(permit_for_owner.clone()) {
            warn!("Failed to store owner permit: {}", e);
        }

        // Send Welcome
        self.send_welcome_message(&permit_for_owner, state).await;

        // Transition to AwaitingPermitGrant
        state.state = PeerState::AwaitingPermitGrant {
            their_did: did.to_string(),
            their_username: username.to_string(),
            is_first_connection: true,
            their_role: HandshakeRole::Owner,
        };
    }

    /// Handle owner reconnection (Node mode)
    ///
    /// **Context**: Known owner reconnecting
    /// **We send**: Welcome with stored permit
    async fn handle_owner_reconnection(
        &self,
        did: &str,
        username: &str,
        stored_permit: &str,
        state: &mut PeerActorState,
    ) {
        debug!("Reconnection from known owner");
        let _ = state.butler.update_owner_last_connected();

        // Send Welcome with stored permit
        self.send_welcome_message(stored_permit, state).await;

        // Transition to AwaitingPermitGrant (waiting for Ack)
        state.state = PeerState::AwaitingPermitGrant {
            their_did: did.to_string(),
            their_username: username.to_string(),
            is_first_connection: false,
            their_role: HandshakeRole::Owner,
        };
    }

    /// Handle viewer connection (Node mode)
    ///
    /// **Context**: Viewer connecting to access shared content
    /// **We store**: Contact record (as Node type contact)
    /// **We issue**: node_to_viewer permit
    /// **We send**: Welcome
    async fn handle_viewer_connection(
        &self,
        did: &str,
        username: &str,
        public_key: &[u8; 32],
        _encryption_key: &[u8; 32],
        state: &mut PeerActorState,
    ) {
        info!("Accepting viewer connection from {} ({})", username, did);

        // Issue permit for viewer
        let peer_pubkey = STANDARD.encode(public_key);
        let (permit_for_viewer, _our_pubkey) = match state.butler.issue_peer_connection_permit(&peer_pubkey, "node_viewer").await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to issue viewer permit: {}", e);
                self.reject("Failed to issue permit", state).await;
                return;
            }
        };

        // TODO: Store viewer as contact with ContactType::Node
        // For now just log
        debug!("Viewer permit issued, contact storage TODO");

        // Send Welcome
        self.send_welcome_message(&permit_for_viewer, state).await;

        // Transition to AwaitingPermitGrant
        state.state = PeerState::AwaitingPermitGrant {
            their_did: did.to_string(),
            their_username: username.to_string(),
            is_first_connection: true,
            their_role: HandshakeRole::Viewer,
        };
    }

    /// Send Welcome message with our identity and permit for peer
    async fn send_welcome_message(&self, permit_for_peer: &str, state: &mut PeerActorState) {
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

        let welcome = Message::Welcome {
            node_id: self.node_id.to_string(),
            node_public_key,
            node_encryption_key,
            signature,
            timestamp,
            permit_for_peer: permit_for_peer.to_string(),
        };

        info!("Sending Welcome");
        self.send_message(&welcome, state).await;
    }

    // ==================== User Mode: Handle Welcome ====================

    /// Handle Welcome message (User mode receives this)
    ///
    /// **Context**: Node responded to our Hello
    /// **We call**: gurkha::decide_welcome_response() for decision
    /// **We validate**: Node identity, permit type matches our role (security!)
    /// **We issue**: Reciprocal permit based on our role
    /// **We send**: PermitGrant
    #[instrument(skip(self, state, permit_for_us))]
    pub(super) async fn on_welcome(
        &self,
        permit_for_us: &str,
        node_public_key: &[u8; 32],
        node_encryption_key: &[u8; 32],
        state: &mut PeerActorState,
    ) {
        if require_user_mode(state.mode, "on_welcome").is_some() {
            return;
        }

        // Extract state data
        let (our_did, our_username, our_role, expected_node_pubkey) = match &state.state {
            PeerState::AwaitingWelcome { our_did, our_username, our_role, expected_node_pubkey, .. } => {
                (our_did.clone(), our_username.clone(), *our_role, expected_node_pubkey.clone())
            }
            _ => {
                warn!("Unexpected Welcome in state {}", state.state.name());
                return;
            }
        };

        info!("Received Welcome from {}", self.node_id);

        // Store peer's encryption key for ECDH
        state.peer_encryption_key = Some(*node_encryption_key);

        // Parse the permit we received
        let parsed_permit = match parse_permit(permit_for_us) {
            Ok(p) => p,
            Err(e) => {
                warn!("Invalid permit in Welcome: {}", e);
                state.state = PeerState::fail("Invalid permit received");
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

        // Build context for gurkha decision
        let ctx = WelcomeContext {
            our_role,
            our_did: &our_did,
            our_pubkey_b64: &our_pubkey_b64,
            expected_node_pubkey: &expected_node_pubkey,
            received_node_pubkey: node_public_key,
            received_permit: parsed_permit.core(),
        };

        // Get decision from gurkha
        let decision = decide_welcome_response(&ctx);
        debug!(?decision, "Welcome decision from gurkha");

        // Execute decision
        match decision {
            WelcomeDecision::Accept { reciprocal_permit_type, our_role } => {
                self.complete_welcome_flow(
                    permit_for_us,
                    &our_did,
                    &our_username,
                    our_role,
                    &reciprocal_permit_type,
                    node_public_key,
                    state,
                ).await;
            }
            WelcomeDecision::RejectNodeMismatch => {
                warn!("Node public key mismatch - possible MITM attack");
                state.state = PeerState::fail("Node identity mismatch");
            }
            WelcomeDecision::RejectPermitTypeMismatch { expected, received } => {
                warn!("Privilege escalation attempt: expected {} got {}", expected, received);
                state.state = PeerState::fail("Invalid permit type - possible privilege escalation");
            }
            WelcomeDecision::RejectAudienceMismatch => {
                warn!("Permit audience mismatch");
                state.state = PeerState::fail("Permit audience mismatch");
            }
        }
    }

    /// Complete Welcome flow after successful validation
    ///
    /// **Context**: gurkha approved the Welcome
    /// **We store**: Node's permit
    /// **We issue**: Reciprocal permit
    /// **We send**: PermitGrant
    async fn complete_welcome_flow(
        &self,
        permit_for_us: &str,
        _our_did: &str,
        _our_username: &str,
        our_role: HandshakeRole,
        reciprocal_permit_type: &str,
        node_public_key: &[u8],
        state: &mut PeerActorState,
    ) {
        let node_id_str = self.node_id.to_string();

        // Check if this is a reconnection
        let stored_permit = state.butler
            .get_sovereign_node(&node_id_str)
            .ok()
            .flatten()
            .and_then(|n| n.permit);

        if let Some(existing_permit) = stored_permit {
            // Reconnection: Verify the permit matches
            if permit_for_us == existing_permit {
                info!("Reconnection: Node returned our stored permit, sending Ack");
                self.send_message(&Message::Ack, state).await;

                // Transition directly to Authenticated
                let peer_type = match our_role {
                    HandshakeRole::Owner => PeerType::MyNode,
                    HandshakeRole::Viewer => PeerType::MyNode, // Viewer's node is still "MyNode" from their perspective
                };

                state.state = PeerState::Authenticated {
                    peer_type,
                    did: node_id_str.clone(),
                    username: "node".to_string(),
                };

                self.notify_authenticated(state, peer_type, &node_id_str, "node");
                info!(?our_role, "Reconnection handshake complete with node {}", self.node_id);
                return;
            } else {
                warn!("Reconnection: Permit mismatch - falling through to first connection flow");
            }
        }

        // First connection flow
        debug!(?our_role, "First connection: issuing {} permit for node", reciprocal_permit_type);

        // Store the permit from node
        if let Err(e) = state.butler.set_sovereign_node_permit(&node_id_str, permit_for_us.to_string()) {
            warn!("Failed to store permit: {}", e);
        }

        // Issue reciprocal permit for the node using its base64-encoded public key as audience
        // Owner issues owner_node, viewer issues viewer_node - bidirectional naming
        let node_pubkey_b64 = STANDARD.encode(node_public_key);
        let role_str = our_role.relationship_for_node();
        let (permit_for_node, _our_pubkey) = match state.butler.issue_peer_connection_permit(&node_pubkey_b64, role_str).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to issue permit for node: {}", e);
                state.state = PeerState::fail("Failed to issue permit");
                return;
            }
        };

        // Send PermitGrant
        let permit_grant = Message::PermitGrant {
            permit_for_node: permit_for_node.clone(),
        };

        info!(?our_role, "Sending PermitGrant to {}", self.node_id);
        self.send_message(&permit_grant, state).await;

        // Transition to AwaitingPermitGrant (waiting for Ack)
        state.state = PeerState::AwaitingPermitGrant {
            their_did: node_id_str,
            their_username: "node".to_string(),
            is_first_connection: true,
            their_role: our_role, // From user's perspective, track our role
        };
    }

    // ==================== Node Mode: Handle PermitGrant ====================

    /// Handle PermitGrant message (Node mode receives this)
    ///
    /// **Context**: User sent us their permit after Welcome
    /// **We call**: gurkha::decide_permit_grant_response() for decision
    /// **We store**: The permit they gave us
    /// **We send**: Ack
    /// **Result**: Both sides authenticated
    #[instrument(skip(self, state, permit_for_node))]
    pub(super) async fn on_permit_grant(&self, permit_for_node: &str, state: &mut PeerActorState) {
        if require_node_mode(state.mode, "on_permit_grant").is_some() {
            return;
        }

        // Extract state data
        let (their_did, their_username, their_role) = match &state.state {
            PeerState::AwaitingPermitGrant { their_did, their_username, their_role, .. } => {
                (their_did.clone(), their_username.clone(), *their_role)
            }
            _ => {
                warn!("Unexpected PermitGrant in state {}", state.state.name());
                return;
            }
        };

        info!("Received PermitGrant from {}", their_username);

        // Parse the permit
        let parsed_permit = match parse_permit(permit_for_node) {
            Ok(p) => p,
            Err(e) => {
                warn!("Invalid permit in PermitGrant: {}", e);
                self.reject("Invalid permit", state).await;
                return;
            }
        };

        // Get our pubkey for validation (base64-encoded)
        let our_pubkey_b64 = match state.butler.get_identity().await {
            Ok(identity) => STANDARD.encode(identity.public_signing_key()),
            Err(e) => {
                error!("Failed to get identity: {}", e);
                self.reject("Internal error", state).await;
                return;
            }
        };

        // Build context for gurkha decision
        let ctx = PermitGrantContext {
            expected_role: their_role,
            our_pubkey_b64: &our_pubkey_b64,
            their_did: &their_did,
            received_permit: parsed_permit.core(),
        };

        // Get decision from gurkha
        let decision = decide_permit_grant_response(&ctx);
        debug!(?decision, "PermitGrant decision from gurkha");

        // Execute decision
        match decision {
            PermitGrantDecision::Accept { peer_role } => {
                // Note: permit_for_owner was already stored in handle_owner_first_connection
                // permit_for_node is the reciprocal permit (for node to auth to owner if needed)
                // We don't overwrite the stored permit here - that would break reconnection!
                match peer_role {
                    HandshakeRole::Owner => {
                        debug!("Owner handshake complete - received reciprocal permit");
                    }
                    HandshakeRole::Viewer => {
                        // TODO: Store viewer permit in contact record
                        debug!("Viewer permit storage TODO");
                    }
                }

                // Transition to Authenticated
                let peer_type: PeerType = peer_role.into();
                state.state = PeerState::Authenticated {
                    peer_type,
                    did: their_did.clone(),
                    username: their_username.clone(),
                };

                // Notify coordinator
                self.notify_authenticated(state, peer_type, &their_did, &their_username);

                // Send Ack
                self.send_message(&Message::Ack, state).await;

                info!(?peer_role, "Handshake complete with {} ({})", their_username, self.node_id);
            }
            PermitGrantDecision::RejectPermitTypeMismatch { expected, received } => {
                warn!("PermitGrant type mismatch: expected {} got {}", expected, received);
                self.reject("Invalid permit type", state).await;
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

    // ==================== Both Modes: Handle Ack ====================

    /// Handle Ack message
    ///
    /// **User mode**: Ack after PermitGrant completes first connection handshake
    /// **Node mode (reconnection)**: Ack after Welcome completes reconnection handshake
    #[instrument(skip(self, state))]
    pub(super) async fn on_ack(&self, state: &mut PeerActorState) {
        // Extract state data
        let (their_did, their_username, is_first_connection, their_role) = match &state.state {
            PeerState::AwaitingPermitGrant { their_did, their_username, is_first_connection, their_role } => {
                (their_did.clone(), their_username.clone(), *is_first_connection, *their_role)
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
            info!(?their_role, "First connection handshake complete with node {}", self.node_id);
        } else {
            // Node mode: Reconnection complete - peer verified our stored permit
            let peer_type: PeerType = their_role.into();
            state.state = PeerState::Authenticated {
                peer_type,
                did: their_did.clone(),
                username: their_username.clone(),
            };
            self.notify_authenticated(state, peer_type, &their_did, &their_username);
            info!(?their_role, "Reconnection handshake complete with {}", their_username);
        }
    }
}
