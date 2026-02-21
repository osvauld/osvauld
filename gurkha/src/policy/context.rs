use std::collections::HashMap;

use super::types::Action;

#[derive(Debug, Clone, Default)]
pub struct ResourceContext {
    pub layer: Option<String>,
    pub path_vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct DecisionContext {
    pub actor_did: String,
    pub actor_roles: Vec<String>,
    pub audience_did: Option<String>,
    pub action: Action,
    pub resource: ResourceContext,
    pub target_fields: HashMap<String, serde_json::Value>,
    pub time_period: Option<String>,
}
