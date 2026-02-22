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
    validate_sthithi_decls(spec, &mut errors);
    validate_entity_bindings(spec, &mut errors);
    validate_transition_refs(spec, &mut errors);
    validate_relay_refs(spec, &mut errors);
    validate_validate_refs(spec, &mut errors);
    validate_permit_refs(spec, &mut errors);
    validate_dynamic_layer_invariants(spec, &mut errors);

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

    let mut seen_sthithis = HashSet::new();
    for sthithi in &spec.sthithis {
        if !seen_sthithis.insert(sthithi.name.clone()) {
            errors.push(Diagnostic::new(
                "E2004",
                format!("duplicate sthithi '{}'", sthithi.name),
                None,
            ));
        }
    }

    let mut seen_permits = HashSet::new();
    for permit in &spec.permits {
        if !seen_permits.insert(permit.name.clone()) {
            errors.push(Diagnostic::new(
                "E2005",
                format!("duplicate permit '{}'", permit.name),
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

        // Validate create_allow role refs
        if let Some(roles) = &layer.create_allow {
            for role in roles {
                if !role_names.contains(role) {
                    errors.push(Diagnostic::new(
                        "E2102",
                        format!(
                            "layer '{}' references unknown role '{}' in create allow",
                            layer.name, role
                        ),
                        None,
                    ));
                }
            }
        }

        // Validate broadcast role refs
        for broadcast in &layer.broadcasts {
            if let BroadcastTarget::ToRoles(roles) = &broadcast.target {
                for role in roles {
                    if !role_names.contains(role) {
                        errors.push(Diagnostic::new(
                            "E2102",
                            format!(
                                "layer '{}' references unknown role '{}' in broadcast",
                                layer.name, role
                            ),
                            None,
                        ));
                    }
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

        // v2: `shard by resolution` is sufficient. The runtime handles shard
        // segmentation — {period} in the path is NOT required.
        // Only error if {period} appears without a shard declaration.
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

// ── v2 Semantic Validation ──────────────────────────────────────────────────

fn validate_sthithi_decls(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    let role_names: HashSet<String> = spec.roles.iter().map(|r| r.name.clone()).collect();

    for sthithi in &spec.sthithis {
        // Check field name uniqueness within a sthithi
        let mut seen_fields = HashSet::new();
        for field in &sthithi.fields {
            if !seen_fields.insert(field.name.clone()) {
                errors.push(Diagnostic::new(
                    "E2501",
                    format!(
                        "sthithi '{}' has duplicate field '{}'",
                        sthithi.name, field.name
                    ),
                    None,
                ));
            }
        }

        let field_names: HashSet<String> = sthithi.fields.iter().map(|f| f.name.clone()).collect();

        // Validate transition fields exist in the sthithi
        for transition in &sthithi.transitions {
            if !field_names.contains(&transition.field) {
                errors.push(Diagnostic::new(
                    "E2502",
                    format!(
                        "sthithi '{}' transitions references unknown field '{}'",
                        sthithi.name, transition.field
                    ),
                    None,
                ));
            }

            // Validate roles in transition rules
            for rule in &transition.rules {
                for role in &rule.allowed_roles {
                    if !role_names.contains(role) {
                        errors.push(Diagnostic::new(
                            "E2503",
                            format!(
                                "sthithi '{}' transition rule references unknown role '{}'",
                                sthithi.name, role
                            ),
                            None,
                        ));
                    }
                }
            }
        }

        // Validate entity rule field references
        for rule in &sthithi.entity_rules {
            if let EntityRule::OnUpdateSet { field, .. } = rule {
                if !field_names.contains(field) {
                    errors.push(Diagnostic::new(
                        "E2504",
                        format!(
                            "sthithi '{}' entity rule references unknown field '{}'",
                            sthithi.name, field
                        ),
                        None,
                    ));
                }
            }
        }
    }
}

fn validate_entity_bindings(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    let sthithi_names: HashSet<String> = spec.sthithis.iter().map(|s| s.name.clone()).collect();

    for layer in &spec.layers {
        if let Some(ref entity) = layer.entity_binding {
            if !sthithi_names.contains(entity) {
                errors.push(Diagnostic::new(
                    "E2505",
                    format!(
                        "layer '{}' binds to unknown sthithi '{}'",
                        layer.name, entity
                    ),
                    None,
                ));
            }
        }
    }
}

fn validate_transition_refs(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    // Build a map of sthithi name -> field names for validating dynamic_by
    let sthithi_fields: HashMap<String, HashSet<String>> = spec
        .sthithis
        .iter()
        .map(|s| {
            let fields: HashSet<String> = s.fields.iter().map(|f| f.name.clone()).collect();
            (s.name.clone(), fields)
        })
        .collect();

    for layer in &spec.layers {
        // Validate dynamic_by: fields must either exist in the bound entity
        // OR appear as path placeholders. In v2, `dynamic by id` is a path
        // parameter — it creates layer instances per unique value, not necessarily
        // a field in the entity schema. We only warn if the field is BOTH
        // absent from the entity AND absent from the path placeholders.
        if let (Some(ref dynamic_fields), Some(ref entity)) =
            (&layer.dynamic_by, &layer.entity_binding)
        {
            if let Some(entity_fields) = sthithi_fields.get(entity) {
                for field in dynamic_fields {
                    let in_entity = entity_fields.contains(field);
                    let in_path = layer.path.contains(&format!("{{{}}}", field));
                    if !in_entity && !in_path {
                        errors.push(Diagnostic::new(
                            "E2506",
                            format!(
                                "layer '{}' dynamic by field '{}' not found in sthithi '{}' or path",
                                layer.name, field, entity
                            ),
                            None,
                        ));
                    }
                }
            }
        }

        // Validate order by field exists in bound entity
        if let (Some(ref order), Some(ref entity)) = (&layer.order, &layer.entity_binding) {
            if let Some(entity_fields) = sthithi_fields.get(entity) {
                if !entity_fields.contains(&order.field) {
                    errors.push(Diagnostic::new(
                        "E2507",
                        format!(
                            "layer '{}' order by field '{}' not found in sthithi '{}'",
                            layer.name, order.field, entity
                        ),
                        None,
                    ));
                }
            }
        }
    }
}

fn validate_relay_refs(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    let layer_names: HashSet<String> = spec.layers.iter().map(|l| l.name.clone()).collect();

    for relay in &spec.relays {
        if !layer_names.contains(&relay.layer) {
            errors.push(Diagnostic::new(
                "E2601",
                format!("relay references unknown layer '{}'", relay.layer),
                None,
            ));
        }
    }
}

fn validate_validate_refs(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    let layer_names: HashSet<String> = spec.layers.iter().map(|l| l.name.clone()).collect();

    for validate in &spec.validates {
        if !layer_names.contains(&validate.layer) {
            errors.push(Diagnostic::new(
                "E2602",
                format!("validate references unknown layer '{}'", validate.layer),
                None,
            ));
        }
    }
}

fn validate_permit_refs(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    let role_names: HashSet<String> = spec.roles.iter().map(|r| r.name.clone()).collect();
    let layer_names: HashSet<String> = spec.layers.iter().map(|l| l.name.clone()).collect();

    for permit in &spec.permits {
        // Validate permit's for-roles exist
        for role in &permit.roles {
            if !role_names.contains(role) {
                errors.push(Diagnostic::new(
                    "E2603",
                    format!(
                        "permit '{}' references unknown role '{}' in for clause",
                        permit.name, role
                    ),
                    None,
                ));
            }
        }

        for stmt in &permit.statements {
            match stmt {
                PermitStmt::Allow(allow) => {
                    // Validate layer reference
                    if !layer_names.contains(&allow.layer) {
                        errors.push(Diagnostic::new(
                            "E2604",
                            format!(
                                "permit '{}' allow references unknown layer '{}'",
                                permit.name, allow.layer
                            ),
                            None,
                        ));
                    }
                }
                PermitStmt::Issue(issue) => {
                    // Validate issue-for role references
                    for role in &issue.roles {
                        if !role_names.contains(role) {
                            errors.push(Diagnostic::new(
                                "E2605",
                                format!(
                                    "permit '{}' issue references unknown role '{}'",
                                    permit.name, role
                                ),
                                None,
                            ));
                        }
                    }
                }
            }
        }
    }
}

fn validate_dynamic_layer_invariants(spec: &AppSpec, errors: &mut Vec<Diagnostic>) {
    for layer in &spec.layers {
        // discover without dynamic is an error
        if layer.discover.is_some() && layer.dynamic_by.is_none() {
            errors.push(Diagnostic::new(
                "E2701",
                format!("layer '{}' uses discover but is not dynamic", layer.name),
                None,
            ));
        }

        // create_allow without dynamic is an error
        if layer.create_allow.is_some() && layer.dynamic_by.is_none() {
            errors.push(Diagnostic::new(
                "E2702",
                format!(
                    "layer '{}' uses create allow but is not dynamic",
                    layer.name
                ),
                None,
            ));
        }

        // retain without sharding is now allowed via v2 (retain is days, not tied to shard only)
        // but retain on non-sharded non-dynamic layers is suspicious — keep as warning-level for now
    }
}
