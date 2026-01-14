use super::types::Capability;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::result::Result as StdResult;
use ucan::Ucan;

// Note: Role and ResourceTokenType removed - using facts-only approach

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

    /// Get token_type from facts
    ///
    /// **Context**: Identifying what kind of permit this is
    /// **We read**: `token_type` fact (e.g., "node_to_owner", "viewer_auth")
    pub fn token_type(&self) -> Option<&str> {
        self.get_fact_string("token_type")
    }

    /// Get audience DID from parsed UCAN
    ///
    /// **Context**: Verifying permit is meant for us
    /// **We read**: UCAN audience field
    pub fn audience(&self) -> Option<&str> {
        Some(self.parsed.audience())
    }

    /// Get issuer DID from parsed UCAN
    ///
    /// **Context**: Verifying permit came from expected party
    /// **We read**: UCAN issuer field
    pub fn issuer(&self) -> Option<&str> {
        Some(self.parsed.issuer())
    }
}

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

        facts
    }
}

/// Parsed Permit token with domain logic (role-agnostic design).
///
/// The protocol only sees capabilities and patterns, not role labels.
#[derive(Debug, Clone)]
pub struct Permit {
    /// Core permit data (raw token, parsed, facts)
    core: PermitCore,
    /// Explicit protocol capabilities (relay, share, accept_publish)
    peer_capabilities: PeerCapabilities,
    /// Fixed layer capabilities (layer_name -> capability)
    layer_capabilities: HashMap<String, Capability>,
    /// Fixed layer configs (layer_name -> config)
    layers: HashMap<String, LayerConfig>,
    /// Layer patterns for identity-based access (pattern -> config)
    /// Patterns support `{aud}`, `{page_id}` placeholders
    layer_patterns: HashMap<String, LayerPatternConfig>,
    /// Delegation templates (template_key -> template) - legacy lookup by role
    delegation_templates: HashMap<String, DelegationTemplate>,
    /// Self-describing templates: what to issue when holder takes actions
    /// Keys: "page_request", "share_link", etc.
    issue_on: HashMap<String, DelegationTemplate>,
    /// Parent permit CIDs for delegation chain
    proof_chain: Vec<String>,
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

        // Convert BTreeMap to serde_json::Map
        let facts: serde_json::Map<String, serde_json::Value> = facts_ref
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        trace!(facts = ?facts.keys().collect::<Vec<_>>(), "Facts extracted");

        // Extract peer_capabilities (role-agnostic protocol capabilities)
        let peer_capabilities = facts
            .get("peer_capabilities")
            .and_then(|v| v.as_object())
            .map(PeerCapabilities::from_json)
            .unwrap_or_default();

        // Extract layer_patterns for identity-based access
        // Format: { "{page_id}/*/{aud}": { "create": true, "sync": true } }
        let mut layer_patterns = HashMap::new();
        if let Some(patterns_obj) = facts.get("layer_patterns").and_then(|v| v.as_object()) {
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
        // Format: { "products": { "sync": true, "write": false, "type": "list" } }
        let mut layers = HashMap::new();
        let mut layer_capabilities = HashMap::new();
        if let Some(layers_obj) = facts.get("layers").and_then(|v| v.as_object()) {
            for (layer_name, layer_val) in layers_obj {
                if let Some(layer_obj) = layer_val.as_object() {
                    let sync = layer_obj
                        .get("sync")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let write = layer_obj
                        .get("write")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let layer_type = layer_obj
                        .get("type")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    layers.insert(layer_name.clone(), LayerConfig {
                        sync,
                        write,
                        layer_type,
                    });

                    // Also populate layer_capabilities for backward compat
                    let cap = if write { Capability::ReadWrite } else { Capability::ReadOnly };
                    layer_capabilities.insert(layer_name.clone(), cap);
                }
            }
        }

        // Extract delegation templates (using helper function for consistency)
        let mut delegation_templates = HashMap::new();
        if let Some(delegation_obj) = facts.get("delegation").and_then(|v| v.as_object()) {
            for (role_key, template_val) in delegation_obj {
                if let Some(template) = Self::parse_delegation_template(role_key, template_val) {
                    delegation_templates.insert(role_key.clone(), template);
                }
            }
        }

        // Extract issue_on templates (self-describing: what to issue on actions)
        // Format: { "page_request": { token_type, layers, layer_patterns, ... }, "share_link": { ... } }
        let mut issue_on = HashMap::new();
        if let Some(issue_on_obj) = facts.get("issue_on").and_then(|v| v.as_object()) {
            for (action_key, template_val) in issue_on_obj {
                if let Some(template) = Self::parse_delegation_template(action_key, template_val) {
                    issue_on.insert(action_key.clone(), template);
                }
            }
        }

        // Extract proof chain
        let proof_chain = parsed.proofs().clone().unwrap_or_default();

        // Create PermitCore with parsed data
        let core = PermitCore::new(token.to_string(), parsed, facts.clone());

        Ok(Self {
            core,
            peer_capabilities,
            layer_capabilities,
            layers,
            layer_patterns,
            delegation_templates,
            issue_on,
            proof_chain,
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

        Some(DelegationTemplate {
            token_type,
            peer_capabilities,
            operations,
            auth_capabilities,
            layer_patterns,
            layers,
            relationship,
            issue_on,
        })
    }

    // Accessors - delegate to core for common fields
    pub fn raw_token(&self) -> &str {
        self.core.raw_token()
    }

    pub fn parsed(&self) -> &Ucan {
        self.core.parsed()
    }

    /// Get the core permit data for handshake decisions
    pub fn core(&self) -> &PermitCore {
        &self.core
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

    // ==================== Peer Capabilities (Role-Agnostic) ====================

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

    // ==================== Layer Access ====================

    /// Get fixed layer capability
    pub fn get_layer_capability(&self, layer: &str) -> Option<Capability> {
        self.layer_capabilities.get(layer).copied()
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

    // ==================== Delegation Templates ====================

    /// Get delegation template by role key (legacy lookup)
    pub fn get_delegation_template(&self, role: &str) -> Option<&DelegationTemplate> {
        self.delegation_templates.get(role)
    }

    /// Get all delegation templates
    pub fn delegation_templates(&self) -> &HashMap<String, DelegationTemplate> {
        &self.delegation_templates
    }

    // ==================== Self-Describing Templates (issue_on) ====================

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

    // ==================== ID Extraction ====================

    /// Extract page_id from token facts
    pub fn page_id(&self) -> Option<String> {
        self.get_fact_string("page_id").map(String::from)
    }

    /// Extract space_id from token facts
    pub fn space_id(&self) -> Option<String> {
        self.get_fact_string("space_id").map(String::from)
    }
}
