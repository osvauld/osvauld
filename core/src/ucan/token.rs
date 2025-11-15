use super::parser::{DelegationTemplate, ResourceUcan, ResourceUcanError, ResourceUcanResult};
use super::types::{Capability, ConnectionTokenType, ResourceTokenType, Role};
use std::collections::HashMap;
use std::result::Result as StdResult;
use ucan::Ucan;

// ============================================================================
// Trait Definitions - Core UCAN behaviors
// ============================================================================

/// Base trait for all UCAN tokens
/// Provides access to raw token, role, and parsed UCAN
pub trait UcanToken {
    fn raw_token(&self) -> &str;
    fn role(&self) -> Role;
    fn parsed(&self) -> &Ucan;
}

/// Tokens with extractable IDs (from facts)
pub trait HasId: UcanToken {
    fn id(&self) -> Result<String, ResourceUcanError>;
}

/// Tokens that can delegate to child tokens
pub trait CanDelegate: UcanToken {
    fn delegation_template(&self, role: Role) -> Result<&DelegationTemplate, ResourceUcanError>;

    fn can_delegate_to(&self, role: Role) -> bool {
        self.delegation_template(role).is_ok()
    }
}

/// Resource-specific operations
pub trait ResourceOps: HasId {
    fn resource_id(&self) -> Result<String, ResourceUcanError> {
        self.id()
    }

    fn doc_capabilities(&self) -> &HashMap<String, Capability>;

    fn has_capability(&self, doc: &str, cap: Capability) -> bool {
        self.doc_capabilities()
            .get(doc)
            .map(|c| c >= &cap)
            .unwrap_or(false)
    }
}

/// Folder-specific operations
pub trait FolderOps: HasId {
    fn folder_id(&self) -> Result<String, ResourceUcanError> {
        self.id()
    }

    fn folder_operations(&self) -> Vec<String>;

    fn has_operation(&self, op: &str) -> bool {
        self.folder_operations().contains(&op.to_string())
    }
}

// ============================================================================
// Connection Token Types (Device-to-device authentication)
// ============================================================================

/// Error type for connection token operations
#[derive(Debug)]
pub enum ConnectionTokenError {
    InvalidTokenType(String),
    ParsingFailed(String),
    MissingField(String),
    ValidationFailed(String),
}

impl std::fmt::Display for ConnectionTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionTokenError::InvalidTokenType(msg) => write!(f, "Invalid token type: {}", msg),
            ConnectionTokenError::ParsingFailed(msg) => write!(f, "Parsing failed: {}", msg),
            ConnectionTokenError::MissingField(msg) => write!(f, "Missing field: {}", msg),
            ConnectionTokenError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
        }
    }
}

impl std::error::Error for ConnectionTokenError {}

pub type ConnectionTokenResult<T> = StdResult<T, ConnectionTokenError>;

/// Parsed connection UCAN with domain logic
#[derive(Debug, Clone)]
pub struct ConnectionToken {
    raw_token: String,
    parsed: Ucan,
    token_type: ConnectionTokenType,
    role: Role,
    first_connection: bool,
}

impl ConnectionToken {
    /// Parse connection UCAN token
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        // Parse UCAN token
        let parsed = Ucan::try_from(token)
            .map_err(|e| ConnectionTokenError::ParsingFailed(format!("UCAN parsing error: {}", e)))?;

        // Extract facts
        let facts = parsed.facts().as_ref().ok_or_else(|| {
            ConnectionTokenError::MissingField("UCAN facts not found".to_string())
        })?;

        // Extract token_type from facts
        let token_type_str = facts
            .get("token_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ConnectionTokenError::MissingField("token_type not found in facts".to_string()))?;

        let token_type = ConnectionTokenType::from_str(token_type_str)
            .map_err(|e| ConnectionTokenError::InvalidTokenType(e))?;

        // Extract role from facts
        let role_str = facts
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ConnectionTokenError::MissingField("role not found in facts".to_string()))?;

        let role = Role::from_str(role_str)
            .map_err(|e| ConnectionTokenError::ParsingFailed(format!("Invalid role: {}", e)))?;

        // Extract first_connection flag (defaults to false if not present)
        let first_connection = facts
            .get("first_connection")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(Self {
            raw_token: token.to_string(),
            parsed,
            token_type,
            role,
            first_connection,
        })
    }

    // Accessors
    pub fn raw_token(&self) -> &str {
        &self.raw_token
    }

    pub fn token_type(&self) -> ConnectionTokenType {
        self.token_type
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn parsed(&self) -> &Ucan {
        &self.parsed
    }

    pub fn is_first_connection(&self) -> bool {
        self.first_connection
    }

    // Validation
    pub fn is_one_time(&self) -> bool {
        self.token_type.is_one_time()
    }

    pub fn can_delegate_to(&self, target_role: Role) -> bool {
        match self.token_type {
            ConnectionTokenType::OneTimeConnection => false, // One-time tokens can't delegate
            ConnectionTokenType::OwnerConnection => true,   // Owner can delegate to anyone
            ConnectionTokenType::NodeConnection => matches!(target_role, Role::User | Role::Viewer),
            ConnectionTokenType::UserConnection => matches!(target_role, Role::Viewer),
            ConnectionTokenType::ViewerAuth => false,        // Viewer auth can't delegate
            ConnectionTokenType::ViewerConnection => false,  // Viewer connection can't delegate
        }
    }
}

