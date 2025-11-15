/// URI standard for UCAN capabilities
///
/// URIs are the single source of truth for resource/folder identification.
/// All ID information is encoded in the URI structure.

// ============================================================================
// URI Builders - Construct capability URIs
// ============================================================================

/// Build resource capability URI
///
/// Format: `domain:resource:id:doc_name`
/// Example: `sthalam:resource:abc123:document`
pub fn resource_capability(domain: &str, resource_id: &str, doc_name: &str) -> String {
    format!("{}:resource:{}:{}", domain, resource_id, doc_name)
}

/// Build folder operation URI
///
/// Format: `domain:folder:id:operation`
/// Example: `sthalam:folder:xyz789:add_resources`
pub fn folder_operation(domain: &str, folder_id: &str, operation: &str) -> String {
    format!("{}:folder:{}:{}", domain, folder_id, operation)
}

/// Build resource wildcard URI (for folder-scoped resources)
///
/// Format: `domain:resource:folder_id/*:doc_name`
/// Example: `sthalam:resource:folder123/*:document`
pub fn resource_wildcard(domain: &str, folder_id: &str, doc_name: &str) -> String {
    format!("{}:resource:{}/*:{}", domain, folder_id, doc_name)
}

/// Build folder wildcard URI (for domain-scoped operations)
///
/// Format: `domain:folder:*:operation`
/// Example: `sthalam:folder:*:add_folder`
pub fn folder_wildcard(domain: &str, operation: &str) -> String {
    format!("{}:folder:*:{}", domain, operation)
}

// ============================================================================
// URI Parsers - Extract IDs from capability URIs (source of truth)
// ============================================================================

/// Error type for URI parsing
#[derive(Debug, Clone)]
pub enum UriParseError {
    InvalidFormat(String),
    MissingField(String),
    InvalidResourceType(String),
}

impl std::fmt::Display for UriParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UriParseError::InvalidFormat(msg) => write!(f, "Invalid URI format: {}", msg),
            UriParseError::MissingField(msg) => write!(f, "Missing URI field: {}", msg),
            UriParseError::InvalidResourceType(msg) => write!(f, "Invalid resource type: {}", msg),
        }
    }
}

impl std::error::Error for UriParseError {}

pub type UriParseResult<T> = std::result::Result<T, UriParseError>;

/// Parse resource ID from a resource capability URI
///
/// Format: `domain:resource:id:doc_name`
/// Returns: `id`
///
/// # Example
/// ```ignore
/// let uri = "sthalam:resource:abc123:document";
/// let id = parse_resource_id(uri)?;
/// assert_eq!(id, "abc123");
/// ```
pub fn parse_resource_id(uri: &str) -> UriParseResult<String> {
    let parts: Vec<&str> = uri.split(':').collect();

    if parts.len() < 3 {
        return Err(UriParseError::InvalidFormat(
            format!("Resource URI must have at least 3 parts, got {}", parts.len())
        ));
    }

    let resource_type = parts.get(1).ok_or_else(|| {
        UriParseError::MissingField("resource type (part 1)".to_string())
    })?;

    if *resource_type != "resource" {
        return Err(UriParseError::InvalidResourceType(
            format!("Expected 'resource', got '{}'", resource_type)
        ));
    }

    let id = parts.get(2).ok_or_else(|| {
        UriParseError::MissingField("resource id (part 2)".to_string())
    })?;

    Ok(id.to_string())
}

/// Parse folder ID from a folder operation URI
///
/// Format: `domain:folder:id:operation` or `domain:folder:*:operation` (wildcard)
/// Returns: `id` or `"*"` for wildcard
///
/// # Example
/// ```ignore
/// let uri = "sthalam:folder:xyz789:add_resources";
/// let id = parse_folder_id(uri)?;
/// assert_eq!(id, "xyz789");
/// ```
pub fn parse_folder_id(uri: &str) -> UriParseResult<String> {
    let parts: Vec<&str> = uri.split(':').collect();

    if parts.len() < 3 {
        return Err(UriParseError::InvalidFormat(
            format!("Folder URI must have at least 3 parts, got {}", parts.len())
        ));
    }

    let resource_type = parts.get(1).ok_or_else(|| {
        UriParseError::MissingField("resource type (part 1)".to_string())
    })?;

    if *resource_type != "folder" {
        return Err(UriParseError::InvalidResourceType(
            format!("Expected 'folder', got '{}'", resource_type)
        ));
    }

    let id = parts.get(2).ok_or_else(|| {
        UriParseError::MissingField("folder id (part 2)".to_string())
    })?;

    Ok(id.to_string())
}

