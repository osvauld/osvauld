use super::types::{Capability, DocMetadata, DocType, ResourceAction, ResourceTokenType, Role, SyncFacts};
use super::uri::{self, ParsedCapabilityUri};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::result::Result as StdResult;
use ucan::Ucan;

/// Error type for UCAN token operations (generic for any capability token)
#[derive(Debug)]
pub enum UcanTokenError {
    InvalidTokenType(String),
    ParsingFailed(String),
    MissingField(String),
    ValidationFailed(String),
}

impl std::fmt::Display for UcanTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UcanTokenError::InvalidTokenType(msg) => write!(f, "Invalid token type: {}", msg),
            UcanTokenError::ParsingFailed(msg) => write!(f, "Parsing failed: {}", msg),
            UcanTokenError::MissingField(msg) => write!(f, "Missing field: {}", msg),
            UcanTokenError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
        }
    }
}

impl std::error::Error for UcanTokenError {}

pub type UcanTokenResult<T> = StdResult<T, UcanTokenError>;

// ============================================================================
// UcanCore<T> - Generic core for all UCAN tokens with parsed capabilities
// ============================================================================

/// Generic core structure containing all common UCAN token fields
///
/// This struct is composed into all specific token types via the UcanToken trait
/// to eliminate duplication. It handles:
/// - Raw token and parsed UCAN from the ucan library
/// - Role and token type (generic parameter T)
/// - Parsed capabilities with type-safe methods
///
/// # Generic Parameter
/// * `T` - The token type enum (ConnectionTokenType, ResourceTokenType, etc.)
#[derive(Debug, Clone)]
pub struct UcanCore<T> {
    /// Raw UCAN token string
    raw_token: String,
    /// Parsed UCAN from ucan library
    parsed: Ucan,
    /// Role extracted from token facts
    role: Role,
    /// Token type (ConnectionTokenType, ResourceTokenType, etc.)
    token_type: T,
    /// Parsed capability URIs - cached for efficient access
    parsed_capabilities: Vec<ParsedCapabilityUri>,
}

impl<T: Clone> UcanCore<T> {
    /// Create UcanCore by parsing a token string
    ///
    /// # Arguments
    /// * `token` - Raw UCAN token string
    /// * `role` - Role extracted from token facts
    /// * `token_type` - Token type value
    ///
    /// # Returns
    /// * `Ok(UcanCore)` - Successfully parsed and initialized
    /// * `Err(UcanTokenError)` - If parsing fails
    pub fn new(token: String, parsed: Ucan, role: Role, token_type: T) -> Self {
        // Parse all capabilities once and cache them
        let parsed_capabilities: Vec<ParsedCapabilityUri> = parsed
            .capabilities()
            .iter()
            .map(|cap| ParsedCapabilityUri::parse(&cap.resource))
            .collect();

        Self {
            raw_token: token,
            parsed,
            role,
            token_type,
            parsed_capabilities,
        }
    }

    /// Get the raw token string
    pub fn raw_token(&self) -> &str {
        &self.raw_token
    }

    /// Get the parsed UCAN from ucan library
    pub fn parsed(&self) -> &Ucan {
        &self.parsed
    }

    /// Get the role
    pub fn role(&self) -> Role {
        self.role.clone()
    }

    /// Get the token type
    pub fn token_type(&self) -> &T {
        &self.token_type
    }

    /// Get parsed capabilities
    ///
    /// Returns cached, type-safe parsed capability URIs for explicit capability checking
    pub fn parsed_capabilities(&self) -> &[ParsedCapabilityUri] {
        &self.parsed_capabilities
    }

    /// Find a folder capability matching the given folder ID and operation
    ///
    /// Utility method for common folder capability checks
    pub fn find_folder_capability(
        &self,
        folder_id: &str,
        operation: &str,
    ) -> Option<&uri::FolderCapabilityUri> {
        for cap in &self.parsed_capabilities {
            if let Some(folder_cap) = cap.as_folder() {
                if folder_cap.folder_id() == folder_id && folder_cap.has_operation(operation) {
                    return Some(folder_cap);
                }
            }
        }
        None
    }