// Implement UcanToken trait for ConnectionToken
impl UcanToken for ConnectionToken {
    fn raw_token(&self) -> &str {
        &self.raw_token
    }

    fn role(&self) -> Role {
        self.role
    }

    fn parsed(&self) -> &Ucan {
        &self.parsed
    }
}

/// One-time connection token (for initial handshake)
#[derive(Debug, Clone)]
pub struct OneTimeConnectionToken(ConnectionToken);

impl OneTimeConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::OneTimeConnection {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected OneTimeConnection, got {:?}", conn.token_type())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}

/// Owner connection token
#[derive(Debug, Clone)]
pub struct OwnerConnectionToken(ConnectionToken);

impl OwnerConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::OwnerConnection {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected OwnerConnection, got {:?}", conn.token_type())
            ));
        }
        if conn.role() != Role::Owner {
            return Err(ConnectionTokenError::ValidationFailed(
                format!("OwnerConnection must have Owner role, got {:?}", conn.role())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}

/// Node connection token
#[derive(Debug, Clone)]
pub struct NodeConnectionToken(ConnectionToken);

impl NodeConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::NodeConnection {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected NodeConnection, got {:?}", conn.token_type())
            ));
        }
        if conn.role() != Role::Node {
            return Err(ConnectionTokenError::ValidationFailed(
                format!("NodeConnection must have Node role, got {:?}", conn.role())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}

/// User connection token
#[derive(Debug, Clone)]
pub struct UserConnectionToken(ConnectionToken);

impl UserConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::UserConnection {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected UserConnection, got {:?}", conn.token_type())
            ));
        }
        if conn.role() != Role::User {
            return Err(ConnectionTokenError::ValidationFailed(
                format!("UserConnection must have User role, got {:?}", conn.role())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}

/// Viewer authentication token (from shareable link)
#[derive(Debug, Clone)]
pub struct ViewerAuthToken(ConnectionToken);

impl ViewerAuthToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::ViewerAuth {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected ViewerAuth, got {:?}", conn.token_type())
            ));
        }
        if conn.role() != Role::Viewer {
            return Err(ConnectionTokenError::ValidationFailed(
                format!("ViewerAuth must have Viewer role, got {:?}", conn.role())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}

/// Viewer connection token (persistent after first connection)
#[derive(Debug, Clone)]
pub struct ViewerConnectionToken(ConnectionToken);

impl ViewerConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::ViewerConnection {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected ViewerConnection, got {:?}", conn.token_type())
            ));
        }
        if conn.role() != Role::Viewer {
            return Err(ConnectionTokenError::ValidationFailed(
                format!("ViewerConnection must have Viewer role, got {:?}", conn.role())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}

// ============================================================================
// Resource Token Types (Resource access and delegation)
// ============================================================================

/// Resource owner token (full control)
#[derive(Debug, Clone)]
pub struct ResourceOwnerToken {
    ucan: ResourceUcan,
}

impl ResourceOwnerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::ResourceOwner {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected ResourceOwner, got {:?}", ucan.token_type())
            ));
        }

        if ucan.role() != Role::Owner {
            return Err(ResourceUcanError::ValidationFailed(
                format!("ResourceOwner must have Owner role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn resource_id(&self) -> String {
        self.ucan.resource_id().expect("ResourceOwner must have resource_id")
    }

    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate> {
        self.ucan.get_delegation_template(role).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role))
        })
    }

    pub fn domain(&self) -> Option<String> {
        self.ucan.domain()
    }
}

// Trait implementations for ResourceOwnerToken
impl UcanToken for ResourceOwnerToken {
    fn raw_token(&self) -> &str {
        self.ucan.raw_token()
    }

    fn role(&self) -> Role {
        self.ucan.role()
    }

    fn parsed(&self) -> &Ucan {
        self.ucan.parsed()
    }
}

impl HasId for ResourceOwnerToken {
    fn id(&self) -> Result<String, ResourceUcanError> {
        self.ucan.resource_id().ok_or_else(|| {
            ResourceUcanError::MissingField("resource_id not found in token facts".to_string())
        })
    }
}

