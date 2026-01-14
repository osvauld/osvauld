//! Permit templates for space creation
//!
//! These templates define the permission structure for spaces.
//! Uses capability-based design with peer_capabilities for protocol decisions.

/// Space permit template
///
/// - owner_template: What the owner can do and delegate
/// - node_template: What the node can do and delegate back to owner
///
/// peer_capabilities control protocol-level behavior:
/// - relay: Can forward data to other peers
/// - share: Can issue delegated permits
/// - accept_publish: Can accept published spaces from this peer
pub const SPACE_TEMPLATE: &str = r#"{
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
                "peer_capabilities": {
                    "relay": true,
                    "share": true,
                    "accept_publish": true
                },
                "operations": {
                    "get_share_link": "allow",
                    "add_pages": "allow",
                    "share_space": "allow"
                },
                "auth_capabilities": {
                    "can_connect": true,
                    "persist_share": true,
                    "can_delegate": true,
                    "sync_enabled": true
                },
                "relationship": "node",
                "issue_on": {
                    "viewer": {
                        "token_type": "space_viewer",
                        "peer_capabilities": {
                            "relay": false,
                            "share": false,
                            "accept_publish": false
                        },
                        "operations": {
                            "request_pages": "allow",
                            "get_share_link": "allow"
                        },
                        "auth_capabilities": {
                            "can_connect": true,
                            "sync_enabled": false
                        },
                        "relationship": "viewer"
                    }
                }
            },
            "viewer": {
                "token_type": "space_viewer",
                "peer_capabilities": {
                    "relay": false,
                    "share": false,
                    "accept_publish": false
                },
                "operations": {
                    "request_pages": "allow",
                    "get_share_link": "allow"
                },
                "auth_capabilities": {
                    "can_connect": true,
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
        "peer_capabilities": {
            "relay": true,
            "share": true,
            "accept_publish": true
        },
        "issue_on": {
            "owner": {
                "token_type": "space_node_share",
                "peer_capabilities": {
                    "relay": false,
                    "share": true,
                    "accept_publish": true
                },
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
