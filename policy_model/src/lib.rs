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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SchemaArtifact {
    #[serde(default)]
    pub entities: HashMap<String, EntitySchema>,
    #[serde(default)]
    pub layer_bindings: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EntitySchema {
    #[serde(default)]
    pub fields: Vec<FieldSchema>,
    #[serde(default)]
    pub transitions: Vec<TransitionSchema>,
    #[serde(default)]
    pub entity_rules: Vec<EntityRuleSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldSchema {
    pub name: String,
    pub required: bool,
    pub immutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionSchema {
    pub field: String,
    pub from: String,
    pub to: String,
    pub allowed_roles: Vec<String>,
    #[serde(default)]
    pub predicates: Vec<Predicate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntityRuleSchema {
    OnUpdateSet {
        field: String,
        source: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        literal: Option<LiteralValue>,
    },
    OnDeleteReject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiteralValue {
    String(String),
    Int(i64),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ValidationArtifact {
    #[serde(default)]
    pub lua_validators: HashMap<String, String>,
    #[serde(default)]
    pub broadcasts: HashMap<String, Vec<BroadcastPolicy>>,
    #[serde(default)]
    pub orderings: HashMap<String, OrderPolicy>,
    #[serde(default)]
    pub dynamic_layers: HashMap<String, DynamicLayerPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicLayerPolicy {
    #[serde(default)]
    pub create_allow: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discover: Option<DiscoverMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverMode {
    Sync,
    Grant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BroadcastPolicy {
    pub target: BroadcastTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debounce_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BroadcastTarget {
    All,
    Granted,
    ToRoles(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderPolicy {
    pub field: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Ascending,
    Descending,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<SchemaArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationArtifact>,
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
