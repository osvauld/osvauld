//! Ed25519 cryptographic operations for UCAN signing
//!
//! This module provides minimal Ed25519 signing and verification for UCAN tokens.
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

/// Ed25519 key material for UCAN signing
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

/// Sign a UCAN token based on a decision structure
pub async fn sign_ucan(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    decision: &crate::decision::TokenDecision,
) -> Result<(String, String), GurkhaError> {
    generate_ucan_with_cid(
        signing_key,
        verifying_key,
        &decision.audience,
        decision.capabilities.clone(),
        Some(decision.facts.clone()),
        decision.expiry,
    )
    .await
}

/// Generate a UCAN token with its CID hash
pub async fn generate_ucan_with_cid(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    audience: &str,
    capabilities: Vec<(String, String)>,
    facts: Option<serde_json::Map<String, serde_json::Value>>,
    expiry_seconds: Option<u64>,
) -> Result<(String, String), GurkhaError> {
    // 1. Create KeyMaterial
    let key_material = Ed25519KeyMaterial::new(signing_key.clone(), verifying_key.clone());

    // 2. Set lifetime (default to 30 years if None)
    let lifetime = expiry_seconds.unwrap_or(30 * 365 * 24 * 60 * 60);

    // 3. Build UCAN with capabilities
    let mut builder = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(audience)
        .with_lifetime(lifetime);

    // 4. Add capabilities
    for (resource, ability) in capabilities {
        let cap = Capability::from((resource.as_str(), ability.as_str(), &json!({})));
        builder = builder.claiming_capability(cap);
    }

    // 5. Add facts if provided
    if let Some(facts_map) = facts {
        for (key, value) in facts_map {
            builder = builder.with_fact(&key, value);
        }
    }

    // 6. Build and sign UCAN
    let ucan = builder
        .build()
        .map_err(|e| GurkhaError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| GurkhaError::SignatureError(e.to_string()))?;

    // 7. Compute CID (required for database storage in share records)
    let token_cid = ucan
        .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
        .map_err(|e| GurkhaError::UcanCidConvertionFailed(e.to_string()))?;

    // 8. Encode token string
    let token_str = ucan
        .encode()
        .map_err(|e| GurkhaError::EncodingError(e.to_string()))?;

    Ok((token_str, token_cid.to_string()))
}

/// Calculate CID from an existing UCAN token string
pub fn get_ucan_cid(token: &str) -> Result<String, GurkhaError> {
    use ucan::ucan::Ucan;

    // Decode the token
    let ucan = Ucan::try_from(token)
        .map_err(|e| GurkhaError::InvalidUcan(format!("Failed to decode token: {}", e)))?;

    // Calculate CID
    let cid = ucan
        .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
        .map_err(|e| GurkhaError::UcanCidConvertionFailed(e.to_string()))?;

    Ok(cid.to_string())
}