/// Extract resource ID from folder-scoped resource URI
///
/// Format: `domain:resource:folder_id/*:doc_name`
/// Returns: `folder_id`
///
/// # Example
/// ```ignore
/// let uri = "sthalam:resource:folder123/*:document";
/// let folder_id = parse_folder_id_from_wildcard(uri)?;
/// assert_eq!(folder_id, "folder123");
/// ```
pub fn parse_folder_id_from_wildcard(uri: &str) -> UriParseResult<String> {
    let parts: Vec<&str> = uri.split(':').collect();

    if parts.len() < 3 {
        return Err(UriParseError::InvalidFormat(
            format!("Wildcard URI must have at least 3 parts, got {}", parts.len())
        ));
    }

    let resource_type = parts.get(1).ok_or_else(|| {
        UriParseError::MissingField("resource type (part 1)".to_string())
    })?;

    if *resource_type != "resource" {
        return Err(UriParseError::InvalidResourceType(
            format!("Expected 'resource', got '{}'", resource_type)
        ));
    }

    let id = parts.get(2).ok_or_else(|| {
        UriParseError::MissingField("folder id (part 2)".to_string())
    })?;

    // Verify this is a wildcard URI (next part should be "*")
    let wildcard = parts.get(3).ok_or_else(|| {
        UriParseError::MissingField("wildcard marker (part 3)".to_string())
    })?;

    if *wildcard != "*" {
        return Err(UriParseError::InvalidFormat(
            format!("Expected wildcard '*', got '{}'", wildcard)
        ));
    }

    Ok(id.to_string())
}

// ============================================================================
// Parsed Capability URI Types - Explicit, type-safe capability access
// ============================================================================

/// Parsed capability URI - can be one of several types
#[derive(Debug, Clone)]
pub enum ParsedCapabilityUri {
    /// Folder operation capability: domain:folder:id:operation:permission
    Folder(FolderCapabilityUri),
    /// Resource document capability: domain:resource:id:doc_name
    Resource(ResourceCapabilityUri),
    /// User capability: domain:user-type:user_id (connect, share, etc.)
    User(UserCapabilityUri),
    /// Unknown or unsupported format
    Other(String),
}

impl ParsedCapabilityUri {
    /// Parse a capability URI string into a typed structure
    pub fn parse(uri: &str) -> Self {
        let parts: Vec<&str> = uri.split(':').collect();

        if parts.len() < 3 {
            return ParsedCapabilityUri::Other(uri.to_string());
        }

        let domain = parts[0];
        let resource_type = parts[1];

        match resource_type {
            "folder" if parts.len() >= 5 => {
                // Format: domain:folder:id:operation:permission
                if let (Some(folder_id), Some(operation), Some(permission)) =
                    (parts.get(2), parts.get(3), parts.get(4))
                {
                    return ParsedCapabilityUri::Folder(FolderCapabilityUri {
                        domain: domain.to_string(),
                        folder_id: folder_id.to_string(),
                        operation: operation.to_string(),
                        permission: permission.to_string(),
                    });
                }
            }
            "resource" if parts.len() >= 4 => {
                // Format: domain:resource:id:doc_name
                if let (Some(resource_id), Some(doc_name)) = (parts.get(2), parts.get(3)) {
                    return ParsedCapabilityUri::Resource(ResourceCapabilityUri {
                        domain: domain.to_string(),
                        resource_id: resource_id.to_string(),
                        doc_name: doc_name.to_string(),
                    });
                }
            }
            "user" if parts.len() >= 3 => {
                // Format: domain:user-type:user_id or domain:user-type:id:...
                if let (Some(capability_type), Some(user_id)) = (parts.get(2), parts.get(3)) {
                    return ParsedCapabilityUri::User(UserCapabilityUri {
                        domain: domain.to_string(),
                        capability_type: capability_type.to_string(),
                        user_id: user_id.to_string(),
                    });
                }
            }
            _ => {}
        }

        ParsedCapabilityUri::Other(uri.to_string())
    }

