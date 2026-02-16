//! Composable test strategies for Scribe property-based testing
//!
//! Layered design:
//!   Level 1 (Primitives): did, page_id, segment - re-exported from gurkha
//!   Level 2 (Scribe Structures): layer_name, broadcast payloads
//!   Level 3 (Complex): Full message strategies

#![allow(dead_code)]

use proptest::prelude::*;
use std::collections::HashMap;

// Re-export shared primitives from gurkha
pub use gurkha::test_strategies::{
    did_strategy, expand_pattern_concrete, page_id_strategy, segment_strategy,
};

/// Layer name with 1-3 segments (e.g., "orders", "shop/orders", "shop/orders/customer")
pub fn layer_name_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // 1 segment
        segment_strategy(),
        // 2 segments
        (segment_strategy(), segment_strategy()).prop_map(|(a, b)| format!("{}/{}", a, b)),
        // 3 segments
        (segment_strategy(), segment_strategy(), segment_strategy())
            .prop_map(|(a, b, c)| format!("{}/{}/{}", a, b, c)),
    ]
}

/// Layer update bytes (simulated Loro update)
pub fn layer_update_bytes_strategy() -> impl Strategy<Value = Vec<u8>> {
    // Generate realistic-looking update bytes (4-256 bytes)
    prop::collection::vec(any::<u8>(), 4..256)
}

/// State vector bytes
pub fn state_vector_strategy() -> impl Strategy<Value = Vec<u8>> {
    // State vectors are typically small (8-64 bytes)
    prop::collection::vec(any::<u8>(), 8..64)
}

/// Broadcast layer name (page_id prefixed)
pub fn broadcast_layer_name_strategy() -> impl Strategy<Value = String> {
    (page_id_strategy(), segment_strategy())
        .prop_map(|(page_id, layer)| format!("{}/{}", page_id, layer))
}

/// BroadcastPayload strategy
pub fn broadcast_payload_strategy() -> impl Strategy<Value = crate::BroadcastPayload> {
    (
        page_id_strategy(),
        broadcast_layer_name_strategy(),
        layer_update_bytes_strategy(),
        state_vector_strategy(),
    )
        .prop_map(
            |(page_id, layer_name, update, state_vector)| crate::BroadcastPayload {
                page_id,
                layer_name: layer_name.clone(),
                layer_type: domains::LayerType::from_layer_name(&layer_name),
                update,
                state_vector,
            },
        )
}

/// Peer identity (did, device_id)
pub fn peer_identity_strategy() -> impl Strategy<Value = (String, String)> {
    (did_strategy(), "[a-f0-9]{16}".prop_map(|s| s))
}

/// HashMap of peer vectors
pub fn peer_vectors_strategy() -> impl Strategy<Value = HashMap<String, Vec<u8>>> {
    prop::collection::hash_map(layer_name_strategy(), state_vector_strategy(), 0..5)
}

/// Generate a set of layer names for a page
pub fn page_layers_strategy(page_id: String) -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(segment_strategy(), 1..6).prop_map(move |segments| {
        segments
            .into_iter()
            .map(|s| format!("{}/{}", page_id, s))
            .collect()
    })
}

/// Generate layer names matching shop pattern: {page_id}/orders/{aud}
pub fn shop_order_layer_strategy() -> impl Strategy<Value = (String, String, String)> {
    (page_id_strategy(), did_strategy()).prop_map(|(page_id, customer_did)| {
        let layer = format!("{}/orders/{}", page_id, customer_did);
        (page_id, customer_did, layer)
    })
}
