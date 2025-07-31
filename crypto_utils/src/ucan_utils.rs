use crate::errors::UcanError;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use log::info;
use openpgp::{
    packet::key::{SecretParts, UnspecifiedRole},
    policy::StandardPolicy,
    types::PublicKeyAlgorithm,
    Cert,
};
use sequoia_openpgp::{self as openpgp, crypto::mpi::SecretKeyMaterial};
use serde_json::json;
use std::boxed::Box;
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
pub fn is_one_time_connect_token(ucan: &Ucan, capability_prefix: &str) -> bool {
    let is_wildcard_audience = ucan.audience() == "*";

    let connect_capability = format!("{}:connect", capability_prefix);
    let has_connect_capability = ucan
        .capabilities()
        .iter()
        .any(|cap| cap.resource == connect_capability);

    is_wildcard_audience && has_connect_capability
}

/// Generate a delegation and connection token
/// This creates a token with very long validity that grants both connection and delegation capabilities
/// Issued directly by root authority (not delegated from one-time token)
pub async fn generate_delegation_and_connection_token(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    capability_prefix: &str,
    audience_ucan_pub_key: &str, // Still takes the base64 key
) -> Result<String, UcanError> {
    let key_material = Ed25519KeyMaterial::new(signing_key.clone(), verifying_key.clone());

    // Convert the audience public key to a DID string.
    let audience_did = pub_key_b64_to_did(audience_ucan_pub_key)?;

    let connect_capability = format!("{}:connect", capability_prefix);
    let connect_cap = Capability::from((connect_capability.as_str(), "use", &json!({})));

    let share_capability = format!("{}:share", capability_prefix);
    let share_cap = Capability::from((share_capability.as_str(), "use", &json!({})));

    let long_lifetime = 30 * 365 * 24 * 60 * 60; // 30 years in seconds

    // Build the token using the newly created DID for the audience.
    let ucan = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(&audience_did) // Use the DID string here
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

/// Parses a UCAN token string and performs basic structural and cryptographic
/// validation, including signature and expiration checks.
///
/// ### Arguments
/// * `token` - The raw UCAN token string.
///
/// ### Returns
/// A `Result` containing the parsed and validated `Ucan` object, or a
/// `UcanError` on failure.
pub async fn validate_structure(token: &str) -> Result<Ucan, UcanError> {
    let ucan = Ucan::try_from(token).map_err(|e| UcanError::ParseError(e.to_string()))?;
    let key_constructors: &[(
        &'static [u8],
        fn(Vec<u8>) -> Result<Box<dyn ucan::crypto::KeyMaterial>>,
    )] = &[(ED25519_MAGIC_BYTES, crate::ucan_utils::bytes_to_ed25519_key)];
    let mut did_parser = DidParser::new(key_constructors);

    let now = chrono::Utc::now().timestamp() as u64;
    ucan.validate(Some(now), &mut did_parser)
        .await
        .map_err(|e| UcanError::ValidationError(e.to_string()))?;

    Ok(ucan)
}
/// Validates that the presenter of a UCAN is its intended audience by comparing
/// the raw public key from the `aud` field with the presenter's public key.
///
/// ### Arguments
/// * `ucan` - A reference to the parsed and structurally valid UCAN.
/// * `presenter_ucan_pub_b64` - The base64-encoded public key of the peer presenting the token.
///
/// ### Returns
/// A `Result` that is empty on success, or a `UcanError` on failure.
pub fn validate_audience(ucan: &Ucan, presenter_ucan_pub_b64: &str) -> Result<(), UcanError> {
    verify_did_key_match(ucan.audience(), presenter_ucan_pub_b64).map_err(|e| match e {
        UcanError::PublicKeyMismatch => UcanError::InvalidAudience,
        _ => e,
    })
}

/// Verifies if the public key in a did:key string matches a base64-encoded key.
fn verify_did_key_match(did: &str, key_b64: &str) -> Result<(), UcanError> {
    info!("did {}", did);
    let key_from_did = if did.starts_with("did:key:z") {
        let decoded_did = bs58::decode(&did[9..])
            .into_vec()
            .map_err(|e| UcanError::DidError(e.to_string()))?;

        if decoded_did.len() <= 2 {
            return Err(UcanError::FormatError("DID is too short".to_string()));
        }
        decoded_did[2..].to_vec()
    } else {
        return Err(UcanError::FormatError("Unsupported DID method".to_string()));
    };

    let key_from_b64 = general_purpose::STANDARD
        .decode(key_b64)
        .map_err(|e| UcanError::DecodingError(e.to_string()))?;

    if key_from_did != key_from_b64 {
        return Err(UcanError::PublicKeyMismatch);
    }

    Ok(())
}

/// Verifies that the UCAN contains a capability that grants the required permission.
/// This function supports wildcard matching for hierarchical resources (e.g., a
/// capability for "my-app:*" grants permission for "my-app:feature-a").
///
/// ### Arguments
/// * `ucan` - A reference to the parsed and structurally valid UCAN.
/// * `required_resource` - The resource string that permission is required for.
/// * `required_ability` - The ability string that is required.
///
/// ### Returns
/// A `Result` that is empty on success, or a `UcanError::CapabilityNotFound` on failure.
pub fn check_capability(
    ucan: &Ucan,
    required_resource: &str,
    required_ability: &str,
) -> Result<(), UcanError> {
    for capability in ucan.capabilities().iter() {
        if capability.ability == required_ability {
            let cap_resource = capability.resource;

            if cap_resource.ends_with('*') {
                let prefix = &cap_resource[..cap_resource.len() - 1];
                if required_resource.starts_with(prefix) {
                    return Ok(());
                }
            } else {
                if cap_resource == required_resource {
                    return Ok(());
                }
            }
        }
    }

    //  If the loop completes, no suitable capability was found.
    Err(UcanError::CapabilityNotFound)
}

/// Recursively verifies the UCAN's authority by checking its proof chain.
/// It ensures that the UCAN was either issued directly by the verifier or
/// was delegated by a trusted party in a chain that originates from the verifier.
///
/// ### Arguments
/// * `ucan` - The UCAN to verify.
/// * `verifier_ucan_pub_b64` - The base64-encoded public key of the authority (the verifier).
///
/// ### Returns
/// A `Result` that is empty on success.
pub async fn verify_authority(ucan: &Ucan, verifier_ucan_pub_b64: &str) -> Result<(), UcanError> {
    let issuer_did = ucan.issuer();

    // 1. Base Case: Check if the UCAN was issued directly by the verifier.
    match verify_did_key_match(issuer_did, verifier_ucan_pub_b64) {
        Ok(()) => return Ok(()), // Success! Direct authority confirmed.
        Err(UcanError::PublicKeyMismatch) => {
            // This is expected for a delegated UCAN. We proceed to check proofs.
        }
        Err(e) => return Err(e), // Propagate other errors.
    }

    // 2. Recursive Step: If not issued by the verifier, a proof is required.
    if let Some(proofs_vec) = ucan.proofs() {
        if proofs_vec.is_empty() {
            return Err(UcanError::ProofRequired);
        }

        for proof_cid_string in proofs_vec {
            let parent_ucan = match validate_structure(proof_cid_string).await {
                Ok(ucan) => ucan,
                Err(_) => continue,
            };

            if parent_ucan.audience() != ucan.issuer() {
                continue;
            }

            if check_capability(&parent_ucan, "*", "ucan/share").is_err() {
                return Err(UcanError::DelegationNotPermitted);
            }

            let recursive_call = verify_authority(&parent_ucan, verifier_ucan_pub_b64);
            if Box::pin(recursive_call).await.is_ok() {
                return Ok(());
            }
        }
    } else {
        return Err(UcanError::ProofRequired);
    }

    // If the loop finishes and no valid proof chain was found, fail.
    Err(UcanError::ProofChainInvalid(
        "No valid proof chain leads back to the verifier.".to_string(),
    ))
}

fn pub_key_b64_to_did(key_b64: &str) -> Result<String, UcanError> {
    let key_bytes = general_purpose::STANDARD
        .decode(key_b64)
        .map_err(|e| UcanError::DecodingError(e.to_string()))?;

    // Prepend the Ed25519 multicodec prefix
    let did_bytes = [ED25519_MAGIC_BYTES, key_bytes.as_slice()].concat();

    // bs58 encode and format as a did:key
    let did = format!("did:key:z{}", bs58::encode(did_bytes).into_string());
    Ok(did)
}

/// Generates a "root" UCAN for a new resource, issued by the owner to themselves.
///
/// This token grants full permissions and serves as the root of authority for
/// any future delegations.
pub async fn generate_resource_owner_ucan(
    owner_signing_key: &SigningKey,
    owner_verifying_key: &VerifyingKey,
    resource_id: &str,
    capability_prefix: &str,
) -> Result<(String, String), UcanError> {
    // 1. Create KeyMaterial for the owner using the struct from ucan_utils.rs.
    let key_material =
        Ed25519KeyMaterial::new(owner_signing_key.clone(), owner_verifying_key.clone());

    // 2. The issuer and audience are the same for the owner's root token.
    // The get_did method is defined by the KeyMaterial trait.
    let owner_did = key_material
        .get_did()
        .await
        .map_err(|e| UcanError::DidError(e.to_string()))?;

    // 3. A root token should have a very long lifetime.
    let long_lifetime = 30 * 365 * 24 * 60 * 60; // 30 years in seconds

    // 4. Define the full set of capabilities for the owner.
    let resource_uri = format!("{}:resource:{}", capability_prefix, resource_id);
    let capabilities = vec![
        Capability::from((resource_uri.as_str(), "crud/read", &json!({}))),
        Capability::from((resource_uri.as_str(), "crud/update", &json!({}))),
        Capability::from((resource_uri.as_str(), "crud/delete", &json!({}))),
        // The "ucan/share" capability is essential for allowing delegation.
        Capability::from((resource_uri.as_str(), "ucan/share", &json!({}))),
    ];

    // 5. Build the UCAN using the builder definition provided.
    let mut builder = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(&owner_did)
        .with_lifetime(long_lifetime);

    // Add each capability to the builder.
    for cap in capabilities {
        builder = builder.claiming_capability(cap);
    }

    // Finalize the builder, sign it, and encode it as a string.
    let ucan = builder
        .build()
        .map_err(|e| UcanError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| UcanError::SignatureError(e.to_string()))?;
    let token_cid = ucan
        .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
        .map_err(|e| UcanError::UcanCidConvertionFailed(e.to_string()))?;
    // 6. Return the encoded token string.
    let token_str = ucan
        .encode()
        .map_err(|e| UcanError::EncodingError(e.to_string()))?;
    Ok((token_str, token_cid.to_string()))
}
