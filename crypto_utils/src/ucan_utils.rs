use crate::errors::UcanError;
use crate::types::TokenValidation;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use openpgp::{
    packet::key::{SecretParts, UnspecifiedRole},
    policy::StandardPolicy,
    types::PublicKeyAlgorithm,
    Cert,
};
use sequoia_openpgp::{self as openpgp, crypto::mpi::SecretKeyMaterial};
use serde_json::json;
use ucan::{
    builder::UcanBuilder,
    capability::Capability,
    crypto::{
        did::{DidParser, ED25519_MAGIC_BYTES},
        JwtSignatureAlgorithm, KeyMaterial,
    },
    Ucan,
};
/// Constructor function for DID parser - converts bytes to Ed25519KeyMaterial
pub fn bytes_to_ed25519_key(bytes: Vec<u8>) -> Result<Box<dyn KeyMaterial>> {
    let key_bytes: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("Invalid key length: expected 32 bytes"))?;

    let public_key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|e| anyhow!("Invalid Ed25519 public key: {}", e))?;

    Ok(Box::new(Ed25519KeyMaterial(public_key, None)))
}

#[derive(Clone)]
pub struct Ed25519KeyMaterial(pub VerifyingKey, pub Option<SigningKey>);

impl Ed25519KeyMaterial {
    pub fn new(signing_key: SigningKey, verifying_key: VerifyingKey) -> Self {
        Self(verifying_key, Some(signing_key))
    }

    pub fn new_verify_only(verifying_key: VerifyingKey) -> Self {
        Self(verifying_key, None)
    }
}

