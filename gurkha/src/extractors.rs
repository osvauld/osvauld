//! UCAN Extraction Functions
//!
//! Small, composable functions that accept parsed UCAN objects and extract specific data.

use crate::errors::GurkhaError;
use ucan::ucan::Ucan;
use tracing::{debug, error, warn};

/// Extract ID from any UCAN capability of the given resource type
///
/// This function extracts the ID without validating specific capabilities.
/// Use this when you just need the ID for delegation, not permission validation.
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
/// * `domain` - Domain prefix (e.g., "sthalam")
/// * `resource_type` - Resource type (e.g., "folder", "resource")
///
/// # Returns
/// * `Ok(id)` - Extracted ID from first matching capability
/// * `Err(CapabilityNotFound)` - No matching capability found
///
/// # Example
/// ```rust
/// let resource_id = extract_id_from_resource_type(&ucan, "sthalam", "resource")?;
/// ```
pub fn extract_id_from_resource_type(
    ucan: &Ucan,
    domain: &str,
    resource_type: &str,
) -> Result<String, GurkhaError> {
    let pattern_prefix = format!("{}:{}:", domain, resource_type);

    debug!("🔍 Extracting ID for resource_type: domain={}, resource_type={}",
           domain, resource_type);

    for capability in ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        if cap_resource.starts_with(&pattern_prefix) {
            let parts: Vec<&str> = cap_resource.split(':').collect();

            if parts.len() >= 3 {
                let id = parts[2];

                if !id.is_empty() && id != "*" {
                    debug!("✓ Extracted ID: {}", id);
                    return Ok(id.to_string());
                }
            }
        }
    }

    error!("❌ No valid ID found for resource_type: {}:{}", domain, resource_type);
    Err(GurkhaError::CapabilityNotFound)
}

/// Validate that a UCAN has a specific capability
///
/// This function checks if the token has permission for a specific operation/document.
/// Use this to validate permissions before performing an action.
///
/// # Arguments
/// * `ucan` - Parsed UCAN object
/// * `domain` - Domain prefix (e.g., "sthalam")
/// * `resource_type` - Resource type (e.g., "folder", "resource")
/// * `operation` - Operation/document to validate (e.g., "add_resources", "share_resource")
///
/// # Returns
/// * `Ok(())` - Token has the required capability
/// * `Err(CapabilityNotFound)` - Token does not have the capability
///
/// # Example
/// ```rust
/// validate_has_capability(&ucan, "sthalam", "resource", "share_resource")?;
/// ```
pub fn validate_has_capability(
    ucan: &Ucan,
    domain: &str,
    resource_type: &str,
    operation: &str,
) -> Result<(), GurkhaError> {
    let pattern_prefix = format!("{}:{}:", domain, resource_type);

    debug!("🔍 Validating capability: domain={}, resource_type={}, operation={}",
           domain, resource_type, operation);

    for capability in ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        if cap_resource.starts_with(&pattern_prefix) {
            let parts: Vec<&str> = cap_resource.split(':').collect();

            if parts.len() >= 4 && parts[3] == operation {
                debug!("✓ Token has required capability: {}", operation);
                return Ok(());
            }
        }
    }

    error!("❌ Capability not found: {}:{}:*:{}", domain, resource_type, operation);
    debug!("Available capabilities in token:");
    for (idx, capability) in ucan.capabilities().iter().enumerate() {
        debug!("  [{}] resource='{}', ability='{}'", idx, capability.resource, capability.ability);
    }

    Err(GurkhaError::CapabilityNotFound)
}

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
