use super::types::{Capability, DocMetadata, DocType, ResourceAction, SyncFacts};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::result::Result as StdResult;
use ucan::Ucan;

// Note: Role and ResourceTokenType removed - using facts-only approach

/// Error type for Permit token operations
#[derive(Debug)]
pub enum PermitError {
    InvalidTokenType(String),
    ParsingFailed(String),
    MissingField(String),
    ValidationFailed(String),
}

impl std::fmt::Display for PermitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermitError::InvalidTokenType(msg) => write!(f, "Invalid token type: {}", msg),
            PermitError::ParsingFailed(msg) => write!(f, "Parsing failed: {}", msg),
            PermitError::MissingField(msg) => write!(f, "Missing field: {}", msg),
            PermitError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
        }
    }
}

impl std::error::Error for PermitError {}

pub type PermitResult<T> = StdResult<T, PermitError>;

// ============================================================================
// PermitCore - Core structure for all Permit tokens
// ============================================================================

/// Core structure containing common Permit token fields.
///
/// This struct is composed into all specific token types to eliminate duplication.
/// It handles:
/// - Raw token string and parsed token from the ucan library
/// - Facts stored as raw JSON for flexible access
///
/// Identity is derived from facts, not stored as enum.
#[derive(Debug, Clone)]
pub struct PermitCore {
    /// Raw permit token string
    raw_token: String,
    /// Parsed token (uses ucan library internally)
    parsed: Ucan,
    /// Raw facts as JSON (source of truth)
    facts: serde_json::Map<String, serde_json::Value>,
}

impl PermitCore {
    /// Create PermitCore from parsed token data.
    ///
    /// # Arguments
    /// * `token` - Raw permit token string
    /// * `parsed` - Parsed token from ucan library
    /// * `facts` - Raw facts as JSON map
    pub fn new(token: String, parsed: Ucan, facts: serde_json::Map<String, serde_json::Value>) -> Self {
        Self {
            raw_token: token,
            parsed,
            facts,
        }
    }

    /// Get the raw token string
    pub fn raw_token(&self) -> &str {
        &self.raw_token
    }

    /// Get the parsed token (internal ucan library type)
    pub fn parsed(&self) -> &Ucan {
        &self.parsed
    }

    /// Get the raw facts (source of truth)
    pub fn facts(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.facts
    }

    /// Get a specific fact by key
    pub fn get_fact(&self, key: &str) -> Option<&serde_json::Value> {
        self.facts.get(key)
    }

    /// Get a string fact by key
    pub fn get_fact_string(&self, key: &str) -> Option<&str> {
        self.get_fact(key).and_then(|v| v.as_str())
    }

    /// Get a bool fact by key
    pub fn get_fact_bool(&self, key: &str) -> Option<bool> {
        self.get_fact(key).and_then(|v| v.as_bool())
    }

    // ==================== Derived Identity Methods ====================
    // Identity is derived from facts, not stored as enum

    /// Check if token holder is owner (derived from operations.own == "allow")
    pub fn is_owner(&self) -> bool {
        self.get_fact("operations")
            .and_then(|ops| ops.as_object())
            .and_then(|obj| obj.get("own"))
            .and_then(|v| v.as_str())
            .map(|s| s == "allow")
            .unwrap_or(false)
    }

    /// Check if token holder is host/node (derived from operations.add_resources == "allow")
    pub fn is_host(&self) -> bool {
        self.get_fact("operations")
            .and_then(|ops| ops.as_object())
            .and_then(|obj| obj.get("add_resources"))
            .and_then(|v| v.as_str())
            .map(|s| s == "allow")
            .unwrap_or(false)
    }

    /// Check if this is a first connection token (derived from first_connection fact)
    pub fn is_first_connection(&self) -> bool {
        self.get_fact_bool("first_connection").unwrap_or(false)
    }

    /// Get relationship label (for logging/debugging, not for logic)
    pub fn relationship(&self) -> Option<&str> {
        self.get_fact_string("relationship")
    }
}

