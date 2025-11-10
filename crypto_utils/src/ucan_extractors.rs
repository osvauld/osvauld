//! UCAN Extraction Functions
//!
//! Small, composable functions that accept parsed UCAN objects and extract specific data.
//! Each function does ONE thing well and can be composed for complex operations.
//!
//! Design principle: Parse once, extract many times.

use crate::errors::UcanError;
use std::collections::HashMap;
use ucan::ucan::Ucan;

// ==================== ID EXTRACTION ====================

/// Extract ID from UCAN capability with specific pattern and ability
///
/// Generic function that finds a capability matching:
/// - Resource pattern: `{domain}:{resource_type}:{id}`
/// - Required ability (e.g., "add_resources", "request_resources")
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
/// * `domain` - Domain prefix (e.g., "sthalam")
/// * `resource_type` - Resource type (e.g., "folder", "resource")
/// * `required_ability` - Ability to check for (e.g., "add_resources")
///
/// # Returns
/// * `Ok(id)` - Extracted ID from matching capability
/// * `Err(CapabilityNotFound)` - No matching capability found
///
/// # Example
/// ```rust
/// let folder_id = extract_id_with_capability(&ucan, "sthalam", "folder", "add_resources")?;
/// ```
pub fn extract_id_with_capability(
    ucan: &Ucan,
    domain: &str,
    resource_type: &str,
    required_ability: &str,
) -> Result<String, UcanError> {
    let pattern = format!("{}:{}:", domain, resource_type);

    for capability in ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        if cap_resource.starts_with(&pattern) && capability.ability == required_ability {
            if let Some(id) = cap_resource.strip_prefix(&pattern) {
                if !id.is_empty() && id != "*" {
                    return Ok(id.to_string());
                }
            }
        }
    }

    Err(UcanError::CapabilityNotFound)
}

// ==================== CAPABILITY CHECKERS ====================

/// Check if UCAN has a specific capability
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
/// * `resource` - Full resource URI (e.g., "sthalam:folder:abc123")
/// * `ability` - Ability to check (e.g., "add_resources")
///
/// # Returns
/// * `true` if capability exists, `false` otherwise
pub fn has_capability(ucan: &Ucan, resource: &str, ability: &str) -> bool {
    ucan.capabilities().iter().any(|cap| {
        cap.resource == resource && cap.ability == ability
    })
}

/// Get all abilities for a resource pattern
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
/// * `pattern` - Resource pattern to match (e.g., "sthalam:folder:abc123")
///
/// # Returns
/// * Vec of ability strings for matching resources
pub fn get_capabilities_for_pattern(ucan: &Ucan, pattern: &str) -> Vec<String> {
    ucan.capabilities()
        .iter()
        .filter(|cap| cap.resource.starts_with(pattern))
        .map(|cap| cap.ability.to_string())
        .collect()
}

// ==================== FACTS EXTRACTORS ====================

