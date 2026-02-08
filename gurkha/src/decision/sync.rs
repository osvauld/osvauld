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

/// Pattern matching for layer names (delegates to parser::matches_wildcard)
fn matches_pattern(layer_name: &str, pattern: &str) -> bool {
    // Treat placeholders like {aud} as wildcards for unexpanded patterns
    let normalized = pattern
        .replace("{page_id}", "*")
        .replace("{aud}", "*")
        .replace("{iss}", "*");
    crate::parser::matches_wildcard(layer_name, &normalized)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_strategies::*;
    use proptest::prelude::*;

    // matches_pattern: Placeholder-to-wildcard conversion

    proptest! {
        /// Patterns with placeholders are normalized to wildcards
        #[test]
        fn matches_pattern_normalizes_placeholders(
            seg1 in segment_strategy(),
            seg2 in segment_strategy(),
            seg3 in segment_strategy(),
        ) {
            let layer = format!("{}/{}/{}", seg1, seg2, seg3);

            // {page_id} becomes * - pattern matches any first segment
            let pattern_page_id = format!("{{page_id}}/{}/{}", seg2, seg3);
            prop_assert!(matches_pattern(&layer, &pattern_page_id),
                "Pattern with {{page_id}} should match via wildcard conversion");

            // {aud} becomes * - pattern matches any middle segment
            let pattern_aud = format!("{}/{{aud}}/{}", seg1, seg3);
            prop_assert!(matches_pattern(&layer, &pattern_aud),
                "Pattern with {{aud}} should match via wildcard conversion");

            // {iss} becomes * - pattern matches any last segment
            let pattern_iss = format!("{}/{}/{{iss}}", seg1, seg2);
            prop_assert!(matches_pattern(&layer, &pattern_iss),
                "Pattern with {{iss}} should match via wildcard conversion");
        }

        /// Multiple placeholders all become wildcards
        #[test]
        fn matches_pattern_multiple_placeholders(
            seg1 in segment_strategy(),
            seg2 in segment_strategy(),
            seg3 in segment_strategy(),
        ) {
            let layer = format!("{}/{}/{}", seg1, seg2, seg3);

            // All placeholders become wildcards
            prop_assert!(matches_pattern(&layer, "{page_id}/{aud}/{iss}"),
                "All placeholders should become wildcards");
        }
    }

}
