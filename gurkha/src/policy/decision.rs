use super::context::DecisionContext;
use super::types::{
    Action, ConditionExpr, DelegationRequest, PolicyEffect, PolicyFacts, Predicate, Ref,
    ResourceSelector, Subject, Value,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenDecision {
    Allow,
    Deny,
}

pub fn can_access(facts: &PolicyFacts, ctx: &DecisionContext) -> TokenDecision {
    let mut allow = false;
    for rule in &facts.policy_rules {
        if !rule.actions.contains(&ctx.action) {
            continue;
        }
        if !matches_resource(&rule.resource, ctx) {
            continue;
        }
        if !rule.subjects.iter().any(|s| matches_subject(s, ctx)) {
            continue;
        }
        if !matches_condition(rule.condition.as_ref(), ctx) {
            continue;
        }

        if rule.effect == PolicyEffect::Deny {
            return TokenDecision::Deny;
        }
        allow = true;
    }

    if allow {
        TokenDecision::Allow
    } else {
        TokenDecision::Deny
    }
}

pub fn can_delegate(
    facts: &PolicyFacts,
    ctx: &DecisionContext,
    req: &DelegationRequest,
) -> TokenDecision {
    for rule in &facts.delegation_rules {
        if !matches_subject(&rule.from, ctx) {
            continue;
        }
        let to_ctx = DecisionContext {
            actor_did: ctx.actor_did.clone(),
            actor_roles: vec![req.to_role.clone()],
            audience_did: ctx.audience_did.clone(),
            action: Action::Delegate,
            resource: ctx.resource.clone(),
            target_fields: ctx.target_fields.clone(),
            time_period: ctx.time_period.clone(),
        };
        if !matches_subject(&rule.to, &to_ctx) {
            continue;
        }
        if req.depth > rule.max_depth {
            continue;
        }
        if !req.actions.iter().all(|a| rule.actions.contains(a)) {
            continue;
        }
        if !resource_subset(&req.resource, &rule.resource) {
            continue;
        }
        if !matches_condition(rule.condition.as_ref(), ctx) {
            continue;
        }
        if rule.no_escalation {
            let can_all = req.actions.iter().all(|a| {
                let mut access_ctx = ctx.clone();
                access_ctx.action = *a;
                can_access(facts, &access_ctx) == TokenDecision::Allow
            });
            if !can_all {
                continue;
            }
        }
        return TokenDecision::Allow;
    }

    TokenDecision::Deny
}

fn resource_subset(requested: &ResourceSelector, granted: &ResourceSelector) -> bool {
    match (requested, granted) {
        (_, ResourceSelector::Any) => true,
        (ResourceSelector::Any, _) => false,
        (ResourceSelector::Layer { name: rn }, ResourceSelector::Layer { name: gn }) => rn == gn,
        (
            ResourceSelector::LayerPath {
                layer: rl,
                bindings: rb,
            },
            ResourceSelector::LayerPath {
                layer: gl,
                bindings: gb,
            },
        ) => {
            rl == gl
                && gb
                    .iter()
                    .all(|(k, v)| rb.get(k).map(|x| x == v).unwrap_or(false))
        }
        (ResourceSelector::LayerPath { layer, .. }, ResourceSelector::Layer { name }) => {
            layer == name
        }
        _ => false,
    }
}

fn matches_resource(resource: &ResourceSelector, ctx: &DecisionContext) -> bool {
    match resource {
        ResourceSelector::Any => true,
        ResourceSelector::Layer { name } => ctx.resource.layer.as_ref() == Some(name),
        ResourceSelector::LayerPath { layer, bindings } => {
            if ctx.resource.layer.as_ref() != Some(layer) {
                return false;
            }
            bindings
                .iter()
                .all(|(k, v)| ctx.resource.path_vars.get(k) == Some(v))
        }
    }
}

fn matches_subject(subject: &Subject, ctx: &DecisionContext) -> bool {
    match subject {
        Subject::Role { name } => ctx.actor_roles.iter().any(|r| r == name),
        Subject::SelfActor => ctx
            .audience_did
            .as_ref()
            .map(|a| a == &ctx.actor_did)
            .unwrap_or(false),
        Subject::Owner => ctx.actor_roles.iter().any(|r| r == "owner"),
        Subject::Creator => ctx.actor_roles.iter().any(|r| r == "creator"),
    }
}

fn matches_condition(condition: Option<&ConditionExpr>, ctx: &DecisionContext) -> bool {
    match condition {
        None => true,
        Some(expr) => eval_expr(expr, ctx).unwrap_or(false),
    }
}

fn eval_expr(expr: &ConditionExpr, ctx: &DecisionContext) -> Option<bool> {
    match expr {
        ConditionExpr::Pred { pred } => eval_predicate(pred, ctx),
        ConditionExpr::And { args } => {
            let mut value = true;
            for arg in args {
                value = value && eval_expr(arg, ctx)?;
            }
            Some(value)
        }
        ConditionExpr::Or { args } => {
            let mut value = false;
            for arg in args {
                value = value || eval_expr(arg, ctx)?;
            }
            Some(value)
        }
        ConditionExpr::Not { arg } => Some(!eval_expr(arg, ctx)?),
    }
}

fn eval_predicate(pred: &Predicate, ctx: &DecisionContext) -> Option<bool> {
    match pred {
        Predicate::Eq { left, right } => {
            Some(resolve_value(left, ctx)? == resolve_value(right, ctx)?)
        }
        Predicate::Ne { left, right } => {
            Some(resolve_value(left, ctx)? != resolve_value(right, ctx)?)
        }
        Predicate::Lt { left, right } => cmp_num(left, right, ctx, |a, b| a < b),
        Predicate::Le { left, right } => cmp_num(left, right, ctx, |a, b| a <= b),
        Predicate::Gt { left, right } => cmp_num(left, right, ctx, |a, b| a > b),
        Predicate::Ge { left, right } => cmp_num(left, right, ctx, |a, b| a >= b),
        Predicate::In { item, set } => {
            let item_val = resolve_value(item, ctx)?;
            Some(
                set.iter()
                    .filter_map(|v| resolve_value(v, ctx))
                    .any(|v| v == item_val),
            )
        }
        Predicate::Matches { value, pattern } => {
            let v = resolve_value(value, ctx)?;
            Some(matches_pattern(&v, pattern))
        }
        Predicate::Exists { value } => Some(resolve_value(value, ctx).is_some()),
        Predicate::RoleIs { role } => Some(ctx.actor_roles.iter().any(|r| r == role)),
    }
}

fn cmp_num<F>(left: &Value, right: &Value, ctx: &DecisionContext, f: F) -> Option<bool>
where
    F: Fn(i64, i64) -> bool,
{
    let l = resolve_value(left, ctx)?;
    let r = resolve_value(right, ctx)?;
    match (l, r) {
        (Value::Num(a), Value::Num(b)) => Some(f(a, b)),
        _ => None,
    }
}

fn resolve_value(value: &Value, ctx: &DecisionContext) -> Option<Value> {
    match value {
        Value::Ref(reference) => resolve_ref(reference, ctx),
        other => Some(other.clone()),
    }
}

fn resolve_ref(reference: &Ref, ctx: &DecisionContext) -> Option<Value> {
    match reference {
        Ref::ActorDid => Some(Value::Str(ctx.actor_did.clone())),
        Ref::ActorRole => ctx.actor_roles.first().map(|v| Value::Str(v.clone())),
        Ref::TargetField { field } => ctx.target_fields.get(field).and_then(json_to_value),
        Ref::ResourcePathVar { var } => ctx
            .resource
            .path_vars
            .get(var)
            .map(|v| Value::Str(v.clone())),
        Ref::TimePeriod => ctx.time_period.clone().map(Value::Str),
    }
}

fn json_to_value(value: &serde_json::Value) -> Option<Value> {
    match value {
        serde_json::Value::String(v) => Some(Value::Str(v.clone())),
        serde_json::Value::Number(v) => v.as_i64().map(Value::Num),
        serde_json::Value::Bool(v) => Some(Value::Bool(*v)),
        _ => None,
    }
}

fn matches_pattern(value: &Value, pattern: &str) -> bool {
    let Value::Str(s) = value else {
        return false;
    };

    if pattern == "*" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        return s.starts_with(prefix);
    }

    s == pattern
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::policy::{DelegationRule, PolicyFacts, PolicyRule};

    fn layer_resource(name: &str) -> ResourceSelector {
        ResourceSelector::Layer {
            name: name.to_string(),
        }
    }

    fn ctx(role: &str, action: Action, layer: &str) -> DecisionContext {
        DecisionContext {
            actor_did: "did:key:actor".to_string(),
            actor_roles: vec![role.to_string()],
            audience_did: Some("did:key:actor".to_string()),
            action,
            resource: crate::policy::ResourceContext {
                layer: Some(layer.to_string()),
                path_vars: HashMap::new(),
            },
            target_fields: HashMap::new(),
            time_period: None,
        }
    }

    #[test]
    fn deny_takes_precedence_over_allow() {
        let facts = PolicyFacts {
            roles: vec!["customer".to_string()],
            policy_rules: vec![
                PolicyRule {
                    effect: PolicyEffect::Allow,
                    subjects: vec![Subject::Role {
                        name: "customer".to_string(),
                    }],
                    actions: vec![Action::Write],
                    resource: layer_resource("orders"),
                    condition: None,
                },
                PolicyRule {
                    effect: PolicyEffect::Deny,
                    subjects: vec![Subject::Role {
                        name: "customer".to_string(),
                    }],
                    actions: vec![Action::Write],
                    resource: layer_resource("orders"),
                    condition: None,
                },
            ],
            delegation_rules: vec![],
            dynamic_layer_schemas: vec![],
        };

        let decision = can_access(&facts, &ctx("customer", Action::Write, "orders"));
        assert_eq!(decision, TokenDecision::Deny);
    }

    #[test]
    fn no_escalation_blocks_delegation_without_actor_rights() {
        let facts = PolicyFacts {
            roles: vec!["owner".to_string(), "viewer".to_string()],
            policy_rules: vec![PolicyRule {
                effect: PolicyEffect::Allow,
                subjects: vec![Subject::Role {
                    name: "owner".to_string(),
                }],
                actions: vec![Action::Read],
                resource: layer_resource("orders"),
                condition: None,
            }],
            delegation_rules: vec![DelegationRule {
                from: Subject::Role {
                    name: "owner".to_string(),
                },
                to: Subject::Role {
                    name: "viewer".to_string(),
                },
                actions: vec![Action::Write],
                resource: layer_resource("orders"),
                max_depth: 2,
                no_escalation: true,
                condition: None,
            }],
            dynamic_layer_schemas: vec![],
        };

        let request = DelegationRequest {
            to_role: "viewer".to_string(),
            actions: vec![Action::Write],
            resource: layer_resource("orders"),
            depth: 1,
        };

        let decision = can_delegate(&facts, &ctx("owner", Action::Delegate, "orders"), &request);
        assert_eq!(decision, TokenDecision::Deny);
    }
}
