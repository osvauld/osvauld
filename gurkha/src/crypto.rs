//! Ed25519 cryptographic operations for Permit signing
//!
//! This module provides minimal Ed25519 signing and verification for Permit tokens.
//! No PGP operations - those belong in the service layer.

use crate::errors::GurkhaError;
use anyhow::{anyhow, Result as AnyResult};
use async_trait::async_trait;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use serde_json::json;
use ucan::{
    builder::UcanBuilder,
    capability::Capability,
    crypto::{
        did::{ED25519_MAGIC_BYTES},
        JwtSignatureAlgorithm, KeyMaterial,
    },
};

/// Ed25519 key material for Permit signing
#[derive(Clone)]
pub struct Ed25519KeyMaterial(pub VerifyingKey, pub Option<SigningKey>);

impl Ed25519KeyMaterial {
    pub fn new(signing_key: SigningKey, verifying_key: VerifyingKey) -> Self {
        Self(verifying_key, Some(signing_key))
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl KeyMaterial for Ed25519KeyMaterial {
    fn get_jwt_algorithm_name(&self) -> String {
        JwtSignatureAlgorithm::EdDSA.to_string()
    }

    async fn get_did(&self) -> AnyResult<String> {
        let bytes = [ED25519_MAGIC_BYTES, self.0.as_bytes()].concat();
        Ok(format!("did:key:z{}", bs58::encode(bytes).into_string()))
    }

    async fn sign(&self, payload: &[u8]) -> AnyResult<Vec<u8>> {
        match &self.1 {
            Some(private_key) => {
                use ed25519_dalek::Signer;
                let signature = private_key.sign(payload);
                Ok(signature.to_bytes().to_vec())
            }
            None => Err(anyhow!("No private key; cannot sign data")),
        }
    }

    async fn verify(&self, payload: &[u8], signature: &[u8]) -> AnyResult<()> {
        let sig_array: [u8; 64] = signature
            .try_into()
            .map_err(|_| anyhow!("Invalid signature length"))?;
        let signature = Signature::from_bytes(&sig_array);

        use ed25519_dalek::Verifier;
        self.0
            .verify(payload, &signature)
            .map_err(|e| anyhow!("Could not verify signature: {:?}", e))?;

        Ok(())
    }
}

/// Sign a permit token based on a decision structure
///
/// FACTS-ONLY ARCHITECTURE (v3):
/// All authorization is stored in the facts field. The capabilities vec should be empty.
/// The decision layer (decision.rs) ensures this by never adding URI capabilities.
pub async fn sign_permit(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    decision: &crate::decision::TokenDecision,
) -> Result<(String, String), GurkhaError> {
    // Enforce facts-only architecture - capabilities should always be empty
    debug_assert!(
        decision.capabilities.is_empty(),
        "Permit v3 uses facts-only architecture. Capabilities vec should be empty. All authorization must be in facts."
    );

    generate_permit_with_cid(
        signing_key,
        verifying_key,
        &decision.audience,
        decision.capabilities.clone(),
        Some(decision.facts.clone()),
        decision.expiry,
        decision.proofs.clone(),
        decision.proof_tokens.clone(),
    )
    .await
}

/// Generate a permit token with its CID hash
///
/// # Permit v3 Architecture Note
/// The `capabilities` parameter exists for UCAN spec compliance but should be empty in v3.
/// All authorization is stored in the `facts` field using CEL-based rules.
///
/// # Arguments
/// * `capabilities` - Should be empty vec in v3 (all auth in facts)
/// * `facts` - Contains all authorization: operations, documents, cel_rules, etc.
pub async fn generate_permit_with_cid(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    audience: &str,
    capabilities: Vec<(String, String)>,
    facts: Option<serde_json::Map<String, serde_json::Value>>,
    expiry_seconds: Option<u64>,
    proofs: Vec<String>,
    proof_tokens: std::collections::HashMap<String, String>,
) -> Result<(String, String), GurkhaError> {
    // 1. Create KeyMaterial
    let key_material = Ed25519KeyMaterial::new(signing_key.clone(), verifying_key.clone());

    // 2. Set lifetime (default to 30 years if None)
    let lifetime = expiry_seconds.unwrap_or(30 * 365 * 24 * 60 * 60);

    // 3. Build permit base (using UCAN library internally)
    let mut builder = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(audience)
        .with_lifetime(lifetime);

    // 4. Add capabilities (should be empty in v3 - all auth in facts)
    for (resource, ability) in capabilities {
        let cap = Capability::from((resource.as_str(), ability.as_str(), &json!({})));
        builder = builder.claiming_capability(cap);
    }

    // 5. Add facts (v3: contains all authorization)
    // Facts structure: operations, documents, cel_rules, relationship, auth_capabilities, etc.
    let mut facts_map = facts.unwrap_or_default();

    // Add prf_tokens if not empty (extension for self-contained validation)
    if !proof_tokens.is_empty() {
        facts_map.insert("prf_tokens".to_string(), json!(proof_tokens));
    }

    for (key, value) in facts_map {
        builder = builder.with_fact(&key, value);
    }

    // 6. Build signable with proofs
    let mut signable = builder
        .build()
        .map_err(|e| GurkhaError::CreationError(e.to_string()))?;

    // Add proofs to the signable (will go into prf field)
    signable.proofs = proofs;

    // 7. Sign permit
    let permit = signable
        .sign()
        .await
        .map_err(|e| GurkhaError::SignatureError(e.to_string()))?;

    // 7. Compute CID (required for database storage in share records)
    let token_cid = permit
        .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
        .map_err(|e| GurkhaError::PermitCidConversionFailed(e.to_string()))?;

    // 8. Encode token string
    let token_str = permit
        .encode()
        .map_err(|e| GurkhaError::EncodingError(e.to_string()))?;

    Ok((token_str, token_cid.to_string()))
}

/// Calculate CID from an existing permit token string
pub fn get_permit_cid(token: &str) -> Result<String, GurkhaError> {
    use ucan::ucan::Ucan;

    // Decode the token
    let permit = Ucan::try_from(token)
        .map_err(|e| GurkhaError::InvalidPermit(format!("Failed to decode token: {}", e)))?;

    // Calculate CID
    let cid = permit
        .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
        .map_err(|e| GurkhaError::PermitCidConversionFailed(e.to_string()))?;

    Ok(cid.to_string())
}