/// Delegation template for a specific role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationTemplate {
    pub token_type: String,                      // Token type for delegated token (e.g., "resource_share")
    pub capabilities: HashMap<String, String>,  // doc_name -> capability
    pub sync: Option<SyncFacts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_types: Option<HashMap<String, String>>,  // doc_name -> doc_type (crdt/asset)
    // CEL-based authorization fields (v3)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_capabilities: Option<HashMap<String, serde_json::Value>>,  // CEL capabilities (can_connect, persist_share, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operations: Option<HashMap<String, serde_json::Value>>,          // Operations (own, read, write)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship: Option<String>,                                     // Relationship label (owner, node, viewer)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cel_rules: Option<HashMap<String, String>>,                      // CEL expression rules
}

impl DelegationTemplate {
    /// Convert template to permit facts JSON
    ///
    /// This creates the facts structure that will be embedded in the delegated token.
    /// Includes token_type, sync facts, and CEL-related fields if present.
    ///
    /// Note: Document capabilities go into permit capability URIs (via build_capabilities()),
    /// not into facts. The `capabilities` field in facts is for CEL auth capabilities.
    pub fn to_facts(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut facts = serde_json::Map::new();

        // Add token_type (data-driven from template)
        facts.insert("token_type".to_string(), serde_json::Value::String(self.token_type.clone()));

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

        // Add doc_types as doc_metadata (converts to DocMetadata format expected by parser)
        if let Some(doc_types) = &self.doc_types {
            let mut doc_metadata_json = serde_json::Map::new();
            for (doc_name, doc_type_str) in doc_types {
                let mut meta = serde_json::Map::new();
                meta.insert("type".to_string(), serde_json::Value::String(doc_type_str.clone()));
                doc_metadata_json.insert(doc_name.clone(), serde_json::Value::Object(meta));
            }
            if !doc_metadata_json.is_empty() {
                facts.insert("doc_metadata".to_string(), serde_json::Value::Object(doc_metadata_json));
            }
        }

        // Add documents map (combines capabilities and doc_types)
        // Format: { doc_name: { capability: "collaborator"|"viewer", type: "crdt"|"asset" } }
        if !self.capabilities.is_empty() {
            let mut documents_json = serde_json::Map::new();
            for (doc_name, capability) in &self.capabilities {
                let mut doc_info = serde_json::Map::new();
                doc_info.insert("capability".to_string(), serde_json::Value::String(capability.clone()));

                // Add type if available from doc_types
                if let Some(doc_types) = &self.doc_types {
                    if let Some(doc_type) = doc_types.get(doc_name) {
                        doc_info.insert("type".to_string(), serde_json::Value::String(doc_type.clone()));
                    }
                }

                documents_json.insert(doc_name.clone(), serde_json::Value::Object(doc_info));
            }
            facts.insert("documents".to_string(), serde_json::Value::Object(documents_json));
        }

        // Add CEL-based authorization capabilities (v3)
        // Fixed: Use "auth_capabilities" instead of "capabilities" to match CEL rules
        if let Some(auth_caps) = &self.auth_capabilities {
            let auth_caps_json: serde_json::Map<String, serde_json::Value> = auth_caps
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            facts.insert("auth_capabilities".to_string(), serde_json::Value::Object(auth_caps_json));
        }

        // Add operations
        if let Some(ops) = &self.operations {
            let ops_json: serde_json::Map<String, serde_json::Value> = ops
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            facts.insert("operations".to_string(), serde_json::Value::Object(ops_json));
        }

        // Add relationship
        if let Some(rel) = &self.relationship {
            facts.insert("relationship".to_string(), serde_json::Value::String(rel.clone()));
        }

        // Add CEL rules
        if let Some(cel) = &self.cel_rules {
            let cel_json: serde_json::Map<String, serde_json::Value> = cel
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            facts.insert("cel_rules".to_string(), serde_json::Value::Object(cel_json));
        }

        facts
    }
}

