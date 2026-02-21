//! Test fixtures for Scribe
//!
//! Re-exports gurkha fixtures and provides scribe-specific test helpers.

// Re-export gurkha test fixtures (permit creation)
pub use gurkha::test_fixtures::*;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::policy_compat;
use crate::state::{PeerConnection, ScribeArgs, SyncConfig, SyncMode};
use crate::storage::{
    LayerStorageRef, NullLayerStorage, NullPeerResolver, NullPeerVectorStorage,
    PeerVectorStorageRef,
};
use crate::BroadcastPayload;
use domains::Layer;

fn to_policy_permit(permit: gurkha::Permit) -> gurkha::PolicyPermit {
    gurkha::PolicyPermit::from_token(permit.raw_token())
        .expect("fixture permit should parse as PolicyPermit")
}

// Mock Storage

/// Create null storage references for testing
pub fn null_layer_storage() -> LayerStorageRef {
    Arc::new(NullLayerStorage)
}

pub fn null_vector_storage() -> PeerVectorStorageRef {
    Arc::new(NullPeerVectorStorage)
}

// ScribeArgs Builders

/// Create minimal ScribeArgs for testing
pub fn minimal_scribe_args(page_id: &str) -> ScribeArgs {
    ScribeArgs {
        page_id: page_id.to_string(),
        layers: HashMap::new(),
        layer_storage: null_layer_storage(),
        vector_storage: null_vector_storage(),
        peer_resolver: None,
        permit_issuer: None,
        sync_config: None,
        sync_event_tx: None,
        validation_handle: None,
        our_permit: None,
        our_did: String::new(),
        our_username: String::new(),
        capture_tx: None,
        is_node: false,
    }
}

/// Create ScribeArgs with layers
pub fn scribe_args_with_layers(page_id: &str, layer_names: &[&str]) -> ScribeArgs {
    let mut layers = HashMap::new();
    for name in layer_names {
        layers.insert(name.to_string(), Layer::new());
    }

    ScribeArgs {
        page_id: page_id.to_string(),
        layers,
        layer_storage: null_layer_storage(),
        vector_storage: null_vector_storage(),
        peer_resolver: None,
        permit_issuer: None,
        sync_config: None,
        sync_event_tx: None,
        capture_tx: None,
        validation_handle: None,
        our_permit: None,
        our_did: String::new(),
        our_username: String::new(),
        is_node: false,
    }
}

/// Create ScribeArgs for node mode
pub fn node_scribe_args(page_id: &str) -> ScribeArgs {
    ScribeArgs {
        page_id: page_id.to_string(),
        layers: HashMap::new(),
        layer_storage: null_layer_storage(),
        vector_storage: null_vector_storage(),
        peer_resolver: Some(Arc::new(NullPeerResolver)),
        permit_issuer: None,
        sync_config: Some(SyncConfig {
            mode: SyncMode::Broadcast,
            sync_target: None,
        }),
        sync_event_tx: None,
        capture_tx: None,
        validation_handle: None,
        our_permit: None,
        our_did: "did:key:node123".to_string(),
        our_username: "test_node".to_string(),
        is_node: true,
    }
}

/// Create ScribeArgs for viewer mode (sync to source)
pub fn viewer_scribe_args(page_id: &str, sync_target: &str) -> ScribeArgs {
    ScribeArgs {
        page_id: page_id.to_string(),
        layers: HashMap::new(),
        layer_storage: null_layer_storage(),
        vector_storage: null_vector_storage(),
        peer_resolver: None,
        permit_issuer: None,
        sync_config: Some(SyncConfig {
            mode: SyncMode::ToSource,
            sync_target: Some(sync_target.to_string()),
        }),
        sync_event_tx: None,
        capture_tx: None,
        validation_handle: None,
        our_permit: None,
        our_did: "did:key:viewer123".to_string(),
        our_username: "test_viewer".to_string(),
        is_node: false,
    }
}

// PeerConnection Builders

/// Create a mock PeerConnection for testing
pub fn mock_subscriber_info(permit: gurkha::PolicyPermit, subscriber_did: &str) -> PeerConnection {
    let (tx, _rx) = mpsc::channel::<BroadcastPayload>(16);
    let is_visible = policy_compat::is_visible(&permit);
    let can_see_others = policy_compat::can_see_others(&permit);
    let display_name = policy_compat::display_name(&permit);
    PeerConnection {
        permit,
        subscriber_did: subscriber_did.to_string(),
        is_visible,
        can_see_others,
        display_name,
        broadcast_tx: tx,
        ephemeral_tx: None,
    }
}

/// Create a subscriber with shop customer permit
pub fn shop_customer_subscriber(page_id: &str, did: &str) -> PeerConnection {
    let permit = to_policy_permit(gurkha::test_fixtures::shop_customer(page_id, did));
    mock_subscriber_info(permit, did)
}

/// Create a subscriber with shop owner permit
pub fn shop_owner_subscriber(page_id: &str, did: &str) -> PeerConnection {
    let permit = to_policy_permit(gurkha::test_fixtures::shop_owner(page_id, did));
    mock_subscriber_info(permit, did)
}

// BroadcastPayload Builders

/// Create a test BroadcastPayload
pub fn test_broadcast_payload(page_id: &str, layer_name: &str) -> BroadcastPayload {
    BroadcastPayload {
        page_id: page_id.to_string(),
        layer_name: layer_name.to_string(),
        layer_type: domains::LayerType::from_layer_name(layer_name),
        update: vec![1, 2, 3, 4],       // Dummy update bytes
        state_vector: vec![0, 0, 0, 1], // Dummy state vector
    }
}

/// Create a BroadcastPayload with specific update data
pub fn broadcast_payload_with_data(
    page_id: &str,
    layer_name: &str,
    update: Vec<u8>,
    state_vector: Vec<u8>,
) -> BroadcastPayload {
    BroadcastPayload {
        page_id: page_id.to_string(),
        layer_name: layer_name.to_string(),
        layer_type: domains::LayerType::from_layer_name(layer_name),
        update,
        state_vector,
    }
}

// Ephemeral Testing

use crate::EphemeralOutbound;

/// Create a mock PeerConnection with ephemeral channel for testing ephemeral flow
///
/// Returns (PeerConnection, ephemeral_rx) so test can verify ephemerals arrive
pub fn mock_subscriber_with_ephemeral(
    permit: gurkha::PolicyPermit,
    subscriber_did: &str,
) -> (PeerConnection, mpsc::Receiver<EphemeralOutbound>) {
    let (broadcast_tx, _broadcast_rx) = mpsc::channel::<BroadcastPayload>(16);
    let (ephemeral_tx, ephemeral_rx) = mpsc::channel::<EphemeralOutbound>(16);
    let is_visible = policy_compat::is_visible(&permit);
    let can_see_others = policy_compat::can_see_others(&permit);
    let display_name = policy_compat::display_name(&permit);

    let info = PeerConnection {
        permit,
        subscriber_did: subscriber_did.to_string(),
        is_visible,
        can_see_others,
        display_name,
        broadcast_tx,
        ephemeral_tx: Some(ephemeral_tx),
    };

    (info, ephemeral_rx)
}
