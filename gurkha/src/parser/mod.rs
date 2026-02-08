use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::result::Result as StdResult;
use ucan::Ucan;

use crate::types::SyncFacts;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_tests;

/// Layer pattern configuration for viewer access
///
/// Defines what operations a viewer can perform on layers matching a pattern.
/// Patterns support `{aud}` placeholder for viewer DID substitution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerPatternConfig {
    /// Can create new layers matching this pattern (node-side)
    #[serde(default)]
    pub create: bool,
    /// Can sync layers matching this pattern
    #[serde(default)]
    pub sync: bool,
    /// Can write to existing layers matching this pattern (client-side)
    #[serde(default)]
    pub write: bool,
}

/// Explicit protocol capabilities (role-agnostic)
///
/// These capabilities control what protocol operations a peer can perform.
/// They are explicitly defined in permits, not derived from role labels.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeerCapabilities {
    /// Can relay/forward data to other peers (node needs this)
    #[serde(default)]
    pub relay: bool,
    /// Can issue delegated permits to others (node needs this for sharing)
    #[serde(default)]
    pub share: bool,
    /// Can accept published spaces from this peer
    #[serde(default)]
    pub accept_publish: bool,
}

impl PeerCapabilities {
    /// Parse capabilities from a JSON object
    pub fn from_json(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self {
            relay: obj.get("relay").and_then(|v| v.as_bool()).unwrap_or(false),
            share: obj.get("share").and_then(|v| v.as_bool()).unwrap_or(false),
            accept_publish: obj.get("accept_publish").and_then(|v| v.as_bool()).unwrap_or(false),
        }
    }

    /// Convert to JSON value for permit facts
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "relay": self.relay,
            "share": self.share,
            "accept_publish": self.accept_publish
        })
    }
}

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

/// Delegation template for a specific role (role-agnostic design)
///
/// Roles like "customer", "admin", "node" are just labels that map to
/// different capability and pattern sets. The protocol only sees capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationTemplate {
    /// Token type for delegated token (e.g., "page_viewer", "page_share")
    pub token_type: String,
    /// Explicit protocol capabilities (relay, share, accept_publish)
    #[serde(default)]
    pub peer_capabilities: PeerCapabilities,
    /// Operations that this role can perform (e.g., "add_pages", "share_page")
    /// operation_name -> "allow" | "deny"
    #[serde(default)]
    pub operations: HashMap<String, String>,
    /// Auth capabilities (can_connect, sync_enabled, etc.)
    #[serde(default)]
    pub auth_capabilities: HashMap<String, serde_json::Value>,
    /// Layer patterns with placeholders like {page_id}, {aud}
    /// Pattern -> { create: bool, sync: bool }
    #[serde(default)]
    pub layer_patterns: HashMap<String, LayerPatternConfig>,
    /// Fixed layers (non-pattern) with their capabilities
    /// layer_name -> { capability: "viewer"|"collaborator", sync: bool }
    #[serde(default)]
    pub layers: HashMap<String, LayerConfig>,
    /// Relationship label (for logging/debugging only, NOT for business logic)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship: Option<String>,
    /// Self-describing templates: what to issue when holder takes actions
    /// Keys: "page_request", "share_link", etc.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub issue_on: HashMap<String, Box<DelegationTemplate>>,
    /// Presence configuration (visibility, display name)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence: Option<PresenceConfig>,
    /// Allowed ephemeral function names (empty = all allowed)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ephemeral_funcs: Vec<String>,
}

/// Configuration for a fixed (non-pattern) layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConfig {
    /// Whether to sync this layer
    #[serde(default)]
    pub sync: bool,
    /// Whether holder can write to this layer
    #[serde(default)]
    pub write: bool,
    /// Layer type: "list", "map", "text"
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub layer_type: Option<String>,
}

