use serde::{Deserialize, Serialize};

use crate::ast::{Action, AppSpec, GrantMode, LayerNamespace, ScopeExpr, TimeModel};
use policy_model::{
    Action as PolicyAction, DynamicLayerSchema, PolicyEffect, PolicyFacts, PolicyRule,
    ResourceSelector, Subject,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledArtifacts {
    pub permit: PermitArtifact,
    pub runtime: RuntimeArtifact,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiAppArtifact {
    pub name: String,
    pub entry: String,
    pub allow_roles: Vec<String>,
}

pub fn compile(spec: &AppSpec) -> CompiledArtifacts {
    let roles = spec.roles.iter().map(|r| r.name.clone()).collect();
    let dynamic_layer_schemas: Vec<DynamicLayerSchema> = spec
        .layers
        .iter()
        .map(|layer| DynamicLayerSchema {
            name: layer.name.clone(),
            path: layer.path.clone(),
            namespace: match layer.namespace {
                LayerNamespace::Shared => "shared".to_string(),
                LayerNamespace::Creator => "creator".to_string(),
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

    let policy_rules = spec
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
                resource: resource_from_scope(&rule.scope, &layer.path),
                condition: None,
            })
        })
        .collect::<Vec<_>>();

    let ui_apps = spec
        .ui_apps
        .iter()
        .map(|app| UiAppArtifact {
            name: app.name.clone(),
            entry: app.entry.clone(),
            allow_roles: app.allowed_roles.clone(),
        })
        .collect();

    CompiledArtifacts {
        permit: PermitArtifact {
            osv_policy: PolicyFacts {
                roles: spec.roles.iter().map(|r| r.name.clone()).collect(),
                policy_rules,
                delegation_rules: Vec::new(),
                dynamic_layer_schemas: dynamic_layer_schemas.clone(),
            },
            roles,
            dynamic_layer_schemas,
        },
        runtime: RuntimeArtifact {
            app_name: spec.name.clone(),
            app_version: spec.version.clone(),
            ui_apps,
        },
    }
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

fn resource_from_scope(scope: &ScopeExpr, fallback_layer: &str) -> ResourceSelector {
    match scope {
        ScopeExpr::All => ResourceSelector::Layer {
            name: fallback_layer.to_string(),
        },
        ScopeExpr::LayerRef(name) => ResourceSelector::Layer { name: name.clone() },
    }
}
