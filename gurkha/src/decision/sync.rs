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
/// With fully-resolved permits, checks if the layer exists with sync=true.
/// Returns DontSend if no access.
pub fn should_send_updates(context: &SyncContext, layer_name: &str) -> SyncDecision {
    tracing::debug!("[should_send_updates] Checking layer '{}'", layer_name);

    if let Some(config) = context.our_permit.get_layer_config(layer_name) {
        if config.sync {
            tracing::info!("[should_send_updates] '{}' → SendIncrementalUpdates", layer_name);
            return SyncDecision::SendIncrementalUpdates;
        }
    }

    tracing::info!("[should_send_updates] '{}' → DontSend (no access)", layer_name);
    SyncDecision::DontSend
}

/// Check if we can receive updates for a layer
///
/// With fully-resolved permits, checks if the layer exists with sync=true.
/// Returns false if no access.
pub fn can_receive_updates(context: &SyncContext, layer_name: &str) -> bool {
    tracing::debug!("[can_receive_updates] Checking layer '{}'", layer_name);

    if let Some(config) = context.our_permit.get_layer_config(layer_name) {
        if config.sync {
            tracing::info!("[can_receive_updates] '{}' → true", layer_name);
            return true;
        }
    }

    tracing::info!("[can_receive_updates] '{}' → false (no access)", layer_name);
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
    use crate::test_fixtures;

    #[test]
    fn should_send_updates_for_synced_layer() {
        let owner = test_fixtures::shop_owner("shop123", "did:key:owner");
        let peer = test_fixtures::shop_customer("shop123", "did:key:peer");
        let ctx = SyncContext::from_permits(owner, peer);

        let result = should_send_updates(&ctx, "shop123/products");
        assert_eq!(result, SyncDecision::SendIncrementalUpdates);
    }

    #[test]
    fn should_not_send_updates_for_unknown_layer() {
        let owner = test_fixtures::shop_owner("shop123", "did:key:owner");
        let peer = test_fixtures::shop_customer("shop123", "did:key:peer");
        let ctx = SyncContext::from_permits(owner, peer);

        let result = should_send_updates(&ctx, "shop123/nonexistent");
        assert_eq!(result, SyncDecision::DontSend);
    }

    #[test]
    fn should_not_send_updates_for_local_only_layer() {
        let owner = test_fixtures::shop_owner("shop123", "did:key:owner");
        let peer = test_fixtures::shop_customer("shop123", "did:key:peer");
        let ctx = SyncContext::from_permits(owner, peer);

        // drafts has sync=false
        let result = should_send_updates(&ctx, "shop123/drafts");
        assert_eq!(result, SyncDecision::DontSend);
    }

    #[test]
    fn can_receive_updates_for_synced_layer() {
        let owner = test_fixtures::shop_owner("shop123", "did:key:owner");
        let peer = test_fixtures::shop_customer("shop123", "did:key:peer");
        let ctx = SyncContext::from_permits(owner, peer);

        assert!(can_receive_updates(&ctx, "shop123/products"));
        assert!(!can_receive_updates(&ctx, "shop123/drafts"));
        assert!(!can_receive_updates(&ctx, "shop123/nonexistent"));
    }

    #[test]
    fn should_request_updates_mirrors_can_receive() {
        let owner = test_fixtures::shop_owner("shop123", "did:key:owner");
        let peer = test_fixtures::shop_customer("shop123", "did:key:peer");
        let ctx = SyncContext::from_permits(owner, peer);

        assert_eq!(
            should_request_updates(&ctx, "shop123/products"),
            SyncDecision::SendIncrementalUpdates
        );
        assert_eq!(
            should_request_updates(&ctx, "shop123/nonexistent"),
            SyncDecision::DontSend
        );
    }
}