/// Presence configuration for a peer
///
/// Controls visibility in the /users layer and presence events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PresenceConfig {
    /// Whether this peer is visible to others (presence layer entry written)
    #[serde(default)]
    pub visible: bool,
    /// Whether this peer can see others' presence (receives presence layer sync)
    #[serde(default = "default_true")]
    pub can_see_others: bool,
    /// Display name for this peer (shown in presence layer)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn default_true() -> bool { true }

/// Internal struct for deserializing permit facts via serde
///
/// This replaces ~80 lines of manual JSON extraction with a single deserialize call.
#[derive(Debug, Clone, Default, Deserialize)]
struct PermitFacts {
    #[serde(default)]
    peer_capabilities: PeerCapabilities,
    #[serde(default)]
    layer_patterns: HashMap<String, LayerPatternConfig>,
    #[serde(default)]
    layers: HashMap<String, LayerConfig>,
    #[serde(default)]
    issue_on: HashMap<String, serde_json::Value>,
    /// Sync behavior facts (local_only, no_incoming_updates, send_full_snapshot)
    #[serde(default)]
    sync: SyncFacts,
    /// Presence configuration (visibility, display name)
    #[serde(default)]
    presence: Option<PresenceConfig>,
    /// Allowed ephemeral function names (empty = all allowed)
    #[serde(default)]
    ephemeral_funcs: Vec<String>,
}

impl DelegationTemplate {
    /// Convert template to permit facts JSON
    ///
    /// This creates the facts structure that will be embedded in the delegated token.
    /// Role-agnostic: only includes capabilities and patterns, no role-based logic.
    pub fn to_facts(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut facts = serde_json::Map::new();

        // Add token_type
        facts.insert("token_type".to_string(), serde_json::Value::String(self.token_type.clone()));

        // Add peer_capabilities
        facts.insert("peer_capabilities".to_string(), self.peer_capabilities.to_json());

        // Add operations (required for delegation chain - e.g., add_pages permission)
        if !self.operations.is_empty() {
            let ops_json: serde_json::Map<String, serde_json::Value> = self.operations
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            facts.insert("operations".to_string(), serde_json::Value::Object(ops_json));
        }

        // Add auth_capabilities
        if !self.auth_capabilities.is_empty() {
            let auth_json: serde_json::Map<String, serde_json::Value> = self.auth_capabilities
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            facts.insert("auth_capabilities".to_string(), serde_json::Value::Object(auth_json));
        }

        // Add layer_patterns
        if !self.layer_patterns.is_empty() {
            let patterns_json: serde_json::Map<String, serde_json::Value> = self.layer_patterns
                .iter()
                .map(|(pattern, config)| {
                    (pattern.clone(), serde_json::json!({
                        "create": config.create,
                        "sync": config.sync,
                        "write": config.write
                    }))
                })
                .collect();
            facts.insert("layer_patterns".to_string(), serde_json::Value::Object(patterns_json));
        }

        // Add fixed layers
        if !self.layers.is_empty() {
            let layers_json: serde_json::Map<String, serde_json::Value> = self.layers
                .iter()
                .map(|(name, config)| {
                    let mut layer_obj = serde_json::Map::new();
                    layer_obj.insert("sync".to_string(), serde_json::Value::Bool(config.sync));
                    layer_obj.insert("write".to_string(), serde_json::Value::Bool(config.write));
                    if let Some(ref t) = config.layer_type {
                        layer_obj.insert("type".to_string(), serde_json::Value::String(t.clone()));
                    }
                    (name.clone(), serde_json::Value::Object(layer_obj))
                })
                .collect();
            facts.insert("layers".to_string(), serde_json::Value::Object(layers_json));
        }

        // Add relationship (for logging only)
        if let Some(rel) = &self.relationship {
            facts.insert("relationship".to_string(), serde_json::Value::String(rel.clone()));
        }

        // Add issue_on (self-describing templates for what to issue on actions)
        if !self.issue_on.is_empty() {
            let issue_on_json: serde_json::Map<String, serde_json::Value> = self.issue_on
                .iter()
                .map(|(action, template)| {
                    (action.clone(), serde_json::Value::Object(template.to_facts()))
                })
                .collect();
            facts.insert("issue_on".to_string(), serde_json::Value::Object(issue_on_json));
        }

        // Add presence configuration
        if let Some(ref presence) = self.presence {
            let mut presence_json = serde_json::Map::new();
            presence_json.insert("visible".to_string(), serde_json::Value::Bool(presence.visible));
            presence_json.insert("can_see_others".to_string(), serde_json::Value::Bool(presence.can_see_others));
            if let Some(ref name) = presence.name {
                presence_json.insert("name".to_string(), serde_json::Value::String(name.clone()));
            }
            facts.insert("presence".to_string(), serde_json::Value::Object(presence_json));
        }

        // Add ephemeral_funcs (allowed ephemeral function names)
        if !self.ephemeral_funcs.is_empty() {
            let funcs: Vec<serde_json::Value> = self.ephemeral_funcs
                .iter()
                .map(|f| serde_json::Value::String(f.clone()))
                .collect();
            facts.insert("ephemeral_funcs".to_string(), serde_json::Value::Array(funcs));
        }

        facts
    }
}