impl CanDelegate for ResourceOwnerToken {
    fn delegation_template(&self, role: Role) -> Result<&DelegationTemplate, ResourceUcanError> {
        self.ucan.get_delegation_template(role.as_str()).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role.as_str()))
        })
    }
}

impl ResourceOps for ResourceOwnerToken {
    fn doc_capabilities(&self) -> &HashMap<String, Capability> {
        self.ucan.capabilities()
    }
}

/// Resource share token (for Node or User role)
#[derive(Debug, Clone)]
pub struct ResourceShareToken {
    ucan: ResourceUcan,
}

impl ResourceShareToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::ResourceShare {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected ResourceShare, got {:?}", ucan.token_type())
            ));
        }

        if !matches!(ucan.role(), Role::Node | Role::User) {
            return Err(ResourceUcanError::ValidationFailed(
                format!("ResourceShare must have Node or User role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn role(&self) -> Role {
        self.ucan.role()
    }

    pub fn resource_id(&self) -> String {
        self.ucan.resource_id().expect("ResourceShare must have resource_id")
    }

    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate> {
        self.ucan.get_delegation_template(role).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role))
        })
    }

    pub fn domain(&self) -> Option<String> {
        self.ucan.domain()
    }
}

// Trait implementations for ResourceShareToken
impl UcanToken for ResourceShareToken {
    fn raw_token(&self) -> &str {
        self.ucan.raw_token()
    }

    fn role(&self) -> Role {
        self.ucan.role()
    }

    fn parsed(&self) -> &Ucan {
        self.ucan.parsed()
    }
}

impl HasId for ResourceShareToken {
    fn id(&self) -> Result<String, ResourceUcanError> {
        self.ucan.resource_id().ok_or_else(|| {
            ResourceUcanError::MissingField("resource_id not found in token facts".to_string())
        })
    }
}

impl CanDelegate for ResourceShareToken {
    fn delegation_template(&self, role: Role) -> Result<&DelegationTemplate, ResourceUcanError> {
        self.ucan.get_delegation_template(role.as_str()).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role.as_str()))
        })
    }
}

impl ResourceOps for ResourceShareToken {
    fn doc_capabilities(&self) -> &HashMap<String, Capability> {
        self.ucan.capabilities()
    }
}

/// Resource viewer token (for Viewer role)
#[derive(Debug, Clone)]
pub struct ResourceViewerToken {
    ucan: ResourceUcan,
}

impl ResourceViewerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::ResourceViewer {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected ResourceViewer, got {:?}", ucan.token_type())
            ));
        }

        if ucan.role() != Role::Viewer {
            return Err(ResourceUcanError::ValidationFailed(
                format!("ResourceViewer must have Viewer role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn resource_id(&self) -> String {
        self.ucan.resource_id().expect("ResourceViewer must have resource_id")
    }

    pub fn domain(&self) -> Option<String> {
        self.ucan.domain()
    }
}

// Trait implementations for ResourceViewerToken (NOTE: No CanDelegate - viewers can't delegate)
impl UcanToken for ResourceViewerToken {
    fn raw_token(&self) -> &str {
        self.ucan.raw_token()
    }

    fn role(&self) -> Role {
        self.ucan.role()
    }

    fn parsed(&self) -> &Ucan {
        self.ucan.parsed()
    }
}

impl HasId for ResourceViewerToken {
    fn id(&self) -> Result<String, ResourceUcanError> {
        self.ucan.resource_id().ok_or_else(|| {
            ResourceUcanError::MissingField("resource_id not found in token facts".to_string())
        })
    }
}

impl ResourceOps for ResourceViewerToken {
    fn doc_capabilities(&self) -> &HashMap<String, Capability> {
        self.ucan.capabilities()
    }
}

/// Folder owner token (full control)
#[derive(Debug, Clone)]
pub struct FolderOwnerToken {
    ucan: ResourceUcan,
}

impl FolderOwnerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::FolderOwner {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected FolderOwner, got {:?}", ucan.token_type())
            ));
        }

        if ucan.role() != Role::Owner {
            return Err(ResourceUcanError::ValidationFailed(
                format!("FolderOwner must have Owner role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn folder_id(&self) -> String {
        self.ucan.folder_id().expect("FolderOwner must have folder_id")
    }

    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate> {
        self.ucan.get_delegation_template(role).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role))
        })
    }

    pub fn domain(&self) -> Option<String> {
        self.ucan.domain()
    }
}

// Trait implementations for FolderOwnerToken
impl UcanToken for FolderOwnerToken {
    fn raw_token(&self) -> &str {
        self.ucan.raw_token()
    }

    fn role(&self) -> Role {
        self.ucan.role()
    }

    fn parsed(&self) -> &Ucan {
        self.ucan.parsed()
    }
}

