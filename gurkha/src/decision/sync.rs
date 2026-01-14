//! Sync Decisions
//!
//! Decision functions for sync authorization:
//! - SyncContext for dual-permit decisions
//! - should_send_updates
//! - can_receive_updates
//! - should_request_updates

use crate::types::SyncDecision;

/// Context for dual-permit sync decisions (CEL-based)
///
/// Uses CEL functions embedded in permits for authorization decisions.
/// Falls back to hardcoded logic if functions are not present.
#[derive(Debug, Clone)]
pub struct SyncContext {
    our_permit: crate::parser::Permit,
    peer_permit: crate::parser::Permit,
}

impl SyncContext {
    /// Create sync context from two raw tokens
    pub fn new(our_token: &str, peer_token: &str) -> Result<Self, String> {
        let our_permit = crate::parser::Permit::from_token(our_token)
            .map_err(|e| format!("Failed to parse our token: {}", e))?;
        let peer_permit = crate::parser::Permit::from_token(peer_token)
            .map_err(|e| format!("Failed to parse peer token: {}", e))?;

        Ok(Self { our_permit, peer_permit })
    }

    /// Create sync context from parsed permits
    pub fn from_permits(our_permit: crate::parser::Permit, peer_permit: crate::parser::Permit) -> Self {
        Self { our_permit, peer_permit }
    }

    pub fn our_permit(&self) -> &crate::parser::Permit {
        &self.our_permit
    }

    pub fn peer_permit(&self) -> &crate::parser::Permit {
        &self.peer_permit
    }
}

/// Determine if we should send updates for a layer
///
/// Uses layer capabilities and patterns from permit.
/// Returns DontSend if no access.
pub fn should_send_updates(context: &SyncContext, layer_name: &str) -> SyncDecision {
    tracing::debug!("🔍 [should_send_updates] Checking layer '{}'", layer_name);

    // Check fixed layer capability
    if let Some(config) = context.our_permit.get_layer_config(layer_name) {
        if config.sync {
            tracing::info!("✅ [should_send_updates] '{}' → SendIncrementalUpdates (fixed layer)", layer_name);
            return SyncDecision::SendIncrementalUpdates;
        }
    }

    // Check layer patterns
    for (pattern, config) in context.our_permit.layer_patterns() {
        if config.sync && matches_pattern(layer_name, pattern) {
            tracing::info!("✅ [should_send_updates] '{}' → SendIncrementalUpdates (pattern match)", layer_name);
            return SyncDecision::SendIncrementalUpdates;
        }
    }

    tracing::info!("🚫 [should_send_updates] '{}' → DontSend (no access)", layer_name);
    SyncDecision::DontSend
}

/// Simple pattern matching for layer names
fn matches_pattern(layer_name: &str, pattern: &str) -> bool {
    // Split into parts and compare
    let layer_parts: Vec<&str> = layer_name.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();

    if layer_parts.len() != pattern_parts.len() {
        return false;
    }

    layer_parts.iter().zip(pattern_parts.iter()).all(|(layer, pat)| {
        *pat == "*" || pat.starts_with('{') || layer == pat
    })
}

/// Check if we can receive updates for a layer
///
/// Uses layer capabilities and patterns from permit.
/// Returns false if no access.
pub fn can_receive_updates(context: &SyncContext, layer_name: &str) -> bool {
    tracing::debug!("🔒 [can_receive_updates] Checking layer '{}'", layer_name);

    // Check fixed layer capability
    if let Some(config) = context.our_permit.get_layer_config(layer_name) {
        if config.sync {
            tracing::info!("🎯 [can_receive_updates] '{}' → true (fixed layer)", layer_name);
            return true;
        }
    }

    // Check layer patterns
    for (pattern, config) in context.our_permit.layer_patterns() {
        if config.sync && matches_pattern(layer_name, pattern) {
            tracing::info!("🎯 [can_receive_updates] '{}' → true (pattern match)", layer_name);
            return true;
        }
    }

    tracing::info!("🎯 [can_receive_updates] '{}' → false (no access)", layer_name);
    false
}

/// Determine if we should REQUEST updates for a layer (used in sync requests)
///
/// Uses simple sync rules - if we can receive, we should request.
/// Returns DontSend if no capability or sync disabled.
pub fn should_request_updates(context: &SyncContext, layer_name: &str) -> SyncDecision {
    tracing::debug!("🔍 [should_request_updates] Checking layer '{}'", layer_name);

    // TODO: Implement simple permit-based logic in Phase 4
    if can_receive_updates(context, layer_name) {
        tracing::info!("✅ [should_request_updates] '{}' → RequestUpdates", layer_name);
        SyncDecision::SendIncrementalUpdates
    } else {
        tracing::info!("🚫 [should_request_updates] '{}' → DontRequest", layer_name);
        SyncDecision::DontSend
    }
}
