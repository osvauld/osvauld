//! Permit bindings for Lua
//!
//! Provides permit:page_id(), permit:our_did(), permit:role(), permit:my_name() methods.

use mlua::{UserData, UserDataMethods};

/// Permit bindings for Lua
///
/// **Methods**:
/// - `permit:page_id()` - Get the page ID
/// - `permit:our_did()` - Get our DID
/// - `permit:my_name()` - Get our username
/// - `permit:role()` - Get our role (owner/viewer)
pub struct PermitBindings {
    page_id: String,
    our_did: String,
    our_name: String,
    our_role: String,
}

impl PermitBindings {
    pub fn new(page_id: String, our_did: String, our_name: String, our_role: String) -> Self {
        Self { page_id, our_did, our_name, our_role }
    }
}

impl UserData for PermitBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("page_id", |_, this, ()| {
            Ok(this.page_id.clone())
        });

        methods.add_method("our_did", |_, this, ()| {
            Ok(this.our_did.clone())
        });

        // Alias for our_did (some apps use my_did)
        methods.add_method("my_did", |_, this, ()| {
            Ok(this.our_did.clone())
        });

        methods.add_method("role", |_, this, ()| {
            Ok(this.our_role.clone())
        });

        // Get our username (from signup)
        methods.add_method("my_name", |_, this, ()| {
            Ok(this.our_name.clone())
        });

        // Helper to construct layer paths like "{page_id}/{layer_type}/{my_did}"
        // Example: permit:my_layer("orders") -> "page123/orders/did:key:xyz"
        methods.add_method("my_layer", |_, this, layer_type: String| {
            Ok(format!("{}/{}/{}", this.page_id, layer_type, this.our_did))
        });
    }
}
