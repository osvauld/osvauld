//! Test fixtures and constants for integration tests

use std::time::Duration;

// =============================================================================
// Timing constants for test synchronization
// =============================================================================

/// Delay after spawning actors to let them initialize
pub const ACTOR_SPAWN_DELAY: Duration = Duration::from_millis(50);

/// Delay for handshake completion between peers
pub const HANDSHAKE_DELAY: Duration = Duration::from_millis(500);

/// Delay for message delivery between peers
pub const MESSAGE_DELIVERY_DELAY: Duration = Duration::from_millis(800);

/// Delay for page sync operations
pub const PAGE_SYNC_DELAY: Duration = Duration::from_millis(1500);

/// Extended delay for complex sync operations (viewer handshake + page sync)
pub const EXTENDED_SYNC_DELAY: Duration = Duration::from_millis(3000);

/// Delay to wait for Scribe's periodic flush to storage (flush interval is 10s)
/// This must be >= 10 seconds to ensure dirty layers are persisted
pub const STORAGE_FLUSH_DELAY: Duration = Duration::from_millis(11000);

/// Timeout for asset blob transfer (metadata sync + blob download)
/// Asset sync flow: SyncOffer → SyncAck → AssetPrepare → AssetReady → AssetAck
pub const ASSET_SYNC_DELAY: Duration = Duration::from_millis(5000);

// =============================================================================
// Test template for space creation
// =============================================================================

/// Standard test template for creating spaces.
/// Includes owner operations and delegation templates for node and viewer roles.
/// Uses capability-based design with peer_capabilities for protocol decisions.
pub const TEST_SPACE_TEMPLATE: &str = r#"{
  "owner_template": {
    "operations": {
      "own": "allow",
      "get_share_link": "allow",
      "add_pages": "allow",
      "share_space": "allow"
    },
    "peer_capabilities": {
      "relay": false,
      "share": true,
      "accept_publish": true
    },
    "issue_on": {
      "node": {
        "token_type": "space_share",
        "peer_capabilities": { "relay": true, "share": true, "accept_publish": true },
        "operations": { "get_share_link": "allow", "add_pages": "allow", "share_space": "allow" },
        "auth_capabilities": { "can_connect": true, "persist_share": true, "can_delegate": false, "sync_enabled": true },
        "relationship": "node",
        "issue_on": {
          "viewer": {
            "token_type": "space_viewer",
            "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
            "operations": { "request_pages": "allow", "get_share_link": "allow" },
            "auth_capabilities": { "can_connect": true, "persist_share": false, "can_delegate": false, "sync_enabled": false },
            "relationship": "viewer"
          }
        }
      },
      "viewer": {
        "token_type": "space_viewer",
        "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
        "operations": { "request_pages": "allow", "get_share_link": "allow" },
        "auth_capabilities": { "can_connect": true, "persist_share": false, "can_delegate": false, "sync_enabled": false },
        "relationship": "viewer"
      }
    }
  }
}"#;

// =============================================================================
// Test template for page creation
// =============================================================================

