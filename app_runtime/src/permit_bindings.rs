//! Permit bindings for Lua
//!
//! Exposes permit information to app Lua code.
//! Provides identity and layer naming helpers.

use mlua::{UserData, UserDataMethods};

/// Lua permit bindings
///
/// **Methods**:
/// - `permit:my_did()` - Returns the viewer's DID
/// - `permit:page_id()` - Returns the page ID
/// - `permit:my_layer(suffix)` - Returns a namespaced layer name: `{page_id}/{suffix}/{my_did}`
/// - `permit:issuer()` - Returns the permit issuer's DID
/// - `permit:role()` - Returns the role from permit facts
pub struct PermitBindings {
    /// Page ID (e.g., "my-shop")
    page_id: String,
    /// Viewer's DID (audience of the permit)
    my_did: String,
    /// Issuer's DID (owner who issued the permit)
    issuer_did: String,
    /// Role from permit facts (e.g., "customer", "admin")
    role: String,
}

impl PermitBindings {
    /// Create permit bindings
    ///
    /// **Note**: In production, these values come from the actual permit.
    /// For now, we generate a pseudo-DID for development.
    pub fn new(page_id: &str, app_name: &str) -> Self {
        // Determine role from app name (simple heuristic for dev)
        let role = if app_name.to_lowercase().contains("owner")
            || app_name.to_lowercase().contains("admin") {
            "owner".to_string()
        } else {
            "customer".to_string()
        };

        // Generate pseudo-DIDs for development
        // In production, these come from actual permits
        let my_did = format!("did:key:{}-{}", role, &page_id[..8.min(page_id.len())]);
        let issuer_did = format!("did:key:owner-{}", &page_id[..8.min(page_id.len())]);

        Self {
            page_id: page_id.to_string(),
            my_did,
            issuer_did,
            role,
        }
    }

    /// Create permit bindings with explicit values
    pub fn with_values(page_id: String, my_did: String, issuer_did: String, role: String) -> Self {
        Self {
            page_id,
            my_did,
            issuer_did,
            role,
        }
    }
}

impl UserData for PermitBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // permit:page_id() -> string
        methods.add_method("page_id", |_, this, ()| {
            Ok(this.page_id.clone())
        });

        // permit:my_did() -> string
        methods.add_method("my_did", |_, this, ()| {
            Ok(this.my_did.clone())
        });

        // permit:issuer() -> string
        methods.add_method("issuer", |_, this, ()| {
            Ok(this.issuer_did.clone())
        });

        // permit:role() -> string
        methods.add_method("role", |_, this, ()| {
            Ok(this.role.clone())
        });

        // permit:my_layer(suffix) -> string
        // Returns a namespaced layer name: {page_id}/{suffix}/{my_did}
        // Example: "my-shop/orders/did:key:customer-abc123"
        methods.add_method("my_layer", |_, this, suffix: String| {
            Ok(format!("{}/{}/{}", this.page_id, suffix, this.my_did))
        });
    }
}
