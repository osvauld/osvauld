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
/// ```ignore
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
}