    /// Find a resource capability matching the given resource ID and document
    ///
    /// Utility method for common resource capability checks
    pub fn find_resource_capability(
        &self,
        resource_id: &str,
        doc_name: &str,
    ) -> Option<&uri::ResourceCapabilityUri> {
        for cap in &self.parsed_capabilities {
            if let Some(resource_cap) = cap.as_resource() {
                if resource_cap.resource_id() == resource_id && resource_cap.doc_name() == doc_name {
                    return Some(resource_cap);
                }
            }
        }
        None
    }
}

/// Delegation template for a specific role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationTemplate {
    pub token_type: String,                      // Token type for delegated token (e.g., "resource_share")
    pub capabilities: HashMap<String, String>,  // doc_name -> capability
    pub sync: Option<SyncFacts>,
}

impl DelegationTemplate {
    /// Build UCAN capabilities list from this template using standard URI format
    ///
    /// Uses the standard URI format from the uri module to construct capability URIs.
    /// Supports wildcard patterns for folder-scoped resources.
    ///
    /// # Arguments
    /// * `domain` - The domain (e.g., "sthalam")
    /// * `id` - The resource or folder ID (can include `/*` for folder-scoped wildcards)
    /// * `resource_type` - Either "resource" or "folder"
    ///
    /// # Returns
    /// Vector of (capability_uri:level, capability_level) tuples for UCAN generation
    ///
    /// # Examples
    /// Resource: `("sthalam:resource:abc123:document:collaborator", "collaborator")`
    /// Folder: `("sthalam:folder:xyz789:add_resources:allow", "allow")`
    /// Wildcard: `("sthalam:resource:folder123/*:document:collaborator", "collaborator")`
    pub fn build_capabilities(
        &self,
        domain: &str,
        id: &str,
        resource_type: &str,
    ) -> Vec<(String, String)> {
        self.capabilities
            .iter()
            .map(|(doc_name, cap_str)| {
                // Check if ID contains wildcard pattern for folder-scoped resources
                let capability_uri = if id.ends_with("/*") && resource_type == "resource" {
                    // Folder-scoped resource wildcard: domain:resource:folder_id/*:doc_name
                    let folder_id = &id[..id.len() - 2]; // Remove the /*
                    uri::resource_wildcard(domain, folder_id, doc_name)
                } else if resource_type == "resource" {
                    uri::resource_capability(domain, id, doc_name)
                } else if resource_type == "folder" {
                    uri::folder_operation(domain, id, doc_name)
                } else {
                    // Fallback for unknown types
                    format!("{}:{}:{}:{}", domain, resource_type, id, doc_name)
                };
                (
                    format!("{}:{}", capability_uri, cap_str), // URI + capability level
                    cap_str.clone(),
                )
            })
            .collect()
    }

    /// Convert template to UCAN facts JSON
    ///
    /// This creates the facts structure that will be embedded in the delegated token.
    /// Includes token_type, capabilities, and sync facts if present.
    pub fn to_facts(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut facts = serde_json::Map::new();

        // Add token_type (data-driven from template)
        facts.insert("token_type".to_string(), serde_json::Value::String(self.token_type.clone()));

        // Add capabilities as a map
        let caps_json: serde_json::Map<String, serde_json::Value> = self.capabilities
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        facts.insert("capabilities".to_string(), serde_json::Value::Object(caps_json));

        // Add sync facts if present
        if let Some(sync) = &self.sync {
            let mut sync_json = serde_json::Map::new();

            if !sync.local_only.is_empty() {
                sync_json.insert(
                    "local_only".to_string(),
                    serde_json::Value::Array(
                        sync.local_only.iter().map(|s| serde_json::Value::String(s.clone())).collect()
                    )
                );
            }

            if !sync.no_incoming_updates.is_empty() {
                sync_json.insert(
                    "no_incoming_updates".to_string(),
                    serde_json::Value::Array(
                        sync.no_incoming_updates.iter().map(|s| serde_json::Value::String(s.clone())).collect()
                    )
                );
            }

            if !sync.send_full_snapshot.is_empty() {
                sync_json.insert(
                    "send_full_snapshot".to_string(),
                    serde_json::Value::Array(
                        sync.send_full_snapshot.iter().map(|s| serde_json::Value::String(s.clone())).collect()
                    )
                );
            }

            if !sync_json.is_empty() {
                facts.insert("sync".to_string(), serde_json::Value::Object(sync_json));
            }
        }

        facts
    }
}