/// Parsed Permit token with domain logic (role-agnostic design).
///
/// The protocol only sees capabilities and patterns, not role labels.
/// Contains all permit data including raw token, parsed UCAN, and extracted facts.
#[derive(Debug, Clone)]
pub struct Permit {
    // Core permit data (merged from PermitCore)
    /// Raw permit token string
    raw_token: String,
    /// Parsed token (uses ucan library internally)
    parsed: Ucan,
    /// Raw facts as JSON (source of truth)
    facts: serde_json::Map<String, serde_json::Value>,

    // Extracted/parsed fields
    /// Explicit protocol capabilities (relay, share, accept_publish)
    peer_capabilities: PeerCapabilities,
    /// Fixed layer configs (layer_name -> config)
    layers: HashMap<String, LayerConfig>,
    /// Layer patterns for identity-based access (pattern -> config)
    /// Patterns support `{aud}`, `{page_id}` placeholders
    layer_patterns: HashMap<String, LayerPatternConfig>,
    /// Self-describing templates: what to issue when holder takes actions
    /// Keys: "page_request", "share_link", etc.
    issue_on: HashMap<String, DelegationTemplate>,
    /// Parent permit CIDs for delegation chain
    proof_chain: Vec<String>,
    /// Sync behavior facts (local_only, no_incoming_updates, send_full_snapshot)
    sync_facts: SyncFacts,
    /// Presence configuration (visibility, display name)
    presence: Option<PresenceConfig>,
    /// Allowed ephemeral function names (empty = all allowed)
    ephemeral_funcs: Vec<String>,
}

impl Permit {
    /// Parse permit token and extract all domain information.
    ///
    /// Role-agnostic: extracts capabilities and patterns, not role labels.
    pub fn from_token(token: &str) -> PermitResult<Self> {
        use tracing::trace;

        trace!("Parsing permit token (len={})", token.len());

        // Parse permit token (uses ucan library internally)
        let parsed = Ucan::try_from(token)
            .map_err(|e| PermitError::ParsingFailed(format!("Permit parsing error: {}", e)))?;

        trace!("JWT decoded - issuer: {}, audience: {}", parsed.issuer(), parsed.audience());

        // Extract facts as raw JSON (source of truth)
        let facts_ref = parsed.facts().as_ref().ok_or_else(|| {
            PermitError::MissingField("Permit facts not found".to_string())
        })?;

        // Convert BTreeMap to serde_json::Value for deserialization
        let facts_value: serde_json::Value = serde_json::to_value(facts_ref)
            .map_err(|e| PermitError::ParsingFailed(format!("Facts conversion error: {}", e)))?;

        // Deserialize structured facts via serde (replaces ~60 lines of manual parsing)
        let parsed_facts: PermitFacts = serde_json::from_value(facts_value.clone())
            .unwrap_or_default();

        // Keep raw facts map for get_fact() lookups
        let facts: serde_json::Map<String, serde_json::Value> = facts_ref
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        trace!(facts = ?facts.keys().collect::<Vec<_>>(), "Facts extracted");

        // Parse issue_on templates
        let issue_on: HashMap<String, DelegationTemplate> = parsed_facts.issue_on
            .iter()
            .filter_map(|(key, val)| {
                Self::parse_delegation_template(key, val).map(|t| (key.clone(), t))
            })
            .collect();

        // Extract proof chain
        let proof_chain = parsed.proofs().clone().unwrap_or_default();

        Ok(Self {
            raw_token: token.to_string(),
            parsed,
            facts,
            peer_capabilities: parsed_facts.peer_capabilities,
            layers: parsed_facts.layers,
            layer_patterns: parsed_facts.layer_patterns,
            issue_on,
            proof_chain,
            sync_facts: parsed_facts.sync,
            presence: parsed_facts.presence,
            ephemeral_funcs: parsed_facts.ephemeral_funcs,
        })
    }

