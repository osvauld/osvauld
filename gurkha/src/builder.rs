//! Permit Token Builder - Execution Layer
//!
//! Separates "what to build" (TokenDecision) from "how to build" (GurkhaPermitBuilder).
//! This layer handles the actual token construction with automatic proof chain handling.

use crate::crypto::sign_permit;
use crate::decision::TokenDecision;
use crate::errors::GurkhaError;
use tracing::{debug, info};

/// Builder for constructing permit tokens from decisions
///
/// This execution layer takes a TokenDecision (pure logic) and builds the actual
/// token with automatic proof chain handling.
///
/// # Example
/// ```rust
/// let secret_key: [u8; 32] = identity.secret_signing_key();
/// let builder = GurkhaPermitBuilder::from_bytes(&secret_key);
/// let (token, cid) = builder.build(decision).await?;
/// ```
pub struct GurkhaPermitBuilder {
    signing_key_bytes: [u8; 32],
}

impl GurkhaPermitBuilder {
    /// Create a new builder from signing key bytes
    pub fn from_bytes(signing_key_bytes: &[u8; 32]) -> Self {
        Self {
            signing_key_bytes: *signing_key_bytes,
        }
    }

    /// Build a permit token from a decision
    ///
    /// Takes a TokenDecision and constructs the actual token with:
    /// - Capabilities from the decision
    /// - Facts from the decision
    /// - Proof chain (prf field) from decision.proofs
    /// - Embedded proof tokens (prf_tokens fact) from decision.proof_tokens
    ///
    /// # Returns
    /// - `Ok((token_string, cid))` - The JWT token and its CID
    /// - `Err(GurkhaError)` - If token construction fails
    pub async fn build(&self, decision: TokenDecision) -> Result<(String, String), GurkhaError> {
        debug!("Building permit token from decision");
        debug!("  Audience: {}", decision.audience);
        debug!("  Capabilities: {:?}", decision.capabilities);
        debug!("  Proofs in chain: {}", decision.proofs.len());
        debug!("  Embedded proof tokens: {}", decision.proof_tokens.len());

        let result = sign_permit(&self.signing_key_bytes, &decision).await?;

        info!("Permit token built successfully");
        debug!("  CID: {}", result.1);

        Ok(result)
    }

    /// Build token and also validate the proof chain
    ///
    /// This is a convenience method that builds the token and immediately validates
    /// its proof chain using the embedded proof tokens.
    ///
    /// # Returns
    /// - `Ok((token_string, cid))` - If both build and validation succeed
    /// - `Err(GurkhaError)` - If build or validation fails
    pub async fn build_and_validate(
        &self,
        decision: TokenDecision,
    ) -> Result<(String, String), GurkhaError> {
        let (token, cid) = self.build(decision).await?;

        // Validate the proof chain
        use crate::parser::Permit;
        use crate::verification::ProofCache;

        debug!("Validating proof chain in built token");
        let permit = Permit::from_token(&token)?;
        let cache = ProofCache::from_permit(&permit);
        cache.validate_chain(&permit)?;

        info!("Token built and proof chain validated");
        Ok((token, cid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::TokenDecision;

    #[tokio::test]
    async fn test_builder_creates_token() {
        use serde_json::json;

        // Create test keys
        let signing_key_bytes = [1u8; 32];

        // Create simple decision using facts-only architecture (v3)
        let mut decision = TokenDecision::new("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK");
        decision.add_fact("token_type".to_string(), json!("test_token"));
        decision.add_fact("operations".to_string(), json!({"read": "allow"}));

        // Build token
        let builder = GurkhaPermitBuilder::from_bytes(&signing_key_bytes);
        let result = builder.build(decision).await;

        assert!(result.is_ok());
        let (token, cid) = result.unwrap();
        assert!(!token.is_empty());
        assert!(!cid.is_empty());
    }
}
