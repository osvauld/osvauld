use crate::errors::AesError;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit},
    Aes256Gcm, Key as Aes_Key, Nonce,
};
use argon2::Argon2;
use base64::{engine::general_purpose, Engine as _};
use rand::rngs::OsRng;
use std::error::Error;

/// Generate a new AES-256-GCM key
pub fn generate_aes_key() -> Aes_Key<Aes256Gcm> {
    Aes256Gcm::generate_key(OsRng)
}

/// Encrypt plaintext with AES-256-GCM using the provided key
/// The nonce is automatically generated and prepended to the ciphertext
pub fn encrypt_with_aes(key: &Aes_Key<Aes256Gcm>, plaintext: &str) -> Result<String, AesError> {
    let cipher = Aes256Gcm::new(key);

    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| AesError::EncryptionError(e.to_string()))?;

    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(general_purpose::STANDARD.encode(combined))
}

/// Decrypt AES-256-GCM encrypted data using the provided key bytes
/// Expects the nonce to be prepended to the ciphertext
pub fn decrypt_with_aes(key_bytes: &[u8], encoded: &str) -> Result<String, AesError> {
    let key = Aes_Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);

    let decoded = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| AesError::Base64DecodeError(e.to_string()))?;

    if decoded.len() < 12 {
        return Err(AesError::DecryptionError("Invalid ciphertext".to_string()));
    }

    let nonce = Nonce::from_slice(&decoded[..12]);
    let ciphertext = &decoded[12..];

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| AesError::DecryptionError(e.to_string()))?;

    String::from_utf8(plaintext).map_err(|e| AesError::Utf8ConversionError(e.to_string()))
}

/// Derive a 32-byte key from a password and salt using Argon2
pub fn derive_key(password: &str, salt: &[u8; 16]) -> Result<[u8; 32], AesError> {
    let mut output_key_material = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut output_key_material)
        .map_err(|e| AesError::KeyDerivationError(e.to_string()))?;
    Ok(output_key_material)
}

/// Encrypt data using password-based encryption with Argon2 key derivation
/// The salt is used both for key derivation and as the AES nonce (first 12 bytes)
pub fn encrypt_certificate(
    data: &[u8],
    password: &str,
    salt: &[u8; 16],
) -> Result<String, Box<dyn Error>> {
    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| AesError::CipherCreationError(e.to_string()))?;

    let nonce = Nonce::from_slice(&salt[..12]);
    let encrypted_data = cipher
        .encrypt(nonce, data)
        .map_err(|e| AesError::EncryptionError(e.to_string()))?;

    Ok(general_purpose::STANDARD.encode(encrypted_data))
}

/// Convert a base64-encoded salt string to a 16-byte array
pub fn get_salt_arr(salt_b64: &str) -> Result<[u8; 16], Box<dyn Error>> {
    let salt = general_purpose::STANDARD.decode(salt_b64)?;

    if salt.len() != 16 {
        return Err("Invalid salt length".into());
    }

    salt.try_into()
        .map_err(|_| "Failed to convert salt to array".into())
}

/// Decrypt password-encrypted data using the provided salt and password
pub fn decrypt_certificate_data(
    encrypted_cert_b64: &str,
    salt_b64: &str,
    password: &str,
) -> Result<Vec<u8>, AesError> {
    let encrypted_data = general_purpose::STANDARD
        .decode(encrypted_cert_b64)
        .map_err(|e| AesError::Base64DecodeError(e.to_string()))?;

    let salt_array = get_salt_arr(salt_b64).unwrap();

    let key = derive_key(password, &salt_array)?;

    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| AesError::CipherCreationError(e.to_string()))?;

    let nonce = Nonce::from_slice(&salt_array[..12]);

    let decrypted_data = cipher
        .decrypt(nonce, encrypted_data.as_ref())
        .map_err(|e| AesError::DecryptionError(e.to_string()))?;

    Ok(decrypted_data)
}