    /// Parse a DelegationTemplate from a JSON value
    fn parse_delegation_template(key: &str, template_val: &serde_json::Value) -> Option<DelegationTemplate> {
        let template_obj = template_val.as_object()?;

        // Extract token_type
        let token_type = template_obj
            .get("token_type")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("delegated_{}", key));

        // Extract peer_capabilities
        let peer_capabilities = template_obj
            .get("peer_capabilities")
            .and_then(|v| v.as_object())
            .map(PeerCapabilities::from_json)
            .unwrap_or_default();

        // Extract layer_patterns
        let mut layer_patterns = HashMap::new();
        if let Some(patterns_obj) = template_obj.get("layer_patterns").and_then(|v| v.as_object()) {
            for (pattern, config) in patterns_obj {
                if let Some(config_obj) = config.as_object() {
                    layer_patterns.insert(
                        pattern.clone(),
                        LayerPatternConfig {
                            create: config_obj.get("create").and_then(|v| v.as_bool()).unwrap_or(false),
                            sync: config_obj.get("sync").and_then(|v| v.as_bool()).unwrap_or(false),
                            write: config_obj.get("write").and_then(|v| v.as_bool()).unwrap_or(false),
                        },
                    );
                }
            }
        }

        // Extract fixed layers
        let mut layers = HashMap::new();
        if let Some(layers_obj) = template_obj.get("layers").and_then(|v| v.as_object()) {
            for (layer_name, layer_val) in layers_obj {
                if let Some(layer_obj) = layer_val.as_object() {
                    layers.insert(layer_name.clone(), LayerConfig {
                        sync: layer_obj.get("sync").and_then(|v| v.as_bool()).unwrap_or(false),
                        write: layer_obj.get("write").and_then(|v| v.as_bool()).unwrap_or(false),
                        layer_type: layer_obj.get("type").and_then(|v| v.as_str()).map(String::from),
                    });
                }
            }
        }

        // Extract relationship (for logging only)
        let relationship = template_obj
            .get("relationship")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Extract operations (e.g., add_pages, share_page)
        let mut operations = HashMap::new();
        if let Some(ops_obj) = template_obj.get("operations").and_then(|v| v.as_object()) {
            for (op_name, op_val) in ops_obj {
                if let Some(op_str) = op_val.as_str() {
                    operations.insert(op_name.clone(), op_str.to_string());
                }
            }
        }

        // Extract auth_capabilities (can_connect, sync_enabled, etc.)
        let mut auth_capabilities = HashMap::new();
        if let Some(auth_obj) = template_obj.get("auth_capabilities").and_then(|v| v.as_object()) {
            for (cap_name, cap_val) in auth_obj {
                auth_capabilities.insert(cap_name.clone(), cap_val.clone());
            }
        }