/// Parsed UCAN token with domain logic (generic for resources and folders)
///
/// Composes UcanCore<ResourceTokenType> with resource-specific fields
/// to eliminate duplication while providing type-safe access.
#[derive(Debug, Clone)]
pub struct GenericUcan {
    /// Core UCAN data (raw token, parsed UCAN, role, token_type, parsed_capabilities)
    core: UcanCore<ResourceTokenType>,
    /// Domain-specific fields below
    capabilities: HashMap<String, Capability>,  // doc_name -> capability
    doc_metadata: HashMap<String, DocMetadata>,  // doc_name -> metadata
    sync_facts: SyncFacts,
    resource_actions: Option<Vec<ResourceAction>>,
    delegation_templates: HashMap<String, DelegationTemplate>,  // role -> template
    proof_chain: Vec<String>,  // Parent UCAN CIDs
}

impl GenericUcan {
    /// Parse UCAN token and extract all domain information
    pub fn from_token(token: &str) -> UcanTokenResult<Self> {
        // Parse UCAN token
        let parsed = Ucan::try_from(token)
            .map_err(|e| UcanTokenError::ParsingFailed(format!("UCAN parsing error: {}", e)))?;

        // Extract facts
        let facts = parsed.facts().as_ref().ok_or_else(|| {
            UcanTokenError::MissingField("UCAN facts not found".to_string())
        })?;

        // Extract token_type from facts
        let token_type_str = facts
            .get("token_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| UcanTokenError::MissingField("token_type not found in facts".to_string()))?;

        let token_type = ResourceTokenType::from_str(token_type_str)
            .map_err(|e| UcanTokenError::InvalidTokenType(e))?;

        // Extract role from facts
        let role_str = facts
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| UcanTokenError::MissingField("role not found in facts".to_string()))?;

        let role = Role::from_str(role_str)
            .map_err(|e| UcanTokenError::ParsingFailed(format!("Invalid role: {}", e)))?;

        // Extract capabilities from UCAN cap field
        let mut capabilities = HashMap::new();
        for cap in parsed.capabilities().iter() {
            // Cap format: "sthalam:resource:{id}:{doc_name}:{capability}"
            let cap_resource = &cap.resource;
            let parts: Vec<&str> = cap_resource.split(':').collect();
            if parts.len() >= 4 {
                let doc_name = parts[2];
                let cap_str = parts[3];
                if let Ok(capability) = Capability::from_str(cap_str) {
                    capabilities.insert(doc_name.to_string(), capability);
                }
            }
        }

        // Extract sync facts
        let sync_facts = if let Some(sync_obj) = facts.get("sync").and_then(|v| v.as_object()) {
            let local_only = sync_obj
                .get("local_only")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let no_incoming_updates = sync_obj
                .get("no_incoming_updates")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let send_full_snapshot = sync_obj
                .get("send_full_snapshot")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            SyncFacts {
                local_only,
                no_incoming_updates,
                send_full_snapshot,
            }
        } else {
            SyncFacts::default()
        };

        // Extract doc_metadata
        let mut doc_metadata = HashMap::new();
        if let Some(metadata_obj) = facts.get("doc_metadata").and_then(|v| v.as_object()) {
            for (doc_name, meta_val) in metadata_obj {
                if let Some(meta_obj) = meta_val.as_object() {
                    if let Some(doc_type_str) = meta_obj.get("type").and_then(|v| v.as_str()) {
                        if let Ok(doc_type) = DocType::from_str(doc_type_str) {
                            let allowed_mimes = meta_obj
                                .get("allowed_mimes")
                                .and_then(|v| v.as_array())
                                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

                            let max_size_mb = meta_obj
                                .get("max_size_mb")
                                .and_then(|v| v.as_u64());

                            doc_metadata.insert(
                                doc_name.clone(),
                                DocMetadata {
                                    name: doc_name.clone(),
                                    doc_type,
                                    allowed_mimes,
                                    max_size_mb,
                                },
                            );
                        }
                    }
                }
            }
        }

        // Extract resource actions
        let resource_actions = facts
            .get("actions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| ResourceAction::from_str(s).ok())
                    .collect()
            });

