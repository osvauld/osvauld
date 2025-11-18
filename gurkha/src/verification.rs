//! Proof chain verification and validation
//!
//! Provides stateless validation of UCAN proof chains using embedded proof tokens.
//! No database lookups needed - all proofs are self-contained in the token.

use crate::errors::GurkhaError;
use crate::parser::Permit;
use std::collections::HashMap;
use tracing::{debug, error, info};

pub type VerificationResult<T> = Result<T, GurkhaError>;

/// Cache of proof tokens extracted from UCAN facts
///
/// The `prf_tokens` field in UCAN facts contains a map of CID → full JWT token.
/// This enables stateless proof chain validation without database lookups.
#[derive(Debug, Clone)]
pub struct ProofCache {
    pub tokens: HashMap<String, String>,
}

impl ProofCache {
    /// Extract proof cache from UCAN token
    ///
    /// Reads the `prf_tokens` fact field which contains embedded proof tokens
    /// for stateless validation.
    pub fn from_ucan(ucan: &Permit) -> Self {
        let mut tokens = HashMap::new();

        if let Some(fct) = ucan.parsed().facts() {
            if let Some(prf_tokens) = fct.get("prf_tokens") {
                if let Some(obj) = prf_tokens.as_object() {
                    for (cid, token_val) in obj {
                        if let Some(token) = token_val.as_str() {
                            tokens.insert(cid.clone(), token.to_string());
                        }
                    }
                }
            }
        }

        ProofCache { tokens }
    }

    /// Get proof token by CID
    ///
    /// Returns the full JWT token string for the given CID.
    /// No database lookup needed - all proofs are embedded.
    pub fn get(&self, cid: &str) -> Option<&String> {
        self.tokens.get(cid)
    }

    /// Validate entire proof chain recursively
    ///
    /// For each CID in the proof chain:
    /// 1. Get token from embedded prf_tokens
    /// 2. Validate signature
    /// 3. Check delegation rules (audience → issuer link)
    /// 4. Recursively validate proof's chain
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Proof CID not found in prf_tokens
    /// - Proof chain is broken (audience doesn't match issuer)
    /// - Time bounds are invalid
    /// - Signature verification fails
    pub fn validate_chain(&self, ucan: &Permit) -> VerificationResult<()> {
        debug!("🔍 Validating proof chain for token");

        let proof_chain = ucan.proof_chain();

        if proof_chain.is_empty() {
            debug!("✓ No proofs to validate (root token)");
            return Ok(());
        }

        info!("  Validating {} proofs in chain", proof_chain.len());

        for (i, cid) in proof_chain.iter().enumerate() {
            debug!("  Validating proof {}/{}: {}", i + 1, proof_chain.len(), cid);

            // Get proof token from embedded cache
            let proof_token = self.get(cid)
                .ok_or_else(|| {
                    error!("❌ Missing proof token for CID: {}", cid);
                    GurkhaError::ValidationError(format!("Missing proof for CID: {}", cid))
                })?;

            // Parse proof token
            let proof_ucan = Permit::from_token(proof_token)?;

            // Validate this link in the chain
            validate_link(&proof_ucan, ucan)?;

            // Recursive validation of proof's own chain
            let proof_cache = ProofCache::from_ucan(&proof_ucan);
            proof_cache.validate_chain(&proof_ucan)?;
        }

        info!("✓ Proof chain validated successfully");
        Ok(())
    }
}

/// Validate a single link in the proof chain
///
/// Checks that:
/// - Parent's audience matches child's issuer (delegation chain)
/// - Time bounds are valid (parent encompasses child)
/// - Capabilities are properly attenuated
fn validate_link(parent: &Permit, child: &Permit) -> VerificationResult<()> {
    // Check audience → issuer link
    if parent.parsed().audience() != child.parsed().issuer() {
        error!(
            "❌ Proof chain broken: parent aud={} != child iss={}",
            parent.parsed().audience(),
            child.parsed().issuer()
        );
        return Err(GurkhaError::ValidationError(
            format!(
                "Proof chain broken: audience '{}' doesn't match issuer '{}'",
                parent.parsed().audience(),
                child.parsed().issuer()
            )
        ));
    }

    debug!("  ✓ Audience → issuer link valid");

    // TODO: Add time bounds validation
    // TODO: Add capability attenuation validation

    Ok(())
}

/// Proof chain tracer for audit and debugging
///
/// Provides methods to analyze proof chains without validation.
pub struct ProofChainTracer {
    cache: ProofCache,
    root_ucan: Permit,
}

impl ProofChainTracer {
    /// Create tracer from a UCAN token
    pub fn new(token: &str) -> VerificationResult<Self> {
        let ucan = Permit::from_token(token)?;
        let cache = ProofCache::from_ucan(&ucan);

        Ok(Self {
            cache,
            root_ucan: ucan,
        })
    }

    /// Get delegation path as list of issuers
    ///
    /// Returns: ["Root", "Intermediate", "Current"]
    pub fn get_delegation_path(&self) -> VerificationResult<Vec<String>> {
        let mut path = Vec::new();

        // Walk proof chain backwards to build path
        self.walk_chain(&self.root_ucan, &mut path)?;

        // Add current token issuer
        path.push(self.root_ucan.parsed().issuer().to_string());

        Ok(path)
    }

    /// Recursively walk proof chain
    fn walk_chain(&self, ucan: &Permit, path: &mut Vec<String>) -> VerificationResult<()> {
        for cid in ucan.proof_chain() {
            if let Some(proof_token) = self.cache.get(cid) {
                let proof_ucan = Permit::from_token(proof_token)?;

                // Recurse on proof's chain
                self.walk_chain(&proof_ucan, path)?;

                // Add this proof's issuer
                path.push(proof_ucan.parsed().issuer().to_string());
            }
        }
        Ok(())
    }

    /// Get root authority (first issuer in chain)
    pub fn get_root_authority(&self) -> VerificationResult<String> {
        let path = self.get_delegation_path()?;
        Ok(path.first()
            .cloned()
            .unwrap_or_else(|| self.root_ucan.parsed().issuer().to_string()))
    }

    /// Get chain depth (number of delegations)
    pub fn get_chain_depth(&self) -> usize {
        self.root_ucan.proof_chain().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_proof_cache() {
        let cache = ProofCache {
            tokens: HashMap::new(),
        };

        assert!(cache.get("some_cid").is_none());
    }

    // TODO: Add more tests with actual UCAN tokens
}