        // Extract issue_on (recursive - templates for what to issue on actions)
        let mut issue_on = HashMap::new();
        if let Some(issue_on_obj) = template_obj.get("issue_on").and_then(|v| v.as_object()) {
            for (action_key, nested_template_val) in issue_on_obj {
                if let Some(nested_template) = Self::parse_delegation_template(action_key, nested_template_val) {
                    issue_on.insert(action_key.clone(), Box::new(nested_template));
                }
            }
        }

        // Extract presence configuration
        let presence = template_obj.get("presence").and_then(|v| v.as_object()).map(|p| {
            PresenceConfig {
                visible: p.get("visible").and_then(|v| v.as_bool()).unwrap_or(false),
                can_see_others: p.get("can_see_others").and_then(|v| v.as_bool()).unwrap_or(true),
                name: p.get("name").and_then(|v| v.as_str()).map(String::from),
            }
        });

        // Extract ephemeral_funcs
        let ephemeral_funcs = template_obj.get("ephemeral_funcs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Some(DelegationTemplate {
            token_type,
            peer_capabilities,
            operations,
            auth_capabilities,
            layer_patterns,
            layers,
            relationship,
            issue_on,
            presence,
            ephemeral_funcs,
        })
    }

    /// Get the raw token string
    pub fn raw_token(&self) -> &str {
        &self.raw_token
    }

    /// Get the parsed UCAN token
    pub fn parsed(&self) -> &Ucan {
        &self.parsed
    }

    pub fn proof_chain(&self) -> &[String] {
        &self.proof_chain
    }

    /// Get raw facts (source of truth)
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

    /// Get token_type from facts
    pub fn token_type(&self) -> Option<&str> {
        self.get_fact_string("token_type")
    }

    /// Get audience DID from parsed UCAN
    pub fn audience(&self) -> Option<&str> {
        Some(self.parsed.audience())
    }

    /// Get issuer DID from parsed UCAN
    pub fn issuer(&self) -> Option<&str> {
        Some(self.parsed.issuer())
    }

    /// Get explicit protocol capabilities
    pub fn peer_capabilities(&self) -> &PeerCapabilities {
        &self.peer_capabilities
    }

    /// Check if peer can relay data to others
    pub fn can_relay(&self) -> bool {
        self.peer_capabilities.relay
    }

    /// Check if peer can issue delegated permits
    pub fn can_share(&self) -> bool {
        self.peer_capabilities.share
    }

    /// Check if peer can accept published spaces
    pub fn can_accept_publish(&self) -> bool {
        self.peer_capabilities.accept_publish
    }

    /// Get fixed layer config
    pub fn get_layer_config(&self, layer: &str) -> Option<&LayerConfig> {
        self.layers.get(layer)
    }

    /// Get all fixed layers
    pub fn layers(&self) -> &HashMap<String, LayerConfig> {
        &self.layers
    }

    /// Get layer patterns for identity-based access control
    ///
    /// Patterns support `{aud}`, `{page_id}` placeholders.
    /// Example: `{page_id}/*/{aud}` expands to `shop123/orders/did:key:viewer123`
    pub fn layer_patterns(&self) -> &HashMap<String, LayerPatternConfig> {
        &self.layer_patterns
    }

    /// Check if permit has any layer patterns defined
    pub fn has_layer_patterns(&self) -> bool {
        !self.layer_patterns.is_empty()
    }

    /// Get layers that are local-only (sync=false)
    ///
    /// These layers should not be sent to peers during sync.
    pub fn local_only_layers(&self) -> Vec<String> {
        self.layers
            .iter()
            .filter(|(_, config)| !config.sync)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get layers that don't accept incoming updates (no_incoming_updates)
    ///
    /// Derived from layers where the peer has no write permission.
    pub fn no_incoming_update_layers(&self) -> Vec<String> {
        self.layers
            .iter()
            .filter(|(_, config)| {
                // No write permission = no incoming updates
                !config.write
            })
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get sync behavior facts (local_only, no_incoming_updates, send_full_snapshot)
    ///
    /// **Context**: Used by scribe to determine sync behavior for layers
    /// **Fields**:
    /// - `local_only`: Layers that stay local (never synced)
    /// - `no_incoming_updates`: Layers that don't accept incoming updates
    /// - `send_full_snapshot`: Layers that send full snapshots instead of incremental
    pub fn sync_facts(&self) -> &SyncFacts {
        &self.sync_facts
    }

    /// Get template for what to issue when an action occurs
    ///
    /// Self-describing permits carry embedded templates for what to issue next.
    /// This eliminates role-based lookups - the permit knows what it can delegate.
    ///
    /// # Arguments
    /// * `action` - The action triggering delegation: "page_request", "share_link", etc.
    ///
    /// # Example
    /// ```ignore
    /// // When viewer requests a page, use embedded template
    /// let template = permit.get_issue_template("page_request")?;
    /// issue_permit_from_template(template, viewer_did)
    /// ```
    pub fn get_issue_template(&self, action: &str) -> Option<&DelegationTemplate> {
        self.issue_on.get(action)
    }

    /// Get all issue_on templates
    pub fn issue_on_templates(&self) -> &HashMap<String, DelegationTemplate> {
        &self.issue_on
    }

    /// Check if permit has any issue_on templates
    pub fn has_issue_templates(&self) -> bool {
        !self.issue_on.is_empty()
    }

    /// Extract page_id from token facts
    pub fn page_id(&self) -> Option<String> {
        self.get_fact_string("page_id").map(String::from)
    }

    /// Extract space_id from token facts
    pub fn space_id(&self) -> Option<String> {
        self.get_fact_string("space_id").map(String::from)
    }

    /// Extract user_id from token facts (base64-encoded public key)
    pub fn user_id(&self) -> Option<String> {
        self.get_fact_string("user_id").map(String::from)
    }

    /// Get presence configuration
    pub fn presence(&self) -> Option<&PresenceConfig> {
        self.presence.as_ref()
    }

    /// Check if this peer is visible to others (join/leave events, /users layer)
    pub fn is_visible(&self) -> bool {
        self.presence.as_ref().map(|p| p.visible).unwrap_or(false)
    }

    /// Check if this peer can see others' presence (receives presence layer sync)
    pub fn can_see_others(&self) -> bool {
        self.presence.as_ref().map(|p| p.can_see_others).unwrap_or(true)
    }

    /// Get display name for this peer (from presence config)
    pub fn display_name(&self) -> Option<&str> {
        self.presence.as_ref().and_then(|p| p.name.as_deref())
    }

    /// Check if permit allows sending a specific ephemeral function
    ///
    /// **Context**: Used to validate outgoing ephemerals against permit
    /// **Logic**: Empty list = all allowed, otherwise func must be in list
    pub fn can_send_ephemeral(&self, func: &str) -> bool {
        self.ephemeral_funcs.is_empty() || self.ephemeral_funcs.contains(&func.to_string())
    }

    /// Get list of allowed ephemeral function names
    pub fn ephemeral_funcs(&self) -> &[String] {
        &self.ephemeral_funcs
    }

    // These methods accept page_id and did for pattern expansion

    /// Check if a layer should sync based on permit configuration
    ///
    /// **Context**: Used to filter out local-only layers (sync: false)
    /// **Checks**:
    /// 1. Named layers in `layers` section - check `sync` flag
    /// 2. Pattern matches in `layer_patterns` section - check `sync` flag
    /// **Returns**: true if layer should sync, false if local-only
    pub fn should_sync_layer(&self, layer_name: &str, page_id: &str, our_did: &str) -> bool {
        // 1. Check named layers section
        for (pattern, config) in &self.layers {
            let expanded = expand_pattern(pattern, page_id, our_did);
            if expanded == layer_name {
                return config.sync;
            }
        }

        // 2. Check layer_patterns section
        for (pattern, config) in &self.layer_patterns {
            let expanded = expand_pattern(pattern, page_id, our_did);
            if matches_wildcard(layer_name, &expanded) {
                return config.sync;
            }
        }

        // Default: sync
        true
    }

    /// Check if permit allows writing to a layer
    ///
    /// **Checks**:
    /// 1. Fixed layer permission (write=true)
    /// 2. Pattern match with write or create permission
    pub fn can_write_layer(&self, layer_name: &str, page_id: &str, our_did: &str) -> bool {
        // 1. Check fixed layers
        for (pattern, config) in &self.layers {
            let expanded = expand_pattern(pattern, page_id, our_did);
            if expanded == layer_name && config.write {
                return true;
            }
        }

        // 2. Check layer_patterns
        for (pattern, config) in &self.layer_patterns {
            let expanded = expand_pattern(pattern, page_id, our_did);
            if matches_wildcard(layer_name, &expanded) && (config.write || config.create) {
                return true;
            }
        }

        false
    }

    /// Check if permit allows reading/syncing a layer
    ///
    /// **Checks**:
    /// 1. Fixed layer exists with sync=true
    /// 2. Pattern match with sync permission
    pub fn can_read_layer(&self, layer_name: &str, page_id: &str, our_did: &str) -> bool {
        // 1. Check fixed layers
        for (pattern, config) in &self.layers {
            let expanded = expand_pattern(pattern, page_id, our_did);
            if expanded == layer_name && config.sync {
                return true;
            }
        }

        // 2. Check layer_patterns
        for (pattern, config) in &self.layer_patterns {
            let expanded = expand_pattern(pattern, page_id, our_did);
            if matches_wildcard(layer_name, &expanded) && config.sync {
                return true;
            }
        }

        false
    }

    /// Get all static layers (fully expanded, no wildcards) for pre-creation
    ///
    /// **Context**: Scribe pre-creates layers that are fully known from permit
    /// **Returns**: Layer names after expanding {page_id} and {aud}
    pub fn static_layers(&self, page_id: &str, our_did: &str) -> Vec<String> {
        let mut layers = Vec::new();

        // Fixed layers with placeholders expanded
        for pattern in self.layers.keys() {
            let expanded = expand_pattern(pattern, page_id, our_did);
            // Only include if no wildcards remain
            if !expanded.contains('*') {
                layers.push(expanded);
            }
        }

        // Pattern layers that are fully expanded (no wildcards)
        for pattern in self.layer_patterns.keys() {
            let expanded = expand_pattern(pattern, page_id, our_did);
            if !expanded.contains('*') {
                layers.push(expanded);
            }
        }

        layers
    }
}

// Pattern Expansion Helpers

/// Expand placeholders in a pattern
///
/// **Placeholders:**
/// - `{page_id}` → replaced with page_id
/// - `{aud}` → replaced with did
///
/// **Example:** `{page_id}/orders/{aud}` → `shop123/orders/did:key:abc`
pub fn expand_pattern(pattern: &str, page_id: &str, did: &str) -> String {
    pattern
        .replace("{page_id}", page_id)
        .replace("{aud}", did)
}

/// Match layer name against pattern with path-segment wildcards
///
/// **Pattern:** `*` matches any single path segment (delimited by `/`)
///
/// **Examples:**
/// - `shop123/*/did:key:abc` matches `shop123/orders/did:key:abc`
/// - `shop123/*/*` matches `shop123/orders/did:key:abc`
/// - `shop123/*` does NOT match `shop123/orders/did:key:abc` (different segment count)
pub fn matches_wildcard(layer_name: &str, pattern: &str) -> bool {
    let layer_parts: Vec<&str> = layer_name.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();

    if layer_parts.len() != pattern_parts.len() {
        return false;
    }

    layer_parts.iter().zip(pattern_parts.iter()).all(|(layer, pat)| {
        *pat == "*" || layer == pat
    })
}