        // Extract delegation templates
        let mut delegation_templates = HashMap::new();
        if let Some(delegation_obj) = facts.get("delegation").and_then(|v| v.as_object()) {
            for (role_key, template_val) in delegation_obj {
                if let Some(template_obj) = template_val.as_object() {
                    // Extract token_type (data-driven from template)
                    let token_type = template_obj
                        .get("token_type")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .unwrap_or_else(|| format!("resource_{}", role_key)); // Fallback for backward compatibility

                    // Extract capabilities map
                    let mut capabilities_map = HashMap::new();
                    if let Some(caps_obj) = template_obj.get("capabilities").and_then(|v| v.as_object()) {
                        for (doc, cap_val) in caps_obj {
                            if let Some(cap_str) = cap_val.as_str() {
                                capabilities_map.insert(doc.clone(), cap_str.to_string());
                            }
                        }
                    }

                    // Extract sync facts if present
                    let sync = if let Some(sync_obj) = template_obj.get("sync").and_then(|v| v.as_object()) {
                        let local_only = sync_obj
                            .get("local_only")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                            .unwrap_or_default();

                        let no_incoming_updates = sync_obj
                            .get("no_incoming_updates")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                            .unwrap_or_default();

                        let send_full_snapshot = sync_obj
                            .get("send_full_snapshot")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                            .unwrap_or_default();

                        Some(SyncFacts {
                            local_only,
                            no_incoming_updates,
                            send_full_snapshot,
                        })
                    } else {
                        None
                    };

                    delegation_templates.insert(
                        role_key.clone(),
                        DelegationTemplate {
                            token_type,
                            capabilities: capabilities_map,
                            sync,
                        },
                    );
                }
            }
        }

        // Extract proof chain
        let proof_chain = parsed.proofs().clone().unwrap_or_default();

        // Create UcanCore with parsed data
        let core = UcanCore::new(token.to_string(), parsed, role, token_type);

