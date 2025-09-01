use crate::crypto_core;
use crate::errors::CryptoError;
use base64::{engine::general_purpose, Engine as _};

/// Encrypt data for a single user using their public key
/// Returns (encrypted_data, encrypted_aes_key)
pub fn encrypt_data_for_user(
    data: &str,
    public_key: &str,
) -> Result<(String, String), CryptoError> {
    // Generate AES key
    let aes_key = crypto_core::generate_aes_key();

    // Encrypt data with AES key
    let encrypted_data =
        crypto_core::encrypt_with_aes(&aes_key, data).map_err(|e| CryptoError::AesError(e))?;

    // Encrypt AES key for the user
    let recipient = crypto_core::get_recipient(public_key).map_err(|e| CryptoError::PgpError(e))?;

    let encrypted_key = crypto_core::encrypt_text_pgp(
        &recipient,
        &general_purpose::STANDARD.encode(aes_key.as_slice()),
    )
    .map_err(|e| CryptoError::Other(e.to_string()))?;

    Ok((encrypted_data, encrypted_key))
}

/// Decrypt AES-encrypted data using a provided key
pub fn decrypt_with_aes(ciphertext: &str, encoded_key: &str) -> Result<String, CryptoError> {
    let key_bytes = general_purpose::STANDARD.decode(encoded_key)?;

    let plaintext = crypto_core::decrypt_with_aes(&key_bytes, ciphertext)
        .map_err(|e| CryptoError::AesError(e))?;

    Ok(plaintext)
}
