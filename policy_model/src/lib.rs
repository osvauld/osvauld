use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Create,
    Read,
    Write,
    Sync,
    Grant,
    Revoke,
    Share,
    Delegate,
}

impl Default for Action {
    fn default() -> Self {
        Self::Read
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Subject {
    Role { name: String },
    SelfActor,
    Owner,
    Creator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResourceSelector {
    Any,
    Layer {
        name: String,
    },
    LayerPath {
        layer: String,
        bindings: HashMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ref {
    ActorDid,
    ActorRole,
    TargetField { field: String },
    ResourcePathVar { var: String },
    TimePeriod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Value {
    Str(String),
    Num(i64),
    Bool(bool),
    Ref(Ref),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Predicate {
    Eq { left: Value, right: Value },
    Ne { left: Value, right: Value },
    Lt { left: Value, right: Value },
    Le { left: Value, right: Value },
    Gt { left: Value, right: Value },
    Ge { left: Value, right: Value },
    In { item: Value, set: Vec<Value> },
    Matches { value: Value, pattern: String },
    Exists { value: Value },
    RoleIs { role: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ConditionExpr {
    Pred { pred: Predicate },
    And { args: Vec<ConditionExpr> },
    Or { args: Vec<ConditionExpr> },
    Not { arg: Box<ConditionExpr> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub effect: PolicyEffect,
    pub subjects: Vec<Subject>,
    pub actions: Vec<Action>,
    pub resource: ResourceSelector,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationRule {
    pub from: Subject,
    pub to: Subject,
    pub actions: Vec<Action>,
    pub resource: ResourceSelector,
    pub max_depth: u8,
    #[serde(default)]
    pub no_escalation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicLayerSchema {
    pub name: String,
    pub path: String,
    pub namespace: String,
    pub grant: String,
    pub storage_strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyFacts {
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub policy_rules: Vec<PolicyRule>,
    #[serde(default)]
    pub delegation_rules: Vec<DelegationRule>,
    #[serde(default)]
    pub dynamic_layer_schemas: Vec<DynamicLayerSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationRequest {
    pub to_role: String,
    pub actions: Vec<Action>,
    pub resource: ResourceSelector,
    pub depth: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPolicy {
    pub app_name: String,
    pub app_version: String,
    pub policy_facts: PolicyFacts,
}
