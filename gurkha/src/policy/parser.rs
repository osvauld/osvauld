use std::collections::BTreeMap;

use serde::Deserialize;
use ucan::Ucan;

use super::error::PolicyError;
use super::types::PolicyFacts;
use super::validator;

#[derive(Debug, Clone, Default)]
pub struct PeerCapabilities {
    pub relay: bool,
    pub share: bool,
    pub accept_publish: bool,
    pub manage_layer_access: bool,
}

#[derive(Debug, Clone)]
pub struct PolicyPermit {
    raw_token: String,
    parsed: Ucan,
    raw_facts: serde_json::Map<String, serde_json::Value>,
    facts: PolicyFacts,
    proof_chain: Vec<String>,
}

impl PolicyPermit {
    pub fn from_token(token: &str) -> Result<Self, PolicyError> {
        let parsed = Ucan::try_from(token).map_err(|e| PolicyError::TokenParse(e.to_string()))?;

        let facts_ref = parsed.facts().as_ref().ok_or(PolicyError::MissingFacts)?;
        let raw_facts: serde_json::Map<String, serde_json::Value> = facts_ref
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let facts_json =
            serde_json::to_value(facts_ref).map_err(|e| PolicyError::FactsDecode(e.to_string()))?;

        let facts = parse_policy_facts(facts_json)?;
        validator::validate_facts(&facts)?;

        Ok(Self {
            raw_token: token.to_string(),
            parsed: parsed.clone(),
            raw_facts,
            facts,
            proof_chain: parsed.proofs().clone().unwrap_or_default(),
        })
    }

    pub fn parsed(&self) -> &Ucan {
        &self.parsed
    }

    pub fn get_fact(&self, key: &str) -> Option<&serde_json::Value> {
        self.raw_facts.get(key)
    }

    pub fn token_type(&self) -> Option<&str> {
        self.get_fact("token_type")
            .and_then(serde_json::Value::as_str)
    }

    pub fn is_first_connection(&self) -> bool {
        self.get_fact("first_connection")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }

    pub fn role(&self) -> Option<&str> {
        self.get_fact("role").and_then(serde_json::Value::as_str)
    }

    pub fn relationship(&self) -> Option<&str> {
        self.get_fact("relationship")
            .and_then(serde_json::Value::as_str)
    }

    pub fn page_id(&self) -> Option<String> {
        self.get_fact("page_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    }

    pub fn space_id(&self) -> Option<String> {
        self.get_fact("space_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    }

    pub fn user_id(&self) -> Option<String> {
        self.get_fact("user_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    }

    pub fn peer_capabilities(&self) -> PeerCapabilities {
        let caps = self
            .get_fact("peer_capabilities")
            .and_then(serde_json::Value::as_object);
        PeerCapabilities {
            relay: caps
                .and_then(|c| c.get("relay"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            share: caps
                .and_then(|c| c.get("share"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            accept_publish: caps
                .and_then(|c| c.get("accept_publish"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            manage_layer_access: caps
                .and_then(|c| c.get("manage_layer_access"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        }
    }

    pub fn facts(&self) -> &PolicyFacts {
        &self.facts
    }

    pub fn issuer(&self) -> &str {
        self.parsed.issuer()
    }

    pub fn audience(&self) -> &str {
        self.parsed.audience()
    }

    pub fn raw_token(&self) -> &str {
        &self.raw_token
    }

    pub fn proof_chain(&self) -> &[String] {
        &self.proof_chain
    }
}

#[derive(Debug, Deserialize)]
struct Envelope {
    osv_policy: PolicyFacts,
}

fn parse_policy_facts(value: serde_json::Value) -> Result<PolicyFacts, PolicyError> {
    if let Ok(env) = serde_json::from_value::<Envelope>(value.clone()) {
        return Ok(env.osv_policy);
    }

    if let Ok(facts) = serde_json::from_value::<PolicyFacts>(value) {
        return Ok(facts);
    }

    Err(PolicyError::MissingPolicyFacts)
}

pub fn facts_to_ucan_map(
    facts: &PolicyFacts,
) -> Result<BTreeMap<String, serde_json::Value>, PolicyError> {
    let mut map = BTreeMap::new();
    let facts_value =
        serde_json::to_value(facts).map_err(|e| PolicyError::FactsDecode(e.to_string()))?;
    map.insert("osv_policy".to_string(), facts_value);
    Ok(map)
}
