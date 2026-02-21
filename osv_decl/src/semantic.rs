use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::Diagnostic;

pub fn check(spec: &AppSpec) -> Result<(), Vec<Diagnostic>> {
    let mut errors = Vec::new();

    validate_unique_names(spec, &mut errors);
    validate_role_references(spec, &mut errors);
    validate_role_cycles(spec, &mut errors);
    validate_layer_rules(spec, &mut errors);
    validate_derive_refs(spec, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_unique_names(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    let mut seen_roles = HashSet::new();
    for role in &spec.roles {
        if !seen_roles.insert(role.name.clone()) {
            errors.push(Diagnostic::new(
                "E2001",
                format!("duplicate role '{}'", role.name),
                None,
            ));
        }
    }

    let mut seen_layers = HashSet::new();
    for layer in &spec.layers {
        if !seen_layers.insert(layer.name.clone()) {
            errors.push(Diagnostic::new(
                "E2002",
                format!("duplicate layer '{}'", layer.name),
                None,
            ));
        }
    }

    let mut seen_apps = HashSet::new();
    for app in &spec.ui_apps {
        if !seen_apps.insert(app.name.clone()) {
            errors.push(Diagnostic::new(
                "E2003",
                format!("duplicate ui_app '{}'", app.name),
                None,
            ));
        }
    }
}

fn validate_role_references(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    let role_names: HashSet<String> = spec.roles.iter().map(|r| r.name.clone()).collect();

    for role in &spec.roles {
        if let Some(parent) = &role.inherits {
            if !role_names.contains(parent) {
                errors.push(Diagnostic::new(
                    "E2101",
                    format!("role '{}' inherits unknown role '{}'", role.name, parent),
                    None,
                ));
            }
        }
    }

    for layer in &spec.layers {
        for rule in &layer.access {
            for role in &rule.roles {
                if !role_names.contains(role) {
                    errors.push(Diagnostic::new(
                        "E2102",
                        format!(
                            "layer '{}' references unknown role '{}' in access rule",
                            layer.name, role
                        ),
                        None,
                    ));
                }
            }
        }
    }

    for app in &spec.ui_apps {
        for role in &app.allowed_roles {
            if !role_names.contains(role) {
                errors.push(Diagnostic::new(
                    "E2103",
                    format!("ui_app '{}' references unknown role '{}'", app.name, role),
                    None,
                ));
            }
        }
    }
}

fn validate_role_cycles(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    let parents: HashMap<String, String> = spec
        .roles
        .iter()
        .filter_map(|r| r.inherits.as_ref().map(|p| (r.name.clone(), p.clone())))
        .collect();

    for role in &spec.roles {
        let mut seen = HashSet::new();
        let mut cursor = role.name.as_str();
        while let Some(parent) = parents.get(cursor) {
            if !seen.insert(cursor.to_string()) {
                errors.push(Diagnostic::new(
                    "E2201",
                    format!("role inheritance cycle detected at '{}'", cursor),
                    None,
                ));
                break;
            }
            cursor = parent;
        }
    }
}

fn validate_layer_rules(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    for layer in &spec.layers {
        let has_period_placeholder = layer.path.contains("{period}");
        let is_sharded = matches!(layer.time, TimeModel::TimeSharded { .. });

        if is_sharded && !has_period_placeholder {
            errors.push(Diagnostic::new(
                "E2301",
                format!(
                    "layer '{}' is sharded but path '{}' is missing '{{period}}'",
                    layer.name, layer.path
                ),
                None,
            ));
        }

        if !is_sharded && has_period_placeholder {
            errors.push(Diagnostic::new(
                "E2302",
                format!(
                    "layer '{}' path '{}' contains '{{period}}' but no shard policy is declared",
                    layer.name, layer.path
                ),
                None,
            ));
        }

        let has_retention = layer
            .cache
            .iter()
            .any(|c| matches!(c, CachePolicy::RetentionDays(_)));
        if has_retention && !is_sharded {
            errors.push(Diagnostic::new(
                "E2303",
                format!(
                    "layer '{}' uses retention_days but is not time-sharded",
                    layer.name
                ),
                None,
            ));
        }

        if layer.grant == GrantMode::Explicit {
            let has_grant_or_revoke = layer.access.iter().any(|rule| {
                rule.actions
                    .iter()
                    .any(|a| matches!(a, Action::Grant | Action::Revoke))
            });
            if !has_grant_or_revoke {
                errors.push(Diagnostic::new(
                    "E2304",
                    format!(
                        "layer '{}' uses grant explicit but no access rule includes grant or revoke",
                        layer.name
                    ),
                    None,
                ));
            }
        }
    }
}

fn validate_derive_refs(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    let layer_names: HashSet<String> = spec.layers.iter().map(|l| l.name.clone()).collect();

    for derive in &spec.derives {
        if !layer_names.contains(&derive.source) {
            errors.push(Diagnostic::new(
                "E2401",
                format!(
                    "derive '{}' references unknown source layer '{}'",
                    derive.target, derive.source
                ),
                None,
            ));
        }

        let has_retention = derive
            .cache
            .iter()
            .any(|c| matches!(c, CachePolicy::RetentionDays(_)));
        if has_retention {
            errors.push(Diagnostic::new(
                "E2402",
                format!(
                    "derive '{}' cannot use retention_days in v1 (retention is layer-shard only)",
                    derive.target
                ),
                None,
            ));
        }
    }
}