/// Parsed Permit token with domain logic (for resources and folders).
///
/// Facts-only approach - identity is derived from facts, not stored as enum.
#[derive(Debug, Clone)]
pub struct Permit {
    /// Core permit data (raw token, parsed, facts)
    core: PermitCore,
    /// Document capabilities (doc_name -> capability)
    capabilities: HashMap<String, Capability>,
    /// Document metadata (doc_name -> metadata)
    doc_metadata: HashMap<String, DocMetadata>,
    /// Sync behavior configuration
    sync_facts: SyncFacts,
    /// Allowed resource actions
    resource_actions: Option<Vec<ResourceAction>>,
    /// Delegation templates (template_key -> template)
    delegation_templates: HashMap<String, DelegationTemplate>,
    /// Parent permit CIDs for delegation chain
    proof_chain: Vec<String>,
}

impl Permit {
    /// Parse permit token and extract all domain information.
    ///
    /// Identity is derived from facts, not stored as enum.
    pub fn from_token(token: &str) -> PermitResult<Self> {
        use tracing::debug;

        debug!("Parsing permit token (len={})", token.len());

        // Parse permit token (uses ucan library internally)
        let parsed = Ucan::try_from(token)
            .map_err(|e| PermitError::ParsingFailed(format!("Permit parsing error: {}", e)))?;

        debug!("JWT decoded - issuer: {}, audience: {}", parsed.issuer(), parsed.audience());

        // Extract facts as raw JSON (source of truth)
        let facts_ref = parsed.facts().as_ref().ok_or_else(|| {
            PermitError::MissingField("Permit facts not found".to_string())
        })?;

        // Clone facts for storage (we'll keep the raw JSON)
        // Convert BTreeMap to serde_json::Map
        let facts: serde_json::Map<String, serde_json::Value> = facts_ref
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        debug!(facts = ?facts.keys().collect::<Vec<_>>(), "Facts extracted");

        // Log key facts for debugging
        if let Some(rel) = facts.get("relationship") {
            debug!(relationship = %rel);
        }
        if let Some(token_type) = facts.get("token_type") {
            debug!(token_type = %token_type);
        }
        if let Some(first_conn) = facts.get("first_connection") {
            debug!(first_connection = %first_conn);
        }
        if let Some(user_id) = facts.get("user_id") {
            debug!(user_id = %user_id);
        }

        // Extract capabilities from facts.documents (facts-only approach)
        // Documents should be in facts, not in the cap field
        let mut capabilities = HashMap::new();
        if let Some(documents_obj) = facts.get("documents").and_then(|v| v.as_object()) {
            debug!(count = documents_obj.len(), "Documents found in facts");
            for (doc_name, doc_val) in documents_obj {
                if let Some(doc_obj) = doc_val.as_object() {
                    if let Some(cap_str) = doc_obj.get("capability").and_then(|v| v.as_str()) {
                        if let Ok(capability) = Capability::from_str(cap_str) {
                            capabilities.insert(doc_name.clone(), capability);
                            debug!(doc = %doc_name, capability = ?capability, "Parsed document capability");
                        }
                    }
                }
            }
        } else {
            debug!("No documents found in facts");
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

                    // Extract capabilities map from "documents" field
                    // Frontend generates: { documents: { doc_name: { capability: "collaborator", type: "crdt" } } }
                    let mut capabilities_map = HashMap::new();
                    if let Some(docs_obj) = template_obj.get("documents").and_then(|v| v.as_object()) {
                        for (doc_name, doc_val) in docs_obj {
                            if let Some(doc_obj) = doc_val.as_object() {
                                if let Some(cap_str) = doc_obj.get("capability").and_then(|v| v.as_str()) {
                                    capabilities_map.insert(doc_name.clone(), cap_str.to_string());
                                }
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

                    // Extract doc_types from "documents" field (same source as capabilities)
                    // Frontend generates: { documents: { doc_name: { capability: "collaborator", type: "crdt" } } }
                    let doc_types = if let Some(docs_obj) = template_obj.get("documents").and_then(|v| v.as_object()) {
                        let mut types_map = HashMap::new();
                        for (doc_name, doc_val) in docs_obj {
                            if let Some(doc_obj) = doc_val.as_object() {
                                if let Some(type_str) = doc_obj.get("type").and_then(|v| v.as_str()) {
                                    types_map.insert(doc_name.clone(), type_str.to_string());
                                }
                            }
                        }
                        if types_map.is_empty() { None } else { Some(types_map) }
                    } else {
                        None
                    };

                    // Extract CEL authorization capabilities (v3)
                    let auth_capabilities = template_obj
                        .get("auth_capabilities")
                        .and_then(|v| v.as_object())
                        .map(|obj| {
                            obj.iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect()
                        });

                    // Extract operations
                    let operations = template_obj
                        .get("operations")
                        .and_then(|v| v.as_object())
                        .map(|obj| {
                            obj.iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect()
                        });

                    // Extract relationship
                    let relationship = template_obj
                        .get("relationship")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    // Extract CEL rules
                    let cel_rules = template_obj
                        .get("cel_rules")
                        .and_then(|v| v.as_object())
                        .map(|obj| {
                            obj.iter()
                                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                                .collect()
                        });

                    delegation_templates.insert(
                        role_key.clone(),
                        DelegationTemplate {
                            token_type,
                            capabilities: capabilities_map,
                            sync,
                            doc_types,
                            auth_capabilities,
                            operations,
                            relationship,
                            cel_rules,
                        },
                    );
                }
            }
        }

        // Extract proof chain
        let proof_chain = parsed.proofs().clone().unwrap_or_default();

        // Create PermitCore with parsed data (facts-only, no role/token_type enums)
        let core = PermitCore::new(token.to_string(), parsed, facts.clone());

        // Log parsed permit facts for debugging
        use tracing::trace;
        trace!(
            "Permit parsed:\n{}",
            serde_json::to_string_pretty(&facts).unwrap_or_else(|_| format!("{:?}", facts))
        );

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

    pub fn parsed(&self) -> &Ucan {
        self.core.parsed()
    }

    pub fn proof_chain(&self) -> &[String] {
        &self.proof_chain
    }

    /// Get raw facts (source of truth)
    pub fn facts(&self) -> &serde_json::Map<String, serde_json::Value> {
        self.core.facts()
    }

    /// Get a specific fact by key
    pub fn get_fact(&self, key: &str) -> Option<&serde_json::Value> {
        self.core.get_fact(key)
    }

    /// Get a string fact by key
    pub fn get_fact_string(&self, key: &str) -> Option<&str> {
        self.core.get_fact_string(key)
    }

    // ==================== Derived Identity (V3) ====================
    // These replace role() and token_type() methods

    /// Check if token holder is owner (derived from operations.own)
    pub fn is_owner(&self) -> bool {
        self.core.is_owner()
    }

    /// Check if token holder is host/node (derived from operations.add_resources)
    pub fn is_host(&self) -> bool {
        self.core.is_host()
    }

    /// Check if this is a first connection token
    pub fn is_first_connection(&self) -> bool {
        self.core.is_first_connection()
    }

    /// Get relationship label (for logging only, not for business logic)
    pub fn relationship(&self) -> Option<&str> {
        self.core.relationship()
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
    /// Check if the token has get_share_link operation (v3 facts-based)
    /// Checks operations.get_share_link == "allow" in facts
    pub fn can_get_folder_share_link(&self) -> bool {
        self.get_fact("operations")
            .and_then(|ops| ops.as_object())
            .and_then(|obj| obj.get("get_share_link"))
            .and_then(|v| v.as_str())
            .map(|s| s == "allow")
            .unwrap_or(false)
    }

    // ==================== ID Extraction (V3: Facts-Only) ====================
    // V3 Migration: These methods now read from facts instead of parsing URIs

    /// Extract resource_id from token facts
    ///
    /// V3: Reads from facts.resource_id (facts-only architecture)
    pub fn resource_id(&self) -> Option<String> {
        self.get_fact_string("resource_id").map(String::from)
    }

    /// Extract folder_id from token facts
    ///
    /// V3: Reads from facts.folder_id (facts-only architecture)
    pub fn folder_id(&self) -> Option<String> {
        self.get_fact_string("folder_id").map(String::from)
    }


    // ==================== CEL-Based Authorization Helpers (V3) ====================
    // These methods evaluate CEL rules from token facts to determine capabilities

    /// Check if share_record should be persisted to database
    ///
    /// Evaluates the `persist_share` CEL rule from token facts.
    /// Falls back to checking the boolean capability directly if no CEL rule exists.
    ///
    /// # Example
    /// ```rust,ignore
    /// if permit.should_persist_share() {
    ///     db.save_share_record(share_record).await?;
    /// }
    /// ```
    pub fn should_persist_share(&self) -> bool {
        self.evaluate_cel_rule("persist_share")
            .or_else(|| {
                // Fallback: check boolean capability directly
                self.get_fact("capabilities")
                    .and_then(|caps| caps.as_object())
                    .and_then(|obj| obj.get("persist_share"))
                    .and_then(|v| v.as_bool())
            })
            .unwrap_or(false)
    }

    /// Check if token can establish P2P connections
    ///
    /// Evaluates the `can_connect` CEL rule from token facts.
    /// Falls back to checking the boolean capability directly if no CEL rule exists.
    ///
    /// # Example
    /// ```rust,ignore
    /// if permit.can_connect() {
    ///     establish_connection(peer_id).await?;
    /// }
    /// ```
    pub fn can_connect(&self) -> bool {
        self.evaluate_cel_rule("can_connect")
            .or_else(|| {
                // Fallback: check boolean capability directly
                self.get_fact("capabilities")
                    .and_then(|caps| caps.as_object())
                    .and_then(|obj| obj.get("can_connect"))
                    .and_then(|v| v.as_bool())
            })
            .unwrap_or(false)
    }

    /// Check if token can issue new delegated tokens
    ///
    /// Evaluates the `can_delegate` CEL rule from token facts.
    /// Falls back to checking the boolean capability directly if no CEL rule exists.
    ///
    /// # Example
    /// ```rust,ignore
    /// if permit.can_delegate() {
    ///     let delegated_token = issue_delegated_token(permit).await?;
    /// }
    /// ```
    pub fn can_delegate(&self) -> bool {
        self.evaluate_cel_rule("can_delegate")
            .or_else(|| {
                // Fallback: check boolean capability directly
                self.get_fact("capabilities")
                    .and_then(|caps| caps.as_object())
                    .and_then(|obj| obj.get("can_delegate"))
                    .and_then(|v| v.as_bool())
            })
            .unwrap_or(false)
    }

    /// Check if token can sync updates with peers
    ///
    /// Evaluates the `sync_enabled` CEL rule from token facts.
    /// Falls back to checking the boolean capability directly if no CEL rule exists.
    ///
    /// # Example
    /// ```rust,ignore
    /// if permit.sync_enabled() {
    ///     sync_updates_with_peer(updates).await?;
    /// }
    /// ```
    pub fn sync_enabled(&self) -> bool {
        self.evaluate_cel_rule("sync_enabled")
            .or_else(|| {
                // Fallback: check boolean capability directly
                self.get_fact("capabilities")
                    .and_then(|caps| caps.as_object())
                    .and_then(|obj| obj.get("sync_enabled"))
                    .and_then(|v| v.as_bool())
            })
            .unwrap_or(false)
    }

    /// Helper method to evaluate a CEL rule from token facts
    ///
    /// # Arguments
    /// * `rule_name` - Name of the rule to evaluate (e.g., "persist_share", "can_connect")
    ///
    /// # Returns
    /// * `Some(true/false)` if CEL rule exists and evaluates successfully
    /// * `None` if no CEL rule exists or evaluation fails
    fn evaluate_cel_rule(&self, rule_name: &str) -> Option<bool> {
        // Get cel_rules object from facts
        let cel_rules = self.get_fact("cel_rules")?.as_object()?;

        // Get the specific rule expression
        let expression = cel_rules.get(rule_name)?.as_str()?;

        // Create validator and evaluate
        let mut validator = crate::cel::OperationValidator::new();
        validator.validate(expression, self.facts()).ok()
    }
}
