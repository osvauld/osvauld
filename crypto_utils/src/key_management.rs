use crate::crypto_core;
use crate::errors::CryptoError;
use crate::types::GeneratedKeys;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{SigningKey, VerifyingKey};
use log::info;
use openpgp::{serialize::Marshal, Cert};
use rand::{rngs::OsRng, RngCore};
use sequoia_openpgp::{self as openpgp};
use std::str::FromStr;

/// Generate a new PGP key pair and encrypt the private key with a password
pub fn generate_keys(password: &str, username: &str) -> Result<GeneratedKeys, CryptoError> {
    let cert = crypto_core::generate_certificate(username)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);

    let mut cert_data = Vec::new();
    cert.as_tsk()
        .serialize(&mut cert_data)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    let encrypted_private_key = crypto_core::encrypt_certificate(&cert_data, password, &salt)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    let public_key = crypto_core::get_public_key_armored(&cert)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    Ok(GeneratedKeys {
        private_key: encrypted_private_key,
        public_key,
        salt: general_purpose::STANDARD.encode(salt),
    })
}

/// Generate a new PGP key pair without password protection
pub fn generate_keys_without_password(username: &str) -> Result<GeneratedKeys, CryptoError> {
    let cert = crypto_core::generate_certificate(username)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    let mut cert_data = Vec::new();
    cert.as_tsk()
        .serialize(&mut cert_data)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    let encoded_private_key = general_purpose::STANDARD.encode(cert_data);
    let public_key = crypto_core::get_public_key_armored(&cert)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    Ok(GeneratedKeys {
        private_key: encoded_private_key,
        public_key,
        salt: general_purpose::STANDARD.encode("".as_bytes()),
    })
}

/// Import an existing certificate and encrypt it with a password
pub fn import_certificate(
    cert_string: &str,
    passphrase: &str,
) -> Result<GeneratedKeys, CryptoError> {
    let cert = Cert::from_str(&cert_string).map_err(|e| CryptoError::CertError(e.to_string()))?;

    let public_key = crypto_core::get_public_key_armored(&cert)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);

    let mut cert_data = Vec::new();
    cert.as_tsk()
        .serialize(&mut cert_data)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    let enc_priv_key = crypto_core::encrypt_certificate(&cert_data, passphrase, &salt)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    Ok(GeneratedKeys {
        private_key: enc_priv_key,
        public_key,
        salt: general_purpose::STANDARD.encode(salt),
    })
}

/// Change the password of an encrypted certificate
pub fn change_certificate_password(
    encrypted_cert_b64: &str,
    salt_b64: &str,
    old_password: &str,
    new_password: &str,
) -> Result<String, CryptoError> {
    let cert = crypto_core::decrypt_certificate(encrypted_cert_b64, salt_b64, old_password)
        .map_err(|e| CryptoError::AesError(e))?;

    let mut cert_data = Vec::new();
    cert.as_tsk()
        .serialize(&mut cert_data)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    let salt_array =
        crypto_core::get_salt_arr(salt_b64).map_err(|e| CryptoError::Other(e.to_string()))?;

    let new_encrypted_cert =
        crypto_core::encrypt_certificate(&cert_data, new_password, &salt_array)
            .map_err(|e| CryptoError::Other(e.to_string()))?;

    Ok(new_encrypted_cert)
}

/// Export certificate to an armored string
pub fn export_certificate(
    passphrase: &str,
    enc_pvt_key: &str,
    salt: &str,
) -> Result<String, CryptoError> {
    let cert = crypto_core::decrypt_certificate(enc_pvt_key, salt, passphrase)
        .map_err(|e| CryptoError::AesError(e))?;

    let mut armored = Vec::new();
    cert.as_tsk()
        .armored()
        .serialize(&mut armored)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    Ok(String::from_utf8(armored)?)
}

/// Generate an Ed25519 key pair and encrypt the private key
pub fn generate_and_encrypt_ed25519_key(
    user_public_key: &str,
) -> Result<(String, String), CryptoError> {
    let device_key = SigningKey::generate(&mut OsRng);
    let verifying_key = device_key.verifying_key();

    let private_key_bytes = device_key.to_bytes();
    let public_key_bytes = verifying_key.to_bytes();

    let private_key_b64 = general_purpose::STANDARD.encode(private_key_bytes);
    let public_key_b64 = general_purpose::STANDARD.encode(public_key_bytes);

    let encrypted_private_key = encrypt_string_with_public_key(&private_key_b64, user_public_key)?;

    Ok((encrypted_private_key, public_key_b64))
}

/// Encrypt a string using a PGP public key
pub fn encrypt_string_with_public_key(data: &str, public_key: &str) -> Result<String, CryptoError> {
    let recipient = crypto_core::get_recipient(public_key).map_err(|e| CryptoError::PgpError(e))?;

    let encrypted_data = crypto_core::encrypt_text_pgp(&recipient, data)
        .map_err(|e| CryptoError::Other(e.to_string()))?;
    info!("encrypted key {}", encrypted_data);

    Ok(encrypted_data)
}

/// Get key ID from a public key
pub fn get_key_id(public_key: &str) -> Result<String, CryptoError> {
    crypto_core::generate_key_id(public_key).map_err(|e| CryptoError::Other(e.to_string()))
}

/// Derive a node ID from a base64-encoded Ed25519 public key
pub fn derive_node_id_from_public_key(public_key_b64: &str) -> Result<[u8; 32], String> {
    let public_key_bytes = general_purpose::STANDARD
        .decode(public_key_b64)
        .map_err(|e| format!("Failed to decode public key: {}", e))?;

    if public_key_bytes.len() != 32 {
        return Err(format!(
            "Invalid public key length: expected 32 bytes, got {}",
            public_key_bytes.len()
        ));
    }

    let key_array: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| "Failed to convert to 32-byte array".to_string())?;

    Ok(key_array)
}