    /// Check if this is a folder capability
    pub fn is_folder(&self) -> bool {
        matches!(self, ParsedCapabilityUri::Folder(_))
    }

    /// Get as folder capability if this is one
    pub fn as_folder(&self) -> Option<&FolderCapabilityUri> {
        match self {
            ParsedCapabilityUri::Folder(f) => Some(f),
            _ => None,
        }
    }

    /// Check if this is a resource capability
    pub fn is_resource(&self) -> bool {
        matches!(self, ParsedCapabilityUri::Resource(_))
    }

    /// Get as resource capability if this is one
    pub fn as_resource(&self) -> Option<&ResourceCapabilityUri> {
        match self {
            ParsedCapabilityUri::Resource(r) => Some(r),
            _ => None,
        }
    }

    /// Check if this is a user capability
    pub fn is_user(&self) -> bool {
        matches!(self, ParsedCapabilityUri::User(_))
    }

    /// Get as user capability if this is one
    pub fn as_user(&self) -> Option<&UserCapabilityUri> {
        match self {
            ParsedCapabilityUri::User(u) => Some(u),
            _ => None,
        }
    }
}

/// Parsed folder operation capability URI
/// Format: `domain:folder:id:operation:permission`
/// Example: `sthalam:folder:xyz789:add_resources:allow`
#[derive(Debug, Clone)]
pub struct FolderCapabilityUri {
    domain: String,
    folder_id: String,
    operation: String,
    permission: String,
}

impl FolderCapabilityUri {
    /// Get the domain (e.g., "sthalam")
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get the folder ID
    pub fn folder_id(&self) -> &str {
        &self.folder_id
    }

    /// Get the operation name (e.g., "add_resources", "get_share_link")
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// Get the permission level (e.g., "allow", "deny")
    pub fn permission(&self) -> &str {
        &self.permission
    }

    /// Check if operation matches the given operation name
    pub fn has_operation(&self, op: &str) -> bool {
        self.operation == op
    }

    /// Check if permission matches the given permission
    pub fn has_permission(&self, perm: &str) -> bool {
        self.permission == perm
    }

    /// Convenience method: check for specific operation and permission
    pub fn matches(&self, operation: &str, permission: &str) -> bool {
        self.has_operation(operation) && self.has_permission(permission)
    }
}

/// Parsed resource document capability URI
/// Format: `domain:resource:id:doc_name`
/// Example: `sthalam:resource:abc123:document`
#[derive(Debug, Clone)]
pub struct ResourceCapabilityUri {
    domain: String,
    resource_id: String,
    doc_name: String,
}

impl ResourceCapabilityUri {
    /// Get the domain (e.g., "sthalam")
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get the resource ID
    pub fn resource_id(&self) -> &str {
        &self.resource_id
    }

    /// Get the document name (e.g., "template_doc", "content_doc")
    pub fn doc_name(&self) -> &str {
        &self.doc_name
    }
}

/// Parsed user capability URI
/// Format: `domain:user-type:user_id`
/// Example: `sthalam:user-connect:user123`
#[derive(Debug, Clone)]
pub struct UserCapabilityUri {
    domain: String,
    capability_type: String,
    user_id: String,
}

impl UserCapabilityUri {
    /// Get the domain (e.g., "sthalam")
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get the capability type (e.g., "connect", "share")
    pub fn capability_type(&self) -> &str {
        &self.capability_type
    }

    /// Get the user ID
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Check if capability type matches
    pub fn has_type(&self, cap_type: &str) -> bool {
        self.capability_type == cap_type
    }
}
