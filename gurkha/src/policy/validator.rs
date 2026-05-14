use std::collections::HashSet;

use super::error::PolicyError;
use super::types::{
    ConditionExpr, DelegationRule, PolicyFacts, PolicyRule, Predicate, Ref, Subject, Value,
};

pub fn validate_facts(facts: &PolicyFacts) -> Result<(), PolicyError> {
    let roles: HashSet<&str> = facts.roles.iter().map(String::as_str).collect();

    for rule in &facts.policy_rules {
        validate_policy_rule(rule, &roles)?;
    }
    for rule in &facts.delegation_rules {
        validate_delegation_rule(rule, &roles)?;
    }

    Ok(())
}

fn validate_policy_rule(rule: &PolicyRule, roles: &HashSet<&str>) -> Result<(), PolicyError> {
    if rule.subjects.is_empty() {
        return Err(PolicyError::Validation(
            "policy rule must include subjects".to_string(),
        ));
    }
    if rule.actions.is_empty() {
        return Err(PolicyError::Validation(
            "policy rule must include actions".to_string(),
        ));
    }
    for subject in &rule.subjects {
        validate_subject(subject, roles)?;
    }
    if let Some(condition) = &rule.condition {
        validate_condition(condition)?;
    }
    Ok(())
}

fn validate_delegation_rule(
    rule: &DelegationRule,
    roles: &HashSet<&str>,
) -> Result<(), PolicyError> {
    validate_subject(&rule.from, roles)?;
    validate_subject(&rule.to, roles)?;
    if rule.actions.is_empty() {
        return Err(PolicyError::Validation(
            "delegation rule must include actions".to_string(),
        ));
    }
    if rule.max_depth == 0 {
        return Err(PolicyError::Validation(
            "delegation max_depth must be greater than 0".to_string(),
        ));
    }
    if let Some(condition) = &rule.condition {
        validate_condition(condition)?;
    }
    Ok(())
}

fn validate_subject(subject: &Subject, roles: &HashSet<&str>) -> Result<(), PolicyError> {
    if let Subject::Role { name } = subject {
        if !roles.contains(name.as_str()) {
            return Err(PolicyError::Validation(format!(
                "unknown role '{}' in subject",
                name
            )));
        }
    }
    Ok(())
}

fn validate_condition(condition: &ConditionExpr) -> Result<(), PolicyError> {
    match condition {
        ConditionExpr::Pred { pred } => validate_predicate(pred),
        ConditionExpr::And { args } | ConditionExpr::Or { args } => {
            if args.is_empty() {
                return Err(PolicyError::Validation(
                    "and/or condition requires at least one argument".to_string(),
                ));
            }
            for arg in args {
                validate_condition(arg)?;
            }
            Ok(())
        }
        ConditionExpr::Not { arg } => validate_condition(arg),
    }
}

fn validate_predicate(pred: &Predicate) -> Result<(), PolicyError> {
    match pred {
        Predicate::Eq { left, right }
        | Predicate::Ne { left, right }
        | Predicate::Lt { left, right }
        | Predicate::Le { left, right }
        | Predicate::Gt { left, right }
        | Predicate::Ge { left, right } => validate_comparable(left, right),
        Predicate::In { item, set } => {
            if set.is_empty() {
                return Err(PolicyError::Validation(
                    "in predicate set cannot be empty".to_string(),
                ));
            }
            validate_value(item)?;
            for value in set {
                validate_value(value)?;
            }
            Ok(())
        }
        Predicate::Matches { value, .. } | Predicate::Exists { value } => validate_value(value),
        Predicate::RoleIs { role } => {
            if role.trim().is_empty() {
                return Err(PolicyError::Validation(
                    "role_is predicate role cannot be empty".to_string(),
                ));
            }
            Ok(())
        }
    }
}

fn validate_comparable(left: &Value, right: &Value) -> Result<(), PolicyError> {
    validate_value(left)?;
    validate_value(right)?;
    match (literal_type(left), literal_type(right)) {
        (Some(a), Some(b)) if a != b => Err(PolicyError::Validation(
            "comparison uses incompatible literal types".to_string(),
        )),
        _ => Ok(()),
    }
}

fn validate_value(value: &Value) -> Result<(), PolicyError> {
    if let Value::Ref(reference) = value {
        validate_ref(reference)?;
    }
    Ok(())
}

fn validate_ref(reference: &Ref) -> Result<(), PolicyError> {
    match reference {
        Ref::TargetField { field } => {
            if field.trim().is_empty() {
                return Err(PolicyError::Validation(
                    "target field reference cannot be empty".to_string(),
                ));
            }
        }
        Ref::ResourcePathVar { var } => {
            if var.trim().is_empty() {
                return Err(PolicyError::Validation(
                    "resource path variable cannot be empty".to_string(),
                ));
            }
        }
        Ref::ActorDid | Ref::ActorRole | Ref::TimePeriod => {}
    }
    Ok(())
}

fn literal_type(value: &Value) -> Option<&'static str> {
    match value {
        Value::Str(_) => Some("string"),
        Value::Num(_) => Some("number"),
        Value::Bool(_) => Some("bool"),
        Value::Ref(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{Action, PolicyEffect, PolicyRule, ResourceSelector};

    #[test]
    fn fails_on_unknown_role_subject() {
        let facts = PolicyFacts {
            roles: vec!["owner".to_string()],
            policy_rules: vec![PolicyRule {
                effect: PolicyEffect::Allow,
                subjects: vec![Subject::Role {
                    name: "viewer".to_string(),
                }],
                actions: vec![Action::Read],
                resource: ResourceSelector::Any,
                condition: None,
            }],
            delegation_rules: vec![],
            dynamic_layer_schemas: vec![],
            schema: None,
            validation: None,
        };

        let err = validate_facts(&facts).expect_err("validator should fail");
        assert!(err.to_string().contains("unknown role 'viewer'"));
    }
}
