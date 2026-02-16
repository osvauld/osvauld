//! Butler Models
//!
//! Re-exports domain models and provides extension functions that require
//! butler-specific dependencies (gurkha, herald).

// Re-export all types from domains
pub use domains::{
    json_to_loro_value, loro_value_to_json, Argon2Params, AssetAck, AssetMetadata, AssetReady,
    AssetRequest, ConnectionDeviceInfo, ConnectionString, ConnectionType, ContactData, ContactType,
    DecryptedPage, DeviceData, DeviceInfo, EncryptedKeyStore, IdentityData, Layer, LayerError,
    OwnerInfo, Page, PageData, PageMeta, PreparedPage, QueryChange, QueryDelta, QueryResult,
    QuerySpec, QuerySubscription, SortOrder, SovereignNode, Space, SpaceData, SpaceMeta, UserInfo,
};

/// Context for launching an app in a renderer
///
/// **Context**: Extracted from Butler identity + page permit for a given page.
/// Both renderers need this to initialize LuaRuntime or game loop.
#[derive(Debug, Clone)]
pub struct AppContext {
    pub user_did: String,
    pub user_name: String,
    pub user_role: String,
}

// Extension functions that require gurkha/herald dependencies
use base64::{engine::general_purpose, Engine as _};

/// Extension trait for ConnectionString that requires gurkha
pub trait ConnectionStringExt {
    /// Extract space_id from the permit (for viewer connections)
    ///
    /// Parses the UCAN permit and extracts the space_id fact.
    fn space_id(&self) -> Result<String, String>;

    /// Get the node's DID from its public key
    ///
    /// Converts the base64 public key to DID format for storage.
    fn node_did(&self) -> Result<String, String>;
}

impl ConnectionStringExt for ConnectionString {
    fn space_id(&self) -> Result<String, String> {
        let permit = gurkha::Permit::from_token(&self.permit)
            .map_err(|e| format!("Invalid permit: {}", e))?;
        permit
            .space_id()
            .map(|s| s.to_string())
            .ok_or_else(|| "Permit missing space_id".to_string())
    }

    fn node_did(&self) -> Result<String, String> {
        herald::Identity::did_from_base64_pubkey(&self.node_public_key)
            .map_err(|e| format!("Invalid node public key: {}", e))
    }
}

/// Extension trait for SovereignNode that requires herald
pub trait SovereignNodeExt {
    /// Create a new SovereignNode from a connection string
    ///
    /// Derives node_id from device_public_key (base64 -> hex)
    /// Converts node_public_key (base64) to DID format for storage
    fn from_connection_string(conn: &ConnectionString, connection_type: ConnectionType) -> Self;
}

impl SovereignNodeExt for SovereignNode {
    fn from_connection_string(conn: &ConnectionString, connection_type: ConnectionType) -> Self {
        // Derive node_id from device_public_key (base64 -> bytes -> hex)
        // This matches iroh's NodeId format: lowercase hex of the Ed25519 public key
        let node_id = general_purpose::STANDARD
            .decode(&conn.device_public_key)
            .map(|bytes| bytes.iter().map(|b| format!("{:02x}", b)).collect())
            .unwrap_or_else(|_| String::new());

        // Convert base64 public key to DID format for consistent storage
        let did = herald::Identity::did_from_base64_pubkey(&conn.node_public_key)
            .expect("Invalid public key in connection string");

        SovereignNode::new(
            did,
            conn.node_encryption_key.clone(),
            conn.name.clone(),
            node_id,
            conn.device_public_key.clone(),
            Some(conn.permit.clone()),
            conn.relay.clone(),
            connection_type,
        )
    }
}