#[cfg_attr(target_arch="wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl KeyMaterial for Ed25519KeyMaterial {
    fn get_jwt_algorithm_name(&self) -> String {
        JwtSignatureAlgorithm::EdDSA.to_string()
    }

    async fn get_did(&self) -> Result<String> {
        let bytes = [ED25519_MAGIC_BYTES, self.0.as_bytes()].concat();
        Ok(format!("did:key:z{}", bs58::encode(bytes).into_string()))
    }

    async fn sign(&self, payload: &[u8]) -> Result<Vec<u8>> {
        match &self.1 {
            Some(private_key) => {
                use ed25519_dalek::Signer;
                let signature = private_key.sign(payload);
                Ok(signature.to_bytes().to_vec())
            }
            None => Err(anyhow!("No private key; cannot sign data")),
        }
    }

    async fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<()> {
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
/// Derive Ed25519 UCAN keys from PGP certificate using deterministic derivation
pub fn derive_ucan_keys_from_pgp(cert: &Cert) -> Result<(SigningKey, VerifyingKey), UcanError> {
    let policy = &StandardPolicy::new();

    // Get the primary key - this should be Ed25519 with Cv25519 cipher suite
    let primary_key = cert.primary_key();

    // Verify it's Ed25519
    if primary_key.key().pk_algo() != PublicKeyAlgorithm::EdDSA {
        return Err(UcanError::KeyExtractionError(
            "Primary key is not Ed25519".to_string(),
        ));
    }

    // Get the secret key material from primary key
    for key_amalgamation in cert.keys().with_policy(policy, None) {
        let key = key_amalgamation.key();

        // Check if this is the primary key
        if key.fingerprint() == primary_key.key().fingerprint() {
            // Try to convert to secret key
            if let Ok(secret_key) = key.clone().parts_into_secret() {
                if secret_key.has_unencrypted_secret() {
                    return derive_ucan_key_from_pgp_secret(&secret_key);
                } else {
                    return Err(UcanError::KeyExtractionError(
                        "Primary key is encrypted - decrypt certificate first".to_string(),
                    ));
                }
            }
        }
    }

    Err(UcanError::KeyExtractionError(
        "Could not access primary key secret material".to_string(),
    ))
}

/// Derive UCAN Ed25519 key from PGP secret key using deterministic derivation
fn derive_ucan_key_from_pgp_secret(
    secret_key: &openpgp::packet::Key<SecretParts, UnspecifiedRole>,
) -> Result<(SigningKey, VerifyingKey), UcanError> {
    // Get the secret key material
    let secret_material = secret_key.secret();

    // Check if it's unencrypted
    if let openpgp::packet::key::SecretKeyMaterial::Unencrypted(unencrypted) = secret_material {
        // Map over the secret material to extract Ed25519 keys
        let result = unencrypted.map(|mpis| {
            match mpis {
                SecretKeyMaterial::EdDSA { scalar } => {
                    // Get the PGP private key bytes
                    let pgp_private_key = scalar.value();

                    if pgp_private_key.len() != 32 {
                        return Err(UcanError::KeyExtractionError(format!(
                            "Expected 32 bytes for Ed25519 secret key, got {}",
                            pgp_private_key.len()
                        )));
                    }

                    // Derive UCAN key from PGP key using Argon2
                    derive_ucan_signing_key(pgp_private_key)
                }
                _ => Err(UcanError::KeyExtractionError(
                    "Not an Ed25519 key".to_string(),
                )),
            }
        });

        result
    } else {
        Err(UcanError::KeyExtractionError(
            "Secret key is encrypted".to_string(),
        ))
    }
}

/// Derive UCAN Ed25519 signing key from PGP private key using Argon2
fn derive_ucan_signing_key(
    pgp_private_key: &[u8],
) -> Result<(SigningKey, VerifyingKey), UcanError> {
    use argon2::Argon2;

    // Use Argon2 for key derivation with domain separation
    let info = b"osvauld-ucan-ed25519-v1";
    let salt = b"osvauld-ucan-salt-v1-16bytes"; // Argon2 needs at least 8 bytes, 16 is good

    // Ensure salt is exactly 16 bytes for Argon2
    let mut salt_array = [0u8; 16];
    let salt_len = salt.len().min(16);
    salt_array[..salt_len].copy_from_slice(&salt[..salt_len]);

    // Derive 32 bytes for Ed25519 private key using Argon2
    let mut derived_key = [0u8; 32];

    // Use info as additional input by combining with pgp_private_key
    let mut input = Vec::with_capacity(pgp_private_key.len() + info.len());
    input.extend_from_slice(pgp_private_key);
    input.extend_from_slice(info);

    Argon2::default()
        .hash_password_into(&input, &salt_array, &mut derived_key)
        .map_err(|e| UcanError::KeyExtractionError(format!("Argon2 derivation failed: {}", e)))?;

    // Create Ed25519 signing key from derived bytes
    let signing_key = SigningKey::from_bytes(&derived_key);
    let verifying_key = signing_key.verifying_key();

    Ok((signing_key, verifying_key))
}

/// Generate a one-time UCAN token for user connection (root capability)
pub async fn generate_one_time_connection_token(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    capability_str: &str,
) -> Result<String, UcanError> {
    let key_material = Ed25519KeyMaterial::new(signing_key.clone(), verifying_key.clone());
    let expiry_seconds = 24 * 60 * 60;
    let capability = Capability::from((capability_str, "use", &json!({})));
    let ucan = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience("*")
        .with_lifetime(expiry_seconds)
        .claiming_capability(capability)
        .build()
        .map_err(|e| UcanError::KeyExtractionError(format!("UCAN build error: {}", e)))?
        .sign()
        .await
        .map_err(|e| UcanError::KeyExtractionError(format!("UCAN signing error: {}", e)))?;

    let token = ucan
        .encode()
        .map_err(|e| UcanError::KeyExtractionError(format!("UCAN encoding error: {}", e)))?;

    Ok(token)
}

/// Check if a UCAN token is a one-time connect token
fn is_one_time_connect_token(ucan: &Ucan, capability_prefix: &str) -> bool {
    let is_wildcard_audience = ucan.audience() == "*";

    let connect_capability = format!("{}:connect", capability_prefix);
    let has_connect_capability = ucan
        .capabilities()
        .iter()
        .any(|cap| cap.resource == connect_capability);

    is_wildcard_audience && has_connect_capability
}

/// Validate UCAN connect token and check if it's one-time
pub async fn validate_connect_ucan_token(
    token: &str,
    capability_prefix: &str,
) -> Result<TokenValidation, String> {
    let ucan = Ucan::try_from(token).map_err(|e| format!("Failed to parse UCAN: {}", e))?;
    let key_constructors: &[(
        &'static [u8],
        fn(Vec<u8>) -> anyhow::Result<Box<dyn KeyMaterial>>,
    )] = &[(ED25519_MAGIC_BYTES, bytes_to_ed25519_key)];
    let mut did_parser = DidParser::new(key_constructors);

    let now = chrono::Utc::now().timestamp() as u64;

    let is_valid = ucan.validate(Some(now), &mut did_parser).await.is_ok();
    let is_one_time = is_one_time_connect_token(&ucan, capability_prefix);

    Ok(TokenValidation {
        is_valid,
        is_one_time,
    })
}

/// Generate a delegation and connection token
/// This creates a token with very long validity that grants both connection and delegation capabilities
/// Issued directly by root authority (not delegated from one-time token)
pub async fn generate_delegation_and_connection_token(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    capability_prefix: &str,
    audience_ucan_pub_key: &str,
) -> Result<String, UcanError> {
    let key_material = Ed25519KeyMaterial::new(signing_key.clone(), verifying_key.clone());

    // Connect capability
    let connect_capability = format!("{}:connect", capability_prefix);
    let connect_cap = Capability::from((connect_capability.as_str(), "use", &json!({})));

    // Share capability
    let share_capability = format!("{}:share", capability_prefix);
    let share_cap = Capability::from((share_capability.as_str(), "use", &json!({})));

    let long_lifetime = 30 * 365 * 24 * 60 * 60; // 30 years in seconds

    // Build delegation token with both capabilities and long expiry
    let ucan = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(audience_ucan_pub_key)
        .with_lifetime(long_lifetime)
        .claiming_capability(connect_cap)
        .claiming_capability(share_cap)
        .build()
        .map_err(|e| UcanError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| UcanError::SignatureError(e.to_string()))?;

    let token = ucan
        .encode()
        .map_err(|e| UcanError::EncodingError(e.to_string()))?;

    Ok(token)
}