/// Standard test template for creating pages.
/// Includes layer definitions and delegation templates for node and viewer roles.
/// Uses capability-based design with peer_capabilities for protocol decisions.
pub const TEST_PAGE_TEMPLATE: &str = r#"{
  "owner_template": {
    "operations": { "own": "allow", "share_page": "allow" },
    "peer_capabilities": { "relay": false, "share": true, "accept_publish": true },
    "layers": {
      "template_doc": { "sync": true, "write": true, "type": "crdt" },
      "content_doc": { "sync": true, "write": true, "type": "crdt" },
      "user_content_doc": { "sync": true, "write": true, "type": "crdt" },
      "collaborative_doc": { "sync": true, "write": true, "type": "crdt" },
      "submissions_doc": { "sync": true, "write": true, "type": "crdt" },
      "static_assets": { "sync": true, "write": true, "type": "asset" }
    },
    "layer_patterns": {
      "{page_id}/assets": { "create": true, "sync": true }
    },
    "sync": { "local_only": ["user_content_doc"] },
    "issue_on": {
      "node": {
        "token_type": "page_share",
        "peer_capabilities": { "relay": true, "share": true, "accept_publish": true },
        "operations": { "share_page": "allow" },
        "layers": {
          "template_doc": { "sync": true, "write": true, "type": "crdt" },
          "content_doc": { "sync": true, "write": true, "type": "crdt" },
          "user_content_doc": { "sync": true, "write": true, "type": "crdt" },
          "collaborative_doc": { "sync": true, "write": true, "type": "crdt" },
          "submissions_doc": { "sync": true, "write": true, "type": "crdt" },
          "static_assets": { "sync": true, "write": true, "type": "asset" }
        },
        "layer_patterns": {
          "{page_id}/assets": { "create": true, "sync": true }
        },
        "sync": { "local_only": ["user_content_doc"] },
        "auth_capabilities": { "can_connect": true, "persist_share": true, "can_delegate": false, "sync_enabled": true },
        "relationship": "node",
        "issue_on": {
          "viewer": {
            "token_type": "page_viewer",
            "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
            "operations": {},
            "layers": {
              "template_doc": { "sync": true, "write": false, "type": "crdt" },
              "content_doc": { "sync": true, "write": false, "type": "crdt" },
              "collaborative_doc": { "sync": true, "write": true, "type": "crdt" },
              "submissions_doc": { "sync": true, "write": true, "type": "crdt" },
              "static_assets": { "sync": true, "write": false, "type": "asset" }
            },
            "layer_patterns": {
              "{page_id}/assets": { "create": false, "sync": true }
            },
            "sync": { "local_only": ["user_content_doc"], "no_incoming_updates": ["submissions_doc"], "send_full_snapshot": ["submissions_doc"] },
            "auth_capabilities": { "can_connect": true, "persist_share": false, "can_delegate": false, "sync_enabled": false },
            "relationship": "viewer"
          }
        }
      },
      "viewer": {
        "token_type": "page_viewer",
        "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
        "operations": {},
        "layers": {
          "template_doc": { "sync": true, "write": false, "type": "crdt" },
          "content_doc": { "sync": true, "write": false, "type": "crdt" },
          "collaborative_doc": { "sync": true, "write": true, "type": "crdt" },
          "submissions_doc": { "sync": true, "write": true, "type": "crdt" },
          "static_assets": { "sync": true, "write": false, "type": "asset" }
        },
        "layer_patterns": {
          "{page_id}/assets": { "create": false, "sync": true }
        },
        "sync": { "local_only": ["user_content_doc"], "no_incoming_updates": ["submissions_doc"], "send_full_snapshot": ["submissions_doc"] },
        "auth_capabilities": { "can_connect": true, "persist_share": false, "can_delegate": false, "sync_enabled": false },
        "relationship": "viewer"
      }
    }
  }
}"#;

// =============================================================================
// Page layer definitions
// =============================================================================

/// Standard page layers for testing.
/// Matches the layers defined in TEST_PAGE_TEMPLATE.
pub const TEST_PAGE_LAYERS: &[&str] = &[
    "template_doc",
    "content_doc",
    "user_content_doc",
    "collaborative_doc",
    "submissions_doc",
    "static_assets",
];

// =============================================================================
// Sync consent templates for viewer-issued permits
// =============================================================================

/// Viewer-issued space consent template.
/// Issued by viewer to node to express consent for receiving space sync updates.
pub const TEST_SYNC_SPACE_CONSENT_TEMPLATE: &str = r#"{
  "consent_template": {
    "token_type": "sync_space_consent",
    "operations": {
      "receive_pages": "allow",
      "receive_updates": "allow"
    },
    "auth_capabilities": {
      "accept_sync": true,
      "accept_new_pages": true
    },
    "relationship": "sync_consent",
    "cel_rules": {
      "is_sync_consent": "token_type == 'sync_space_consent' && relationship == 'sync_consent'",
      "can_accept_sync": "auth_capabilities.accept_sync == true",
      "can_accept_pages": "auth_capabilities.accept_new_pages == true"
    },
    "functions": {
      "can_send_sync": "self.token_type == 'sync_space_consent' && self.space_id == context.space_id",
      "can_send_new_page": "self.auth_capabilities.accept_new_pages == true && self.space_id == context.space_id"
    }
  }
}"#;

/// Viewer-issued page consent template.
/// Issued by viewer to node to express consent for receiving page layer updates.
pub const TEST_SYNC_PAGE_CONSENT_TEMPLATE: &str = r#"{
  "consent_template": {
    "token_type": "sync_page_consent",
    "operations": {
      "receive_layer_updates": "allow"
    },
    "layers": {
      "template_doc": { "sync": true, "write": false, "type": "crdt" },
      "content_doc": { "sync": true, "write": false, "type": "crdt" },
      "collaborative_doc": { "sync": true, "write": true, "type": "crdt" },
      "submissions_doc": { "sync": true, "write": true, "type": "crdt" },
      "static_assets": { "sync": true, "write": false, "type": "asset" }
    },
    "sync": {
      "no_incoming_updates": ["submissions_doc"]
    },
    "auth_capabilities": {
      "accept_sync": true
    },
    "relationship": "sync_consent",
    "cel_rules": {
      "is_sync_consent": "token_type == 'sync_page_consent' && relationship == 'sync_consent'",
      "can_accept_layer": "has(layers[context.layer]) && layers[context.layer].sync == true"
    },
    "functions": {
      "can_send_layer": "self.token_type == 'sync_page_consent' && self.page_id == context.page_id && has(self.layers[context.layer])",
      "is_my_consent": "self.iss == context.our_pubkey && self.aud == context.their_pubkey"
    }
  }
}"#;
