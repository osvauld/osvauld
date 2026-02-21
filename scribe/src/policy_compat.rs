use std::collections::{HashMap, HashSet};

use gurkha::policy::{Action, DecisionContext, ResourceContext, ResourceSelector, TokenDecision};

pub fn role(permit: &gurkha::PolicyPermit) -> Option<String> {
    permit
        .get_fact("role")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            permit
                .get_fact("relationship")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
}

pub fn relationship(permit: &gurkha::PolicyPermit) -> Option<String> {
    permit
        .get_fact("relationship")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| role(permit))
}

pub fn actor_roles(permit: &gurkha::PolicyPermit) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    if let Some(r) = role(permit) {
        if seen.insert(r.clone()) {
            out.push(r);
        }
    }

    for r in &permit.facts().roles {
        if seen.insert(r.clone()) {
            out.push(r.clone());
        }
    }

    out
}

fn can_layer_action(
    permit: &gurkha::PolicyPermit,
    layer_name: &str,
    actor_did: &str,
    action: Action,
) -> bool {
    // Path 1: policy_rules (static layers declared in app.osv)
    // Only return on Allow — Deny could mean "no matching rule", so fall through.
    let roles = actor_roles(permit);
    let resource = ResourceContext {
        layer: Some(layer_name.to_string()),
        path_vars: HashMap::new(),
    };
    let ctx = DecisionContext {
        actor_did: actor_did.to_string(),
        actor_roles: roles,
        audience_did: Some(permit.audience().to_string()),
        action,
        resource,
        target_fields: HashMap::new(),
        time_period: None,
    };
    if gurkha::can_access(permit.facts(), &ctx) == TokenDecision::Allow {
        return true;
    }

    // Path 2: dynamic layer schemas (pattern-based layers from app.osv)
    let schemas = dynamic_layer_schemas(permit);
    if let Some(parsed) = gurkha::parse_dynamic_layer(&schemas, layer_name) {
        if let Some(schema) = schemas.get(&parsed.schema_key) {
            return match action {
                Action::Sync | Action::Read => schema.permissions.sync,
                Action::Write => schema.permissions.write,
                _ => false,
            };
        }
    }

    // Path 3: legacy "layers" fact (backward compat during migration)
    if let Some(layers_obj) = permit
        .get_fact("layers")
        .and_then(serde_json::Value::as_object)
    {
        let cfg = layers_obj.get(layer_name).or_else(|| {
            let suffix = format!("/{}", layer_name);
            layers_obj
                .iter()
                .find(|(k, _)| k.ends_with(&suffix))
                .map(|(_, v)| v)
        });

        if let Some(value) = cfg {
            if let Ok(config) = serde_json::from_value::<gurkha::LayerConfig>(value.clone()) {
                return match action {
                    Action::Sync | Action::Read => config.sync,
                    Action::Write => config.write,
                    _ => false,
                };
            }
        }
    }

    false
}

pub fn can_read_layer(permit: &gurkha::PolicyPermit, layer_name: &str, actor_did: &str) -> bool {
    can_layer_action(permit, layer_name, actor_did, Action::Read)
}

pub fn can_write_layer(permit: &gurkha::PolicyPermit, layer_name: &str, actor_did: &str) -> bool {
    can_layer_action(permit, layer_name, actor_did, Action::Write)
}

pub fn should_sync_layer(permit: &gurkha::PolicyPermit, layer_name: &str, actor_did: &str) -> bool {
    can_layer_action(permit, layer_name, actor_did, Action::Sync)
}

