use crate::crypto_core;
use crate::errors::{CryptoError, PgpError};
use crate::signature_utils;

/// Verify a PGP signature
pub fn verify_signature(
    public_key: &str,
    message: &str,
    signature: &str,
) -> Result<bool, PgpError> {
    crypto_core::verify_signature(public_key, message, signature)
}

/// Verify a cleartext signed message and return the original message
pub async fn verify_clear_text_message(
    public_key: &str,
    signed_message: &str,
) -> Result<String, CryptoError> {
    let (is_valid, message_option) =
        signature_utils::verify_message_cleartext(public_key, signed_message)
            .map_err(|e| CryptoError::from(e))?;

    if is_valid {
        if let Some(message) = message_option {
            Ok(message)
        } else {
            Err(PgpError::InvalidSignature(
                "Signature is valid but message content is missing.".to_string(),
            ))?
        }
    } else {
        Err(PgpError::InvalidSignature(
            "PGP signature verification failed.".to_string(),
        ))?
    }
}