impl HasId for FolderOwnerToken {
    fn id(&self) -> Result<String, ResourceUcanError> {
        self.ucan.folder_id().ok_or_else(|| {
            ResourceUcanError::MissingField("folder_id not found in token facts".to_string())
        })
    }
}

impl CanDelegate for FolderOwnerToken {
    fn delegation_template(&self, role: Role) -> Result<&DelegationTemplate, ResourceUcanError> {
        self.ucan.get_delegation_template(role.as_str()).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role.as_str()))
        })
    }
}

impl FolderOps for FolderOwnerToken {
    fn folder_operations(&self) -> Vec<String> {
        // TODO: Extract from UCAN capabilities or facts
        // For now, return empty - will be implemented in Phase 4
        Vec::new()
    }
}

/// Folder share token (for Node or User role)
#[derive(Debug, Clone)]
pub struct FolderShareToken {
    ucan: ResourceUcan,
}

impl FolderShareToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::FolderShare {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected FolderShare, got {:?}", ucan.token_type())
            ));
        }

        if !matches!(ucan.role(), Role::Node | Role::User) {
            return Err(ResourceUcanError::ValidationFailed(
                format!("FolderShare must have Node or User role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn role(&self) -> Role {
        self.ucan.role()
    }

    pub fn folder_id(&self) -> String {
        self.ucan.folder_id().expect("FolderShare must have folder_id")
    }

    pub fn get_delegation_template(&self, role: &str) -> ResourceUcanResult<&DelegationTemplate> {
        self.ucan.get_delegation_template(role).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role))
        })
    }

    pub fn domain(&self) -> Option<String> {
        self.ucan.domain()
    }
}

// Trait implementations for FolderShareToken
impl UcanToken for FolderShareToken {
    fn raw_token(&self) -> &str {
        self.ucan.raw_token()
    }

    fn role(&self) -> Role {
        self.ucan.role()
    }

    fn parsed(&self) -> &Ucan {
        self.ucan.parsed()
    }
}

impl HasId for FolderShareToken {
    fn id(&self) -> Result<String, ResourceUcanError> {
        self.ucan.folder_id().ok_or_else(|| {
            ResourceUcanError::MissingField("folder_id not found in token facts".to_string())
        })
    }
}

impl CanDelegate for FolderShareToken {
    fn delegation_template(&self, role: Role) -> Result<&DelegationTemplate, ResourceUcanError> {
        self.ucan.get_delegation_template(role.as_str()).ok_or_else(|| {
            ResourceUcanError::MissingField(format!("No delegation template for role: {}", role.as_str()))
        })
    }
}

impl FolderOps for FolderShareToken {
    fn folder_operations(&self) -> Vec<String> {
        // TODO: Extract from UCAN capabilities or facts
        // For now, return empty - will be implemented in Phase 4
        Vec::new()
    }
}

/// Folder viewer token (for Viewer role)
#[derive(Debug, Clone)]
pub struct FolderViewerToken {
    ucan: ResourceUcan,
}

impl FolderViewerToken {
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        let ucan = ResourceUcan::from_token(token)?;

        if ucan.token_type() != ResourceTokenType::FolderViewer {
            return Err(ResourceUcanError::InvalidTokenType(
                format!("Expected FolderViewer, got {:?}", ucan.token_type())
            ));
        }

        if ucan.role() != Role::Viewer {
            return Err(ResourceUcanError::ValidationFailed(
                format!("FolderViewer must have Viewer role, got {:?}", ucan.role())
            ));
        }

        Ok(Self { ucan })
    }

    pub fn ucan(&self) -> &ResourceUcan {
        &self.ucan
    }

    pub fn folder_id(&self) -> String {
        self.ucan.folder_id().expect("FolderViewer must have folder_id")
    }

    pub fn domain(&self) -> Option<String> {
        self.ucan.domain()
    }
}

// Trait implementations for FolderViewerToken (NOTE: No CanDelegate - viewers can't delegate)
impl UcanToken for FolderViewerToken {
    fn raw_token(&self) -> &str {
        self.ucan.raw_token()
    }

    fn role(&self) -> Role {
        self.ucan.role()
    }

    fn parsed(&self) -> &Ucan {
        self.ucan.parsed()
    }
}

impl HasId for FolderViewerToken {
    fn id(&self) -> Result<String, ResourceUcanError> {
        self.ucan.folder_id().ok_or_else(|| {
            ResourceUcanError::MissingField("folder_id not found in token facts".to_string())
        })
    }
}

impl FolderOps for FolderViewerToken {
    fn folder_operations(&self) -> Vec<String> {
        // TODO: Extract from UCAN capabilities or facts
        // For now, return empty - will be implemented in Phase 4
        Vec::new()
    }
}
