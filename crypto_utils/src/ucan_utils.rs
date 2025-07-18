use crate::errors::UcanError;
use ed25519_dalek::{SigningKey, VerifyingKey};

use openpgp::{
    packet::key::{SecretParts, UnspecifiedRole},
    policy::StandardPolicy,
    types::PublicKeyAlgorithm,
    Cert,
};

use sequoia_openpgp::{self as openpgp, crypto::mpi::SecretKeyMaterial};

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