/// Get entire facts map from UCAN
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
///
/// # Returns
/// * Some(facts_map) if facts exist, None otherwise
pub fn get_facts(ucan: &Ucan) -> Option<serde_json::Map<String, serde_json::Value>> {
    ucan.facts().as_ref().map(|f| {
        f.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
}

/// Get specific fact value by key
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
/// * `key` - Fact key to lookup
///
/// # Returns
/// * Some(value) if key exists, None otherwise
pub fn get_fact_value(ucan: &Ucan, key: &str) -> Option<serde_json::Value> {
    ucan.facts()
        .as_ref()
        .and_then(|facts| facts.get(key))
        .cloned()
}

/// Get role from UCAN facts
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
///
/// # Returns
/// * Some(role) if "role" fact exists, None otherwise
pub fn get_role(ucan: &Ucan) -> Option<String> {
    get_fact_value(ucan, "role")
        .and_then(|v| v.as_str().map(String::from))
}

/// Get folder_id from UCAN facts
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
///
/// # Returns
/// * Some(folder_id) if "folder_id" fact exists, None otherwise
pub fn get_folder_id_from_facts(ucan: &Ucan) -> Option<String> {
    get_fact_value(ucan, "folder_id")
        .and_then(|v| v.as_str().map(String::from))
}

// ==================== TEMPLATE EXTRACTORS ====================

/// Get raw template object from UCAN facts
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
/// * `template_key` - Template key (e.g., "owner_template", "viewer_template")
///
/// # Returns
/// * `Ok(template_object)` - Template as JSON object
/// * `Err(TemplateNotFound)` - Template key not found in facts
/// * `Err(TemplateInvalid)` - Template exists but is not an object
pub fn get_template_object(
    ucan: &Ucan,
    template_key: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, UcanError> {
    let facts = get_facts(ucan).ok_or(UcanError::TemplateNotFound)?;

    let template_value = facts
        .get(template_key)
        .ok_or(UcanError::TemplateNotFound)?;

    let template_obj = template_value
        .as_object()
        .ok_or_else(|| {
            UcanError::TemplateInvalid(format!("{} must be an object", template_key))
        })?;

    Ok(template_obj.clone())
}

/// Extract capabilities map from template object
///
/// # Arguments
/// * `template_obj` - Template object from UCAN facts
///
/// # Returns
/// * `Ok(capabilities)` - HashMap of doc_name -> ability
/// * `Err(TemplateInvalid)` - Missing or malformed capabilities field
///
/// # Example
/// ```rust
/// let template_obj = get_template_object(&ucan, "viewer_template")?;
/// let capabilities = extract_capabilities_from_template(&template_obj)?;
/// // capabilities = {"main_doc": "crud/readonly", "meta_doc": "crud/merge"}
/// ```
pub fn extract_capabilities_from_template(
    template_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<HashMap<String, String>, UcanError> {
    let capabilities_value = template_obj.get("capabilities").ok_or_else(|| {
        UcanError::TemplateInvalid("Missing 'capabilities' field".to_string())
    })?;

    let capabilities_obj = capabilities_value.as_object().ok_or_else(|| {
        UcanError::TemplateInvalid("'capabilities' must be an object".to_string())
    })?;

    let mut capabilities = HashMap::new();
    for (doc_name, ability_value) in capabilities_obj {
        let ability = ability_value.as_str().ok_or_else(|| {
            UcanError::TemplateInvalid(format!("Ability for '{}' must be a string", doc_name))
        })?;
        capabilities.insert(doc_name.clone(), ability.to_string());
    }

    Ok(capabilities)
}

/// Extract no_update_from_node list from template object
///
/// # Arguments
/// * `template_obj` - Template object from UCAN facts
///
/// # Returns
/// * Vec of document names that should not accept updates from node
/// * Empty Vec if field not present (optional field)
///
/// # Example
/// ```rust
/// let template_obj = get_template_object(&ucan, "viewer_template")?;
/// let no_update = extract_no_update_from_node(&template_obj);
/// // no_update = ["main_doc"]
/// ```
pub fn extract_no_update_from_node(
    template_obj: &serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    template_obj
        .get("no_update_from_node")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract dont_send_to_node list from template object
///
/// # Arguments
/// * `template_obj` - Template object from UCAN facts
///
/// # Returns
/// * Vec of document names that should not be sent to node
/// * Empty Vec if field not present (optional field)
///
/// # Example
/// ```rust
/// let template_obj = get_template_object(&ucan, "viewer_template")?;
/// let dont_send = extract_dont_send_to_node(&template_obj);
/// // dont_send = ["meta_doc"]
/// ```
pub fn extract_dont_send_to_node(
    template_obj: &serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    template_obj
        .get("dont_send_to_node")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

// ==================== VALIDATION HELPERS ====================

/// Check if audience matches expected public key
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
/// * `expected_pub_key` - Expected audience public key (base64)
///
/// # Returns
/// * `Ok(())` if audience matches
/// * `Err(AudienceMismatch)` if audience doesn't match
pub fn check_audience_match(ucan: &Ucan, expected_pub_key: &str) -> Result<(), UcanError> {
    crate::ucan_utils::validate_audience(ucan, expected_pub_key)
}

/// Check if this is a one-time connect token
///
/// One-time tokens have issuer == audience and no embedded proofs.
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
/// * `domain` - Domain to check (e.g., "sthalam")
///
/// # Returns
/// * `true` if this is a one-time connect token, `false` otherwise
pub fn is_one_time_connect_token(ucan: &Ucan, domain: &str) -> bool {
    crate::ucan_utils::is_one_time_connect_token(ucan, domain)
}
