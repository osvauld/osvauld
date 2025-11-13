use super::capability::{Capability, DocMetadata, DocType, ResourceAction, ResourceTokenType, Role, SyncFacts};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::result::Result as StdResult;
use ucan::Ucan;

/// Error type for resource UCAN operations
#[derive(Debug)]
pub enum ResourceUcanError {
    InvalidTokenType(String),
    ParsingFailed(String),
    MissingField(String),
    ValidationFailed(String),
}

impl std::fmt::Display for ResourceUcanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceUcanError::InvalidTokenType(msg) => write!(f, "Invalid token type: {}", msg),
            ResourceUcanError::ParsingFailed(msg) => write!(f, "Parsing failed: {}", msg),
            ResourceUcanError::MissingField(msg) => write!(f, "Missing field: {}", msg),
            ResourceUcanError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
        }
    }
}

impl std::error::Error for ResourceUcanError {}

pub type ResourceUcanResult<T> = StdResult<T, ResourceUcanError>;

/// Delegation template for a specific role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationTemplate {
    pub capabilities: HashMap<String, String>,  // doc_name -> capability
    pub sync: Option<SyncFacts>,
}

impl DelegationTemplate {
    /// Build UCAN capabilities list from this template
    ///
    /// # Arguments
    /// * `id` - The resource or folder ID
    /// * `resource_type` - Either "resource" or "folder"
    ///
    /// # Returns
    /// Vector of (URI, capability) tuples for UCAN generation
    pub fn build_capabilities(&self, id: &str, resource_type: &str) -> Vec<(String, String)> {
        self.capabilities
            .iter()
            .map(|(doc_name, cap_str)| {
                let uri = format!("sthalam:{}:{}:{}", resource_type, id, doc_name);
                (uri, cap_str.clone())
            })
            .collect()
    }

    /// Convert template to UCAN facts JSON
    ///
    /// This creates the facts structure that will be embedded in the delegated token.
    /// Includes both capabilities and sync facts if present.
    pub fn to_facts(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut facts = serde_json::Map::new();

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

/// Parsed resource UCAN with domain logic
#[derive(Debug, Clone)]
pub struct ResourceUcan {
    raw_token: String,
    parsed: Ucan,
    role: Role,
    token_type: ResourceTokenType,
    capabilities: HashMap<String, Capability>,  // doc_name -> capability
    doc_metadata: HashMap<String, DocMetadata>,  // doc_name -> metadata
    sync_facts: SyncFacts,
    resource_actions: Option<Vec<ResourceAction>>,
    delegation_templates: HashMap<String, DelegationTemplate>,  // role -> template
    proof_chain: Vec<String>,  // Parent UCAN CIDs
}

impl ResourceUcan {
    /// Parse UCAN token and extract all domain information
    pub fn from_token(token: &str) -> ResourceUcanResult<Self> {
        // Parse UCAN token
        let parsed = Ucan::try_from(token)
            .map_err(|e| ResourceUcanError::ParsingFailed(format!("UCAN parsing error: {}", e)))?;

        // Extract facts
        let facts = parsed.facts().as_ref().ok_or_else(|| {
            ResourceUcanError::MissingField("UCAN facts not found".to_string())
        })?;

        // Extract token_type from facts
        let token_type_str = facts
            .get("token_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResourceUcanError::MissingField("token_type not found in facts".to_string()))?;

        let token_type = ResourceTokenType::from_str(token_type_str)
            .map_err(|e| ResourceUcanError::InvalidTokenType(e))?;

        // Extract role from facts
        let role_str = facts
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResourceUcanError::MissingField("role not found in facts".to_string()))?;

        let role = Role::from_str(role_str)
            .map_err(|e| ResourceUcanError::ParsingFailed(format!("Invalid role: {}", e)))?;

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
                            capabilities: capabilities_map,
                            sync,
                        },
                    );
                }
            }
        }

        // Extract proof chain
        let proof_chain = parsed.proofs().clone().unwrap_or_default();

        Ok(Self {
            raw_token: token.to_string(),
            parsed,
            role,
            token_type,
            capabilities,
            doc_metadata,
            sync_facts,
            resource_actions,
            delegation_templates,
            proof_chain,
        })
    }

    // Accessors
    pub fn raw_token(&self) -> &str {
        &self.raw_token
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn token_type(&self) -> ResourceTokenType {
        self.token_type
    }

    pub fn proof_chain(&self) -> &[String] {
        &self.proof_chain
    }

    pub fn parsed(&self) -> &Ucan {
        &self.parsed
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

    // Resource/Folder ID extraction from capabilities
    pub fn resource_id(&self) -> Option<String> {
        // Extract from first capability with format: "sthalam:resource:{id}:..."
        for cap in self.parsed.capabilities().iter() {
            let parts: Vec<&str> = cap.resource.split(':').collect();
            if parts.len() >= 2 && parts[0] == "resource" {
                return Some(parts[1].to_string());
            }
        }
        None
    }

    pub fn folder_id(&self) -> Option<String> {
        // Extract from first capability with format: "sthalam:folder:{id}:..."
        for cap in self.parsed.capabilities().iter() {
            let parts: Vec<&str> = cap.resource.split(':').collect();
            if parts.len() >= 2 && parts[0] == "folder" {
                return Some(parts[1].to_string());
            }
        }
        None
    }
}
