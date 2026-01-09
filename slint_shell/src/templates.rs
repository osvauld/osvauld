//! Permit templates for space creation
//!
//! These templates define the permission structure for spaces.

/// Space permit template
///
/// - owner_template: What the owner can do and delegate
/// - node_template: What the node can do and delegate back to owner
pub const SPACE_TEMPLATE: &str = r#"{
    "owner_template": {
        "operations": {
            "own": "allow",
            "get_share_link": "allow",
            "add_pages": "allow",
            "share_space": "allow"
        },
        "delegation": {
            "node": {
                "token_type": "space_share",
                "operations": {
                    "get_share_link": "allow",
                    "add_pages": "allow",
                    "share_space": "allow"
                },
                "auth_capabilities": {
                    "can_connect": true,
                    "persist_share": true,
                    "can_delegate": false,
                    "sync_enabled": true
                },
                "relationship": "node"
            },
            "viewer": {
                "token_type": "space_viewer",
                "operations": {
                    "request_pages": "allow",
                    "get_share_link": "allow"
                },
                "auth_capabilities": {
                    "can_connect": true,
                    "persist_share": false,
                    "can_delegate": false,
                    "sync_enabled": false
                },
                "relationship": "viewer"
            }
        }
    },
    "node_template": {
        "operations": {
            "sync": "allow",
            "share_space": "allow"
        },
        "delegation": {
            "owner": {
                "token_type": "space_node_share",
                "operations": {
                    "sync": "allow"
                },
                "auth_capabilities": {
                    "can_connect": true,
                    "sync_enabled": true
                },
                "relationship": "owner"
            }
        }
    }
}"#;
