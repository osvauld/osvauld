//! Test fixtures for creating signed permits
//!
//! Loads permit templates from test_permits/ directory and creates
//! valid signed permits for integration testing.

use crate::crypto::generate_permit_with_cid;
use crate::parser::Permit;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

/// Deterministic test signing key (32 bytes)
const TEST_KEY: [u8; 32] = [1u8; 32];

/// Load a permit template file from test_permits/
pub fn load_template(filename: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_permits")
        .join(filename);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e))
}

/// Create a permit from a role in a template file
///
/// # Arguments
/// * `filename` - Template file (e.g., "shop.json")
/// * `role` - Role key in the template (e.g., "owner", "customer")
/// * `page_id` - Page ID to inject into facts
/// * `audience` - DID of the permit holder
pub fn create_permit(filename: &str, role: &str, page_id: &str, audience: &str) -> Permit {
    let template = load_template(filename);
    let role_template = template[role].as_object()
        .unwrap_or_else(|| panic!("Missing role '{}' in {}", role, filename));

    // Build facts from template
    let mut facts = serde_json::Map::new();

    // Copy all fields from role template
    for (key, value) in role_template {
        facts.insert(key.clone(), value.clone());
    }

    // Inject page_id
    facts.insert("page_id".to_string(), json!(page_id));

    let (token, _cid) = pollster::block_on(generate_permit_with_cid(
        &TEST_KEY,
        audience,
        vec![],              // No capabilities (facts-only architecture)
        Some(facts),
        None,                // No expiry
        vec![],              // No proofs
        HashMap::new(),      // No proof tokens
    ))
    .expect("Failed to create test permit");

    Permit::from_token(&token).expect("Failed to parse test permit")
}

/// Create a permit token string from a role in a template file
///
/// Same as `create_permit` but returns the raw token string instead of parsed Permit.
/// Useful for tests that need to pass the token to Subscribe messages.
pub fn create_permit_token(filename: &str, role: &str, page_id: &str, audience: &str) -> String {
    let template = load_template(filename);
    let role_template = template[role].as_object()
        .unwrap_or_else(|| panic!("Missing role '{}' in {}", role, filename));

    // Build facts from template
    let mut facts = serde_json::Map::new();

    // Copy all fields from role template
    for (key, value) in role_template {
        facts.insert(key.clone(), value.clone());
    }

    // Inject page_id
    facts.insert("page_id".to_string(), json!(page_id));

    let (token, _cid) = pollster::block_on(generate_permit_with_cid(
        &TEST_KEY,
        audience,
        vec![],              // No capabilities (facts-only architecture)
        Some(facts),
        None,                // No expiry
        vec![],              // No proofs
        HashMap::new(),      // No proof tokens
    ))
    .expect("Failed to create test permit");

    token
}

// Shop scenario helpers

/// Create shop owner permit
pub fn shop_owner(page_id: &str, did: &str) -> Permit {
    create_permit("shop.json", "owner", page_id, did)
}

/// Create shop customer permit
pub fn shop_customer(page_id: &str, did: &str) -> Permit {
    create_permit("shop.json", "customer", page_id, did)
}

/// Create shop admin permit
pub fn shop_admin(page_id: &str, did: &str) -> Permit {
    create_permit("shop.json", "admin", page_id, did)
}

// Booking scenario helpers

/// Create booking provider permit (owner of the business)
pub fn booking_provider(page_id: &str, did: &str) -> Permit {
    create_permit("booking.json", "provider", page_id, did)
}

/// Create booking customer permit
pub fn booking_customer(page_id: &str, did: &str) -> Permit {
    create_permit("booking.json", "customer", page_id, did)
}

// Collaborative scenario helpers

/// Create collaborative document owner permit
pub fn collab_owner(page_id: &str, did: &str) -> Permit {
    create_permit("collaborative.json", "owner", page_id, did)
}

/// Create collaborative document collaborator permit
pub fn collab_collaborator(page_id: &str, did: &str) -> Permit {
    create_permit("collaborative.json", "collaborator", page_id, did)
}

// Game scenario helpers

/// Create game host permit
pub fn game_host(page_id: &str, did: &str) -> Permit {
    create_permit("game.json", "host", page_id, did)
}

/// Create game player permit
pub fn game_player(page_id: &str, did: &str) -> Permit {
    create_permit("game.json", "player", page_id, did)
}

// Edge case helpers

/// Create edge case permit
pub fn edge_case(role: &str, page_id: &str, did: &str) -> Permit {
    create_permit("edge_cases.json", role, page_id, did)
}

// Handshake scenario helpers

/// Create owner first connection permit (can publish, first_connection=true)
pub fn handshake_owner_first(page_id: &str, did: &str) -> Permit {
    create_permit("handshake.json", "owner_first_connection", page_id, did)
}

/// Create owner reconnection permit (can publish, first_connection=false)
pub fn handshake_owner_reconnect(page_id: &str, did: &str) -> Permit {
    create_permit("handshake.json", "owner_reconnection", page_id, did)
}

/// Create viewer permit (cannot publish)
pub fn handshake_viewer(page_id: &str, did: &str) -> Permit {
    create_permit("handshake.json", "viewer", page_id, did)
}

/// Create viewer first connection permit
pub fn handshake_viewer_first(page_id: &str, did: &str) -> Permit {
    create_permit("handshake.json", "viewer_first_connection", page_id, did)
}

// Chat scenario helpers (presence + ephemerals)

/// Create chat owner permit (visible, all ephemerals)
pub fn chat_owner(page_id: &str, did: &str) -> Permit {
    create_permit("chat.json", "owner", page_id, did)
}

/// Create chat owner permit token
pub fn chat_owner_token(page_id: &str, did: &str) -> String {
    create_permit_token("chat.json", "owner", page_id, did)
}

/// Create chat participant permit (visible, typing/cursor ephemerals)
pub fn chat_participant(page_id: &str, did: &str) -> Permit {
    create_permit("chat.json", "participant", page_id, did)
}

/// Create chat participant permit token
pub fn chat_participant_token(page_id: &str, did: &str) -> String {
    create_permit_token("chat.json", "participant", page_id, did)
}

/// Create chat lurker permit (invisible, no ephemerals)
pub fn chat_lurker(page_id: &str, did: &str) -> Permit {
    create_permit("chat.json", "lurker", page_id, did)
}

/// Create chat lurker permit token
pub fn chat_lurker_token(page_id: &str, did: &str) -> String {
    create_permit_token("chat.json", "lurker", page_id, did)
}