        Ok(Self {
            core,
            capabilities,
            doc_metadata,
            sync_facts,
            resource_actions,
            delegation_templates,
            proof_chain,
        })
    }

    // Accessors - delegate to core for common fields
    pub fn raw_token(&self) -> &str {
        self.core.raw_token()
    }

    pub fn role(&self) -> Role {
        self.core.role()
    }

    pub fn token_type(&self) -> ResourceTokenType {
        self.core.token_type().clone()
    }

    pub fn parsed(&self) -> &Ucan {
        self.core.parsed()
    }

    pub fn parsed_capabilities(&self) -> &[ParsedCapabilityUri] {
        self.core.parsed_capabilities()
    }

    pub fn proof_chain(&self) -> &[String] {
        &self.proof_chain
    }

    // Capability queries
    pub fn has_capability(&self, doc: &str) -> bool {
        self.capabilities.contains_key(doc)
    }

    pub fn get_capability(&self, doc: &str) -> Option<Capability> {
        self.capabilities.get(doc).copied()
    }

    pub fn capabilities(&self) -> &HashMap<String, Capability> {
        &self.capabilities
    }

    // Sync behavior queries
    pub fn is_local_only(&self, doc: &str) -> bool {
        self.sync_facts.local_only.contains(&doc.to_string())
    }

    pub fn has_no_incoming_updates(&self, doc: &str) -> bool {
        self.sync_facts.no_incoming_updates.contains(&doc.to_string())
    }

    pub fn should_send_full_snapshot(&self, doc: &str) -> bool {
        self.sync_facts.send_full_snapshot.contains(&doc.to_string())
    }

    pub fn sync_facts(&self) -> &SyncFacts {
        &self.sync_facts
    }

    // Document type queries
    pub fn get_doc_type(&self, doc: &str) -> Option<DocType> {
        self.doc_metadata.get(doc).map(|m| m.doc_type)
    }

    pub fn supports_merge(&self, doc: &str) -> bool {
        self.get_doc_type(doc)
            .map(|dt| dt.supports_merge())
            .unwrap_or(false)
    }

    pub fn doc_metadata(&self) -> &HashMap<String, DocMetadata> {
        &self.doc_metadata
    }

    // Template access (for delegation)
    pub fn get_delegation_template(&self, role: &str) -> Option<&DelegationTemplate> {
        self.delegation_templates.get(role)
    }

    pub fn delegation_templates(&self) -> &HashMap<String, DelegationTemplate> {
        &self.delegation_templates
    }

    // Resource-level actions
    pub fn can_get_share_link(&self) -> bool {
        self.resource_actions
            .as_ref()
            .map(|actions| actions.contains(&ResourceAction::GetShareLink))
            .unwrap_or(false)
    }

    pub fn can_delete(&self) -> bool {
        self.resource_actions
            .as_ref()
            .map(|actions| actions.contains(&ResourceAction::Delete))
            .unwrap_or(false)
    }

    pub fn resource_actions(&self) -> Option<&Vec<ResourceAction>> {
        self.resource_actions.as_ref()
    }

    // Folder-level actions
    /// Check if the token has get_share_link capability for a folder
    /// This checks folder-level capabilities (not resource-level)
    /// Folder capabilities have format: "domain:folder:folder_id:get_share_link"
    pub fn can_get_folder_share_link(&self) -> bool {
        self.parsed().capabilities().iter().any(|cap| {
            cap.resource.contains(":folder:") && cap.resource.contains(":get_share_link")
        })
    }

    // Resource/Folder ID extraction from capability URIs (source of truth)
    // The URI standard is the canonical format for resource/folder identification
    pub fn resource_id(&self) -> Option<String> {
        // Extract from first capability URI with resource type
        for cap in self.parsed().capabilities().iter() {
            if let Ok(id) = uri::parse_resource_id(&cap.resource) {
                return Some(id);
            }
        }
        None
    }

    pub fn folder_id(&self) -> Option<String> {
        // Try to extract from first capability URI with folder type
        for cap in self.parsed().capabilities().iter() {
            if let Ok(id) = uri::parse_folder_id(&cap.resource) {
                return Some(id);
            }
            // Also check wildcard resource URIs (folder_id/*)
            if let Ok(id) = uri::parse_folder_id_from_wildcard(&cap.resource) {
                return Some(id);
            }
        }
        None
    }

    /// Extract domain from capability URIs (source of truth)
    /// Format: domain:resource:id:doc_name or domain:folder:id:operation
    pub fn domain(&self) -> Option<String> {
        // Extract domain from first capability URI
        for cap in self.parsed().capabilities().iter() {
            let parts: Vec<&str> = cap.resource.split(':').collect();
            if !parts.is_empty() {
                return Some(parts[0].to_string());
            }
        }
        None
    }
}
