use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use crate::ast::{
    sanitize_app_name, Action, AppSpec, BroadcastTarget, DiscoverMode, EntityActionSource,
    EntityRule, FieldSource, GrantMode, LayerNamespace, PermitStmt, Predicate, PredicateExpr,
    RelayEvent, Resolution, SortDirection, TimeModel, ValueRef,
};
use policy_model::{
    Action as PolicyAction, DelegationRule, DynamicLayerSchema, PolicyEffect, PolicyFacts,
    PolicyRule, Ref, ResourceSelector, Subject, Value,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledArtifacts {
    pub permit: PermitArtifact,
    pub runtime: RuntimeArtifact,
    pub schema: SchemaArtifact,
    pub validation: ValidationArtifact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermitArtifact {
    pub osv_policy: PolicyFacts,
    pub roles: Vec<String>,
    pub dynamic_layer_schemas: Vec<DynamicLayerSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeArtifact {
    pub app_name: String,
    pub app_version: String,
    pub ui_apps: Vec<UiAppArtifact>,
    /// Sanitized layer names for all ui_apps (code layers).
    /// Used by scribe/butler to identify code layers without prefix matching.
    pub app_layer_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiAppArtifact {
    pub name: String,
    /// Sanitized code layer name derived from ui_app display name.
    /// Example: "Group Chat" → "group_chat"
    pub layer_name: String,
    pub allow_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaArtifact {
    pub entities: HashMap<String, EntitySchema>,
    pub layer_bindings: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchema {
    pub fields: Vec<FieldSchema>,
    pub transitions: Vec<TransitionSchema>,
    pub entity_rules: Vec<EntityRuleSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
    pub name: String,
    pub required: bool,
    pub immutable: bool,
    pub default: Option<DefaultValue>,
    pub source: Option<FieldSourceSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultValue {
    String(String),
    Int(u32),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldSourceSchema {
    Clock,
    PeerDid,
    PeerRole,
    Mode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionSchema {
    pub field: String,
    pub from: String,
    pub to: String,
    pub allowed_roles: Vec<String>,
    pub predicates: Vec<Predicate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntityRuleSchema {
    OnUpdateSet {
        field: String,
        source: EntityActionSource,
    },
    OnDeleteReject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationArtifact {
    pub lua_validators: HashMap<String, String>,
    pub derivations: Vec<DerivationDef>,
    pub relays: Vec<RelayDef>,
    pub broadcasts: HashMap<String, Vec<BroadcastPolicy>>,
    pub orderings: HashMap<String, OrderPolicy>,
    pub dynamic_layers: HashMap<String, DynamicLayerPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationDef {
    pub source_layer: String,
    pub target_layer: String,
    pub handler: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayDef {
    pub layer: String,
    pub event: RelayEvent,
    pub handler: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastPolicy {
    pub target: BroadcastTarget,
    pub debounce_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPolicy {
    pub field: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicLayerPolicy {
    pub dynamic_by: Vec<String>,
    pub create_allow: Vec<String>,
    pub discover: Option<DiscoverMode>,
    pub retain_days: Option<u32>,
    pub shard_resolution: Option<Resolution>,
}

pub fn compile(spec: &AppSpec) -> CompiledArtifacts {
    let layer_paths_by_name: HashMap<&str, &str> = spec
        .layers
        .iter()
        .map(|layer| (layer.name.as_str(), layer.path.as_str()))
        .collect();

    let roles = spec.roles.iter().map(|r| r.name.clone()).collect();
    let dynamic_layer_schemas: Vec<DynamicLayerSchema> = spec
        .layers
        .iter()
        .map(|layer| DynamicLayerSchema {
            name: layer.name.clone(),
            path: layer.path.clone(),
            namespace: match layer.namespace {
                LayerNamespace::Shared => "shared".to_string(),
                LayerNamespace::Page => "page".to_string(),
            },
            grant: match layer.grant {
                GrantMode::Open => "open".to_string(),
                GrantMode::Explicit => "explicit".to_string(),
                GrantMode::RoleScoped => "role_scoped".to_string(),
            },
            storage_strategy: match layer.time {
                TimeModel::Unsharded => "single_doc".to_string(),
                TimeModel::TimeSharded { .. } => "time_sharded".to_string(),
            },
            resolution: match &layer.time {
                TimeModel::Unsharded => None,
                TimeModel::TimeSharded { resolution, .. } => {
                    Some(format!("{}", format_resolution(resolution)))
                }
            },
        })
        .collect();

    let mut policy_rules = spec
        .layers
        .iter()
        .flat_map(|layer| {
            layer.access.iter().map(|rule| PolicyRule {
                effect: PolicyEffect::Allow,
                subjects: rule
                    .roles
                    .iter()
                    .map(|role| Subject::Role { name: role.clone() })
                    .collect(),
                actions: rule.actions.iter().map(format_action).collect(),
                resource: ResourceSelector::Layer {
                    name: layer.path.clone(),
                },
                condition: lower_predicate_expr(rule.predicate.as_ref()),
            })
        })
        .collect::<Vec<_>>();

    policy_rules.extend(lower_permit_policy_rules(spec, &layer_paths_by_name));

    let delegation_rules = lower_delegation_rules(spec, &layer_paths_by_name);
    let schema = lower_schema_artifact(spec, &layer_paths_by_name);
    let validation = lower_validation_artifact(spec, &layer_paths_by_name);

    let ui_apps: Vec<UiAppArtifact> = spec
        .ui_apps
        .iter()
        .map(|app| UiAppArtifact {
            name: app.name.clone(),
            layer_name: sanitize_app_name(&app.name),
            allow_roles: app.allowed_roles.clone(),
        })
        .collect();

    let app_layer_names: Vec<String> = ui_apps.iter().map(|a| a.layer_name.clone()).collect();

    CompiledArtifacts {
        permit: PermitArtifact {
            osv_policy: PolicyFacts {
                roles: spec.roles.iter().map(|r| r.name.clone()).collect(),
                policy_rules,
                delegation_rules,
                dynamic_layer_schemas: dynamic_layer_schemas.clone(),
                schema: Some(to_policy_schema(&schema)),
                validation: Some(to_policy_validation(&validation)),
            },
            roles,
            dynamic_layer_schemas,
        },
        runtime: RuntimeArtifact {
            app_name: spec.name.clone(),
            app_version: spec.version.clone(),
            ui_apps,
            app_layer_names,
        },
        schema,
        validation,
    }
}

fn to_policy_schema(schema: &SchemaArtifact) -> policy_model::SchemaArtifact {
    let entities = schema
        .entities
        .iter()
        .map(|(name, entity)| {
            let fields = entity
                .fields
                .iter()
                .map(|field| policy_model::FieldSchema {
                    name: field.name.clone(),
                    required: field.required,
                    immutable: field.immutable,
                })
                .collect();

            let transitions = entity
                .transitions
                .iter()
                .map(|transition| policy_model::TransitionSchema {
                    field: transition.field.clone(),
                    from: transition.from.clone(),
                    to: transition.to.clone(),
                    allowed_roles: transition.allowed_roles.clone(),
                    predicates: transition.predicates.iter().map(lower_predicate).collect(),
                })
                .collect();

            let entity_rules = entity
                .entity_rules
                .iter()
                .map(|rule| match rule {
                    EntityRuleSchema::OnUpdateSet { field, source } => {
                        let (source_name, literal) = match source {
                            EntityActionSource::FromClock => ("from_clock".to_string(), None),
                            EntityActionSource::FromPeer => ("from_peer".to_string(), None),
                            EntityActionSource::FromMode => ("from_mode".to_string(), None),
                            EntityActionSource::ToLiteral(value) => (
                                "to_literal".to_string(),
                                Some(ast_literal_to_policy_literal(value)),
                            ),
                        };
                        policy_model::EntityRuleSchema::OnUpdateSet {
                            field: field.clone(),
                            source: source_name,
                            literal,
                        }
                    }
                    EntityRuleSchema::OnDeleteReject => {
                        policy_model::EntityRuleSchema::OnDeleteReject
                    }
                })
                .collect();

            (
                name.clone(),
                policy_model::EntitySchema {
                    fields,
                    transitions,
                    entity_rules,
                },
            )
        })
        .collect();

    policy_model::SchemaArtifact {
        entities,
        layer_bindings: schema.layer_bindings.clone(),
    }
}

fn to_policy_validation(validation: &ValidationArtifact) -> policy_model::ValidationArtifact {
    policy_model::ValidationArtifact {
        lua_validators: validation.lua_validators.clone(),
        broadcasts: validation
            .broadcasts
            .iter()
            .map(|(layer, rules)| {
                let mapped = rules
                    .iter()
                    .map(|rule| policy_model::BroadcastPolicy {
                        target: match &rule.target {
                            BroadcastTarget::All => policy_model::BroadcastTarget::All,
                            BroadcastTarget::Granted => policy_model::BroadcastTarget::Granted,
                            BroadcastTarget::ToRoles(roles) => {
                                policy_model::BroadcastTarget::ToRoles(roles.clone())
                            }
                        },
                        debounce_ms: rule.debounce_ms,
                    })
                    .collect();
                (layer.clone(), mapped)
            })
            .collect(),
        orderings: validation
            .orderings
            .iter()
            .map(|(layer, order)| {
                (
                    layer.clone(),
                    policy_model::OrderPolicy {
                        field: order.field.clone(),
                        direction: match order.direction {
                            SortDirection::Ascending => policy_model::SortDirection::Ascending,
                            SortDirection::Descending => policy_model::SortDirection::Descending,
                        },
                    },
                )
            })
            .collect(),
        dynamic_layers: validation
            .dynamic_layers
            .iter()
            .map(|(layer, policy)| {
                (
                    layer.clone(),
                    policy_model::DynamicLayerPolicy {
                        create_allow: policy.create_allow.clone(),
                        discover: policy.discover.as_ref().map(|mode| match mode {
                            DiscoverMode::Sync => policy_model::DiscoverMode::Sync,
                            DiscoverMode::Grant => policy_model::DiscoverMode::Grant,
                        }),
                    },
                )
            })
            .collect(),
    }
}

fn lower_schema_artifact(
    spec: &AppSpec,
    layer_paths_by_name: &HashMap<&str, &str>,
) -> SchemaArtifact {
    let mut entities = HashMap::new();
    for sthithi in &spec.sthithis {
        let fields = sthithi
            .fields
            .iter()
            .map(|field| FieldSchema {
                name: field.name.clone(),
                required: field.required,
                immutable: field.immutable,
                default: field.default.as_ref().map(lower_default),
                source: field.source.as_ref().map(lower_field_source),
            })
            .collect();

        let transitions = sthithi
            .transitions
            .iter()
            .flat_map(|transition| {
                transition.rules.iter().map(|rule| TransitionSchema {
                    field: transition.field.clone(),
                    from: rule.from_state.clone(),
                    to: rule.to_state.clone(),
                    allowed_roles: rule.allowed_roles.clone(),
                    predicates: rule
                        .predicate
                        .as_ref()
                        .map(|p| p.predicates.clone())
                        .unwrap_or_default(),
                })
            })
            .collect();

        let entity_rules = sthithi
            .entity_rules
            .iter()
            .map(|rule| match rule {
                EntityRule::OnUpdateSet { field, source } => EntityRuleSchema::OnUpdateSet {
                    field: field.clone(),
                    source: source.clone(),
                },
                EntityRule::OnDeleteReject => EntityRuleSchema::OnDeleteReject,
            })
            .collect();

        entities.insert(
            sthithi.name.clone(),
            EntitySchema {
                fields,
                transitions,
                entity_rules,
            },
        );
    }

    let mut layer_bindings = HashMap::new();
    for layer in &spec.layers {
        if let (Some(entity), Some(path)) = (
            &layer.entity_binding,
            layer_paths_by_name.get(layer.name.as_str()),
        ) {
            layer_bindings.insert((*path).to_string(), entity.clone());
        }
    }

    SchemaArtifact {
        entities,
        layer_bindings,
    }
}

fn lower_validation_artifact(
    spec: &AppSpec,
    layer_paths_by_name: &HashMap<&str, &str>,
) -> ValidationArtifact {
    let mut lua_validators = HashMap::new();
    for validate in &spec.validates {
        if let Some(path) = layer_paths_by_name.get(validate.layer.as_str()) {
            lua_validators.insert((*path).to_string(), validate.handler.clone());
        }
    }

    let derivations = spec
        .derives
        .iter()
        .map(|derive| DerivationDef {
            source_layer: layer_paths_by_name
                .get(derive.source.as_str())
                .map(|path| (*path).to_string())
                .unwrap_or_else(|| derive.source.clone()),
            target_layer: derive.target.clone(),
            handler: derive.using.clone(),
        })
        .collect();

    let relays = spec
        .relays
        .iter()
        .filter_map(|relay| {
            layer_paths_by_name
                .get(relay.layer.as_str())
                .map(|path| RelayDef {
                    layer: (*path).to_string(),
                    event: relay.event.clone(),
                    handler: relay.handler.clone(),
                })
        })
        .collect();

    let mut broadcasts = HashMap::new();
    let mut orderings = HashMap::new();
    let mut dynamic_layers = HashMap::new();

    for layer in &spec.layers {
        let Some(path) = layer_paths_by_name.get(layer.name.as_str()) else {
            continue;
        };
        let layer_path = (*path).to_string();

        if !layer.broadcasts.is_empty() {
            let policies = layer
                .broadcasts
                .iter()
                .map(|broadcast| BroadcastPolicy {
                    target: broadcast.target.clone(),
                    debounce_ms: broadcast.debounce,
                })
                .collect();
            broadcasts.insert(layer_path.clone(), policies);
        }

        if let Some(order) = &layer.order {
            orderings.insert(
                layer_path.clone(),
                OrderPolicy {
                    field: order.field.clone(),
                    direction: order.direction.clone(),
                },
            );
        }

        if let Some(dynamic_by) = &layer.dynamic_by {
            dynamic_layers.insert(
                layer_path,
                DynamicLayerPolicy {
                    dynamic_by: dynamic_by.clone(),
                    create_allow: layer.create_allow.clone().unwrap_or_default(),
                    discover: layer.discover.clone(),
                    retain_days: layer.retain,
                    shard_resolution: match &layer.time {
                        TimeModel::TimeSharded { resolution, .. } => Some(resolution.clone()),
                        TimeModel::Unsharded => None,
                    },
                },
            );
        }
    }

    ValidationArtifact {
        lua_validators,
        derivations,
        relays,
        broadcasts,
        orderings,
        dynamic_layers,
    }
}

fn lower_permit_policy_rules(
    spec: &AppSpec,
    layer_paths_by_name: &HashMap<&str, &str>,
) -> Vec<PolicyRule> {
    let mut rules = Vec::new();

    for permit in &spec.permits {
        let subjects: Vec<Subject> = permit
            .roles
            .iter()
            .map(|role| Subject::Role { name: role.clone() })
            .collect();

        for stmt in &permit.statements {
            if let PermitStmt::Allow(allow) = stmt {
                if let Some(layer_path) = layer_paths_by_name.get(allow.layer.as_str()) {
                    rules.push(PolicyRule {
                        effect: PolicyEffect::Allow,
                        subjects: subjects.clone(),
                        actions: allow.actions.iter().map(format_action).collect(),
                        resource: ResourceSelector::Layer {
                            name: (*layer_path).to_string(),
                        },
                        condition: lower_predicate_expr(allow.predicate.as_ref()),
                    });
                }
            }
        }
    }

    rules
}

fn lower_delegation_rules(
    spec: &AppSpec,
    layer_paths_by_name: &HashMap<&str, &str>,
) -> Vec<DelegationRule> {
    let mut rules = Vec::new();

    for permit in &spec.permits {
        let allow_specs: Vec<(Vec<PolicyAction>, String)> = permit
            .statements
            .iter()
            .filter_map(|stmt| match stmt {
                PermitStmt::Allow(allow) => {
                    layer_paths_by_name.get(allow.layer.as_str()).map(|path| {
                        (
                            allow.actions.iter().map(format_action).collect(),
                            (*path).to_string(),
                        )
                    })
                }
                _ => None,
            })
            .collect();

        for stmt in &permit.statements {
            if let PermitStmt::Issue(issue) = stmt {
                for from_role in &permit.roles {
                    for to_role in &issue.roles {
                        for (actions, layer_path) in &allow_specs {
                            rules.push(DelegationRule {
                                from: Subject::Role {
                                    name: from_role.clone(),
                                },
                                to: Subject::Role {
                                    name: to_role.clone(),
                                },
                                actions: actions.clone(),
                                resource: ResourceSelector::Layer {
                                    name: layer_path.clone(),
                                },
                                max_depth: 3,
                                no_escalation: true,
                                condition: None,
                            });
                        }
                    }
                }
            }
        }
    }

    rules
}

fn format_resolution(resolution: &crate::ast::Resolution) -> &'static str {
    match resolution {
        crate::ast::Resolution::Minute => "minute",
        crate::ast::Resolution::Hour => "hour",
        crate::ast::Resolution::Day => "day",
        crate::ast::Resolution::Week => "week",
        crate::ast::Resolution::Month => "month",
    }
}

fn format_action(action: &Action) -> PolicyAction {
    match action {
        Action::Create => PolicyAction::Create,
        Action::Read => PolicyAction::Read,
        Action::Write => PolicyAction::Write,
        Action::Sync => PolicyAction::Sync,
        Action::Grant => PolicyAction::Grant,
        Action::Revoke => PolicyAction::Revoke,
    }
}

fn lower_predicate_expr(predicate: Option<&PredicateExpr>) -> Option<policy_model::ConditionExpr> {
    let predicate = predicate?;
    let mut preds = predicate
        .predicates
        .iter()
        .map(lower_predicate)
        .collect::<Vec<_>>();

    if preds.is_empty() {
        return None;
    }

    if preds.len() == 1 {
        return Some(policy_model::ConditionExpr::Pred {
            pred: preds.remove(0),
        });
    }

    Some(policy_model::ConditionExpr::And {
        args: preds
            .into_iter()
            .map(|pred| policy_model::ConditionExpr::Pred { pred })
            .collect(),
    })
}

fn lower_predicate(predicate: &Predicate) -> policy_model::Predicate {
    match predicate {
        Predicate::Is { field, value } => policy_model::Predicate::Eq {
            left: Value::Ref(Ref::TargetField {
                field: field.clone(),
            }),
            right: lower_value_ref(value),
        },
        Predicate::IsNot { field, value } => policy_model::Predicate::Ne {
            left: Value::Ref(Ref::TargetField {
                field: field.clone(),
            }),
            right: lower_value_ref(value),
        },
        Predicate::In { field, values } => policy_model::Predicate::In {
            item: Value::Ref(Ref::TargetField {
                field: field.clone(),
            }),
            set: values.iter().map(lower_value_ref).collect(),
        },
        Predicate::Above { field, value } => policy_model::Predicate::Gt {
            left: Value::Ref(Ref::TargetField {
                field: field.clone(),
            }),
            right: lower_value_ref(value),
        },
        Predicate::Below { field, value } => policy_model::Predicate::Lt {
            left: Value::Ref(Ref::TargetField {
                field: field.clone(),
            }),
            right: lower_value_ref(value),
        },
    }
}

fn lower_value_ref(value: &ValueRef) -> Value {
    match value {
        ValueRef::SelfRef => Value::Ref(Ref::ActorDid),
        ValueRef::Peer => Value::Ref(Ref::ActorDid),
        ValueRef::User => Value::Str("user".to_string()),
        ValueRef::Node => Value::Str("node".to_string()),
        ValueRef::StringLit(v) => Value::Str(v.clone()),
        ValueRef::IntLit(v) => Value::Num(i64::from(*v)),
        ValueRef::BoolTrue => Value::Bool(true),
        ValueRef::BoolFalse => Value::Bool(false),
    }
}

fn lower_default(value: &crate::ast::Literal) -> DefaultValue {
    match value {
        crate::ast::Literal::String(v) => DefaultValue::String(v.clone()),
        crate::ast::Literal::Int(v) => DefaultValue::Int(*v),
        crate::ast::Literal::Bool(v) => DefaultValue::Bool(*v),
        crate::ast::Literal::Null => DefaultValue::Null,
    }
}

fn lower_field_source(source: &FieldSource) -> FieldSourceSchema {
    match source {
        FieldSource::Clock => FieldSourceSchema::Clock,
        FieldSource::Peer => FieldSourceSchema::PeerDid,
        FieldSource::SelfRef => FieldSourceSchema::PeerRole,
        FieldSource::Mode => FieldSourceSchema::Mode,
    }
}

fn ast_literal_to_policy_literal(literal: &crate::ast::Literal) -> policy_model::LiteralValue {
    match literal {
        crate::ast::Literal::String(v) => policy_model::LiteralValue::String(v.clone()),
        crate::ast::Literal::Int(v) => policy_model::LiteralValue::Int(i64::from(*v)),
        crate::ast::Literal::Bool(v) => policy_model::LiteralValue::Bool(*v),
        crate::ast::Literal::Null => policy_model::LiteralValue::Null,
    }
}
