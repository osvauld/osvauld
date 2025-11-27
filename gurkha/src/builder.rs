//! Permit Token Builder - Execution Layer
//!
//! Separates "what to build" (TokenDecision) from "how to build" (GurkhaPermitBuilder).
//! This layer handles the actual token construction with automatic proof chain handling.

use crate::crypto::sign_permit;
use crate::decision::TokenDecision;
use crate::errors::GurkhaError;
use ed25519_dalek::{SigningKey, VerifyingKey};
use tracing::{debug, info};

/// Builder for constructing permit tokens from decisions
///
/// This execution layer takes a TokenDecision (pure logic) and builds the actual
/// token with automatic proof chain handling.
///
/// # Example
/// ```rust
/// let decision = TokenDecision::new(audience, capabilities, facts);
/// let builder = GurkhaPermitBuilder::new(&signing_key, &verifying_key);
/// let (token, cid) = builder.build(decision).await?;
/// ```
pub struct GurkhaPermitBuilder<'a> {
    signing_key: &'a SigningKey,
    verifying_key: &'a VerifyingKey,
}

impl<'a> GurkhaPermitBuilder<'a> {
    /// Create a new builder with cryptographic keys
    pub fn new(signing_key: &'a SigningKey, verifying_key: &'a VerifyingKey) -> Self {
        Self {
            signing_key,
            verifying_key,
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
        debug!("🔨 Building permit token from decision");
        debug!("  Audience: {}", decision.audience);
        debug!("  Capabilities: {:?}", decision.capabilities);
        debug!("  Proofs in chain: {}", decision.proofs.len());
        debug!("  Embedded proof tokens: {}", decision.proof_tokens.len());

        let result = sign_permit(self.signing_key, self.verifying_key, &decision).await?;

        info!("✓ Permit token built successfully");
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

        debug!("🔍 Validating proof chain in built token");
        let permit = Permit::from_token(&token)?;
        let cache = ProofCache::from_permit(&permit);
        cache.validate_chain(&permit)?;

        info!("✓ Token built and proof chain validated");
        Ok((token, cid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::TokenDecision;
    use serde_json::Map;

    #[tokio::test]
    async fn test_builder_creates_token() {
        // Create test keys
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let verifying_key = signing_key.verifying_key();

        // Create simple decision
        let decision = TokenDecision::new(
            "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK".to_string(),
            vec![("sthalam:folder:test_id:read".to_string(), "read".to_string())],
            Map::new(),
        );

        // Build token
        let builder = GurkhaPermitBuilder::new(&signing_key, &verifying_key);
        let result = builder.build(decision).await;

        assert!(result.is_ok());
        let (token, cid) = result.unwrap();
        assert!(!token.is_empty());
        assert!(!cid.is_empty());
    }
}
