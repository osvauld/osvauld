//! Storage traits for Scribe actor
//!
//! Butler implements these using RedbStore + encryption.
//! Scribe only knows about the abstract interface.
//!
//! ## Design
//!
//! These traits replace the closure-based storage injection pattern:
//! - `LayerStorage` replaces `SaveLayerFn`
//! - `PeerVectorStorage` replaces `LoadPeerVectorFn` + `SavePeerVectorFn`
//! - `PeerResolver` replaces `ListAuthorizedUsersFn` + `LoadUserPermitFn`
//!
//! Each trait is scoped to a single page (page_id passed at construction time).

use std::collections::HashMap;
use std::sync::Arc;

use crate::Result;

// Layer Storage Trait

/// Layer persistence (scoped to a page)
///
/// **Implemented by**: Butler's RedbStore + encryption layer
/// **Used by**: Scribe flush handler, immediate app layer saves
pub trait LayerStorage: Send + Sync {
    /// Save layer snapshot
    ///
    /// **Context**: Called during periodic flush or immediate save (app layers)
    /// **Implementor**: Should encrypt data before persistence
    fn save_layer(&self, layer_name: &str, data: &[u8]) -> Result<()>;
}

// Peer Vector Storage Trait

/// Peer state vector persistence (scoped to a page)
///
/// **Implemented by**: Butler's RedbStore
/// **Used by**: Scribe subscription handling, unsubscription, flush
///
/// State vectors track what each peer has already received.
/// This enables incremental sync (send only what they're missing).
pub trait PeerVectorStorage: Send + Sync {
    /// Load peer's state vectors for all layers
    ///
    /// **Context**: Called when peer subscribes to restore their last known state
    /// **Returns**: HashMap of layer_name -> state_vector bytes, or None if first connection
    fn load_vectors(
        &self,
        user_did: &str,
        device_id: &str,
    ) -> Result<Option<HashMap<String, Vec<u8>>>>;

    /// Save peer's state vectors for all layers
    ///
    /// **Context**: Called during flush and on unsubscribe
    /// **Note**: Vectors are stored as-is (Loro's binary format)
    fn save_vectors(
        &self,
        user_did: &str,
        device_id: &str,
        vectors: &HashMap<String, Vec<u8>>,
    ) -> Result<()>;
}

// Peer Resolver Trait (Node Mode Only)

/// Peer authorization (node mode only)
///
/// **Implemented by**: Butler's permit store
/// **Used by**: Scribe for node-mode peer lookup and validation
///
/// Only used when Scribe runs on a node (is_node=true).
/// Allows looking up authorized users and their permits.
pub trait PeerResolver: Send + Sync {
    /// List all authorized user_dids for this page
    ///
    /// **Context**: Node mode sync - finding peers to broadcast to
    /// **Returns**: List of DIDs that have permits for this page
    fn list_authorized_users(&self) -> Vec<String>;

    /// Load a user's permit for this page
    ///
    /// **Context**: Node mode - getting permit for validation/role extraction
    /// **Returns**: Permit token string if user has access, None otherwise
    fn load_user_permit(&self, user_did: &str) -> Option<String>;
}

// Permit Issuer Trait (Node Mode Only)

/// Layer permit issuance (node mode only, scoped to a page)
///
/// **Implemented by**: Butler using signing key + gurkha
/// **Used by**: Scribe layer_unit for issuing layer permits on dynamic layer detection
///
/// Scribe must never hold signing keys. This trait abstracts permit creation
/// so Scribe can request permits without accessing crypto directly.
pub trait PermitIssuer: Send + Sync {
    /// Issue a layer permit for a single dynamic layer
    ///
    /// **Context**: Node detected new dynamic layer, needs to issue permits
    /// **Returns**: (permit_token, cid) — ready to store and send
    fn issue_layer_permit(
        &self,
        audience: &str,
        layer_name: &str,
        config: gurkha::LayerConfig,
        intent_cid: Option<&str>,
    ) -> Result<(String, String)>;