pub fn no_incoming_updates(permit: &gurkha::PolicyPermit) -> Vec<String> {
    permit
        .get_fact("sync")
        .and_then(serde_json::Value::as_object)
        .and_then(|sync| sync.get("no_incoming_updates"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn is_visible(permit: &gurkha::PolicyPermit) -> bool {
    permit
        .get_fact("presence")
        .and_then(serde_json::Value::as_object)
        .and_then(|p| p.get("visible"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub fn can_see_others(permit: &gurkha::PolicyPermit) -> bool {
    permit
        .get_fact("presence")
        .and_then(serde_json::Value::as_object)
        .and_then(|p| p.get("can_see_others"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

pub fn display_name(permit: &gurkha::PolicyPermit) -> Option<String> {
    permit
        .get_fact("presence")
        .and_then(serde_json::Value::as_object)
        .and_then(|p| p.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

pub fn can_manage_layer_access(permit: &gurkha::PolicyPermit) -> bool {
    permit
        .get_fact("peer_capabilities")
        .and_then(serde_json::Value::as_object)
        .and_then(|caps| caps.get("manage_layer_access"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub fn ephemeral_funcs(permit: &gurkha::PolicyPermit) -> Vec<String> {
    permit
        .get_fact("ephemeral_funcs")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn can_send_ephemeral(permit: &gurkha::PolicyPermit, func: &str) -> bool {
    let funcs = ephemeral_funcs(permit);
    funcs.is_empty() || funcs.iter().any(|f| f == func)
}

pub fn static_layers(permit: &gurkha::PolicyPermit, page_id: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    if let Some(layers_obj) = permit
        .get_fact("layers")
        .and_then(serde_json::Value::as_object)
    {
        for layer_name in layers_obj.keys() {
            let bare = layer_name
                .strip_prefix(&format!("{}/", page_id))
                .unwrap_or(layer_name)
                .to_string();
            if seen.insert(bare.clone()) {
                out.push(bare);
            }
        }
    }

    for rule in &permit.facts().policy_rules {
        if rule.effect != gurkha::PolicyEffect::Allow {
            continue;
        }
        match &rule.resource {
            ResourceSelector::Layer { name } => {
                // Skip dynamic schema patterns (contain placeholders like {id})
                if !name.contains('{') && seen.insert(name.clone()) {
                    out.push(name.clone());
                }
            }
            ResourceSelector::LayerPath { layer, .. } => {
                if !layer.contains('{') && seen.insert(layer.clone()) {
                    out.push(layer.clone());
                }
            }
            ResourceSelector::Any => {}
        }
    }

    out
}

fn parse_namespace(value: &str) -> gurkha::LayerNamespace {
    match value {
        "shared" => gurkha::LayerNamespace::Shared,
        _ => gurkha::LayerNamespace::Creator,
    }
}

fn parse_grant(value: &str) -> gurkha::GrantType {
    match value {
        "explicit" => gurkha::GrantType::Explicit,
        _ => gurkha::GrantType::Open,
    }
}

fn parse_storage_strategy(value: &str) -> gurkha::StorageStrategy {
    match value {
        "time_sharded" => gurkha::StorageStrategy::TimeSharded,
        _ => gurkha::StorageStrategy::SingleDoc,
    }
}

fn parse_resolution(value: Option<&str>) -> Option<gurkha::Resolution> {
    match value {
        Some("minute") => Some(gurkha::Resolution::Minute),
        Some("hour") => Some(gurkha::Resolution::Hour),
        Some("day") => Some(gurkha::Resolution::Day),
        Some("week") => Some(gurkha::Resolution::Week),
        Some("month") => Some(gurkha::Resolution::Month),
        _ => None,
    }
}

pub fn dynamic_layer_schemas(
    permit: &gurkha::PolicyPermit,
) -> HashMap<String, gurkha::DynamicLayerSchema> {
    let mut out: HashMap<String, gurkha::DynamicLayerSchema> = HashMap::new();

    for schema in &permit.facts().dynamic_layer_schemas {
        let key = if schema.path.is_empty() {
            schema.name.clone()
        } else {
            schema.path.clone()
        };
        out.insert(
            key,
            gurkha::DynamicLayerSchema {
                layer_type: "map".to_string(),
                grant: parse_grant(&schema.grant),
                permissions: gurkha::LayerConfig {
                    sync: true,
                    write: true,
                    layer_type: None,
                },
                namespace: parse_namespace(&schema.namespace),
                storage_strategy: parse_storage_strategy(&schema.storage_strategy),
                resolution: parse_resolution(schema.resolution.as_deref()),
            },
        );
    }

    if let Some(v) = permit.get_fact("dynamic_layer_schemas") {
        if let Ok(parsed) =
            serde_json::from_value::<HashMap<String, gurkha::DynamicLayerSchema>>(v.clone())
        {
            for (k, schema) in parsed {
                out.entry(k).or_insert(schema);
            }
        }
    }

    out
}

pub fn parse_dynamic_layer(
    permit: &gurkha::PolicyPermit,
    layer_name: &str,
) -> Option<gurkha::DynamicLayerRef> {
    let schemas = dynamic_layer_schemas(permit);
    gurkha::parse_dynamic_layer(&schemas, layer_name)
}

pub fn has_dynamic_layer_schemas(permit: &gurkha::PolicyPermit) -> bool {
    !dynamic_layer_schemas(permit).is_empty()
}

pub fn layer_config_for(
    permit: &gurkha::PolicyPermit,
    full_layer_name: &str,
    bare_layer_name: &str,
) -> Option<gurkha::LayerConfig> {
    let layers = permit.get_fact("layers")?.as_object()?;
    let cfg = layers
        .get(full_layer_name)
        .or_else(|| layers.get(bare_layer_name))?
        .clone();
    serde_json::from_value(cfg).ok()
}

pub fn authorized_peers(permit: &gurkha::PolicyPermit) -> Option<Vec<String>> {
    match permit.get_fact("authorized_peers") {
        None => None,
        Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Array(items)) => Some(
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect(),
        ),
        Some(_) => Some(Vec::new()),
    }
}

pub fn is_peer_authorized(permit: &gurkha::PolicyPermit, peer_did: &str) -> bool {
    match authorized_peers(permit) {
        None => true,
        Some(peers) => peers.iter().any(|p| p == peer_did),
    }
}

pub fn matches_dynamic_schema_for_write(
    permit: &gurkha::PolicyPermit,
    layer_name: &str,
    page_id: &str,
) -> bool {
    let bare = layer_name
        .strip_prefix(&format!("{}/", page_id))
        .unwrap_or(layer_name);
    let schemas = dynamic_layer_schemas(permit);
    let Some(parsed) = gurkha::parse_dynamic_layer(&schemas, bare) else {
        return false;
    };

    schemas
        .get(&parsed.schema_key)
        .map(|schema| schema.permissions.write)
        .unwrap_or(false)
}
