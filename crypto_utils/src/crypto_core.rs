use crate::aes_utils;
use crate::errors::AesError;
use crate::pgp_utils;
use openpgp::Cert;
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::{self as openpgp};
// Re-export functions from the specialized modules
pub use aes_utils::{
    decrypt_with_aes, encrypt_certificate, encrypt_with_aes, generate_aes_key, get_salt_arr,
};
pub use pgp_utils::{
    decrypt_text_pgp, encrypt_text_pgp, generate_certificate, generate_key_id, get_decryption_key,
    get_public_key_armored, get_recipient, get_signing_keypair, hash_text_sha512, sign_message,
    verify_signature,
};

/// High-level function that combines AES and PGP operations to decrypt a certificate
/// This bridges the gap between the two crypto modules
pub fn decrypt_certificate(
    encrypted_cert_b64: &str,
    salt_b64: &str,
    password: &str,
) -> Result<Cert, AesError> {
    // Use AES utilities to decrypt the raw certificate data
    let decrypted_data =
        aes_utils::decrypt_certificate_data(encrypted_cert_b64, salt_b64, password)?;

    // Parse the decrypted data into a PGP certificate
    let cert = Cert::from_bytes(&decrypted_data)
        .map_err(|e| AesError::CertificateParseError(e.to_string()))?;

    Ok(cert)
}