    /// List layer authority permits applicable to a specific audience on this page.
    ///
    /// **Context**: Subscription-time issuance computes missing layer access permits
    /// from creator->node authority permits.
    /// **Returns**: Vec of (layer_name, version, authority_permit_token).
    fn list_layer_authority_permits_for_audience(
        &self,
        audience: &str,
    ) -> Result<Vec<(String, u64, String)>>;

    /// Get latest layer authority permit for page/layer/audience.
    fn get_layer_authority_permit(
        &self,
        audience: &str,
        layer_name: &str,
    ) -> Result<Option<(u64, String)>>;

    /// Check if this audience already has a stored layer access permit.
    fn has_layer_access_permit(&self, audience: &str, layer_name: &str) -> Result<bool>;

    /// Issue and persist a layer authority permit for this page/layer/audience.
    ///
    /// **Context**: Creator-side dynamic layer creation (viewer/owner) and
    /// node-side authority workflows.
    fn issue_layer_authority_permit(
        &self,
        audience: &str,
        layer_name: &str,
        config: gurkha::LayerConfig,
        authorized_peers: Option<Vec<String>>,
        version: u64,
    ) -> Result<(String, String)>;
}

// Type Aliases for Convenience

/// Reference-counted layer storage
pub type LayerStorageRef = Arc<dyn LayerStorage>;

/// Reference-counted peer vector storage
pub type PeerVectorStorageRef = Arc<dyn PeerVectorStorage>;

/// Reference-counted peer resolver
pub type PeerResolverRef = Arc<dyn PeerResolver>;

/// Reference-counted permit issuer
pub type PermitIssuerRef = Arc<dyn PermitIssuer>;

// Null Implementations (for testing)

/// Null layer storage that discards all saves
///
/// Useful for testing when persistence isn't needed
pub struct NullLayerStorage;

impl LayerStorage for NullLayerStorage {
    fn save_layer(&self, _layer_name: &str, _data: &[u8]) -> Result<()> {
        Ok(())
    }
}

/// Null peer vector storage that stores nothing
///
/// Useful for testing when peer tracking isn't needed
pub struct NullPeerVectorStorage;

impl PeerVectorStorage for NullPeerVectorStorage {
    fn load_vectors(
        &self,
        _user_did: &str,
        _device_id: &str,
    ) -> Result<Option<HashMap<String, Vec<u8>>>> {
        Ok(None)
    }

    fn save_vectors(
        &self,
        _user_did: &str,
        _device_id: &str,
        _vectors: &HashMap<String, Vec<u8>>,
    ) -> Result<()> {
        Ok(())
    }
}

/// Null peer resolver that has no authorized users
///
/// Useful for testing non-node mode
pub struct NullPeerResolver;

impl PeerResolver for NullPeerResolver {
    fn list_authorized_users(&self) -> Vec<String> {
        Vec::new()
    }

    fn load_user_permit(&self, _user_did: &str) -> Option<String> {
        None
    }
}

/// Null permit issuer that returns test tokens
///
/// Useful for testing when permit issuance isn't needed
pub struct NullPermitIssuer;

impl PermitIssuer for NullPermitIssuer {
    fn issue_layer_permit(
        &self,
        _audience: &str,
        _layer_name: &str,
        _config: gurkha::LayerConfig,
        _intent_cid: Option<&str>,
    ) -> Result<(String, String)> {
        Ok(("test-token".into(), "test-cid".into()))
    }

    fn list_layer_authority_permits_for_audience(
        &self,
        _audience: &str,
    ) -> Result<Vec<(String, u64, String)>> {
        Ok(Vec::new())
    }

    fn get_layer_authority_permit(
        &self,
        _audience: &str,
        _layer_name: &str,
    ) -> Result<Option<(u64, String)>> {
        Ok(None)
    }

    fn has_layer_access_permit(&self, _audience: &str, _layer_name: &str) -> Result<bool> {
        Ok(false)
    }

    fn issue_layer_authority_permit(
        &self,
        _audience: &str,
        _layer_name: &str,
        _config: gurkha::LayerConfig,
        _authorized_peers: Option<Vec<String>>,
        _version: u64,
    ) -> Result<(String, String)> {
        Ok(("test-authority-token".into(), "test-authority-cid".into()))
    }
}
