mod crypto_core;
mod errors;
pub mod types;
mod ucan_utils;

use crate::errors::{AesError, CryptoUtilsError, PgpError, UcanError};
use crate::types::{EncryptedResource, GeneratedKeys};
use aes_gcm::{Aes256Gcm, Key as Aes_Key};
use anyhow::Result;

use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{SecretKey, SigningKey, VerifyingKey};
use log::info;
use openpgp::{policy::StandardPolicy, serialize::Marshal, Cert};
use rand::{rngs::OsRng, RngCore};
use sequoia_openpgp::{self as openpgp};
use std::str::FromStr;
use thiserror::Error;

// Define a comprehensive error type
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("AES error: {0}")]
    AesError(#[from] AesError),

    #[error("PGP error: {0}")]
    PgpError(#[from] PgpError),

    #[error("Crypto utils error: {0}")]
    CryptoUtilsError(#[from] CryptoUtilsError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("Certificate error: {0}")]
    CertError(String),
    #[error("UCAN error: {0}")] // Add this line
    UcanError(#[from] UcanError),

    #[error("Other error: {0}")]
    Other(String),
}

// Public API - Stateless Functions

/// Generate a new PGP key pair and encrypt the private key with a password
pub fn generate_keys(password: &str, username: &str) -> Result<GeneratedKeys, CryptoError> {
    // Convert errors from generate_certificate
    let cert = crypto_core::generate_certificate(username)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);

    let mut cert_data = Vec::new();
    cert.as_tsk()
        .serialize(&mut cert_data)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    // Convert errors from encrypt_certificate
    let encrypted_private_key = crypto_core::encrypt_certificate(&cert_data, password, &salt)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    // Convert errors from get_public_key_armored
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

/// Encrypt data for a single user using their public key
pub fn encrypt_data_for_user(
    data: &str,
    public_key: &str,
) -> Result<(String, String), CryptoError> {
    // Generate AES key
    let aes_key = crypto_core::generate_aes_key();

    // Encrypt data with AES key - convert AesError
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

/// Get key ID from a public key
pub fn get_key_id(public_key: &str) -> Result<String, CryptoError> {
    crypto_core::generate_key_id(public_key).map_err(|e| CryptoError::Other(e.to_string()))
}

/// Decrypt AES-encrypted data using a provided key
pub fn decrypt_with_aes(ciphertext: &str, encoded_key: &str) -> Result<String, CryptoError> {
    let key_bytes = general_purpose::STANDARD.decode(encoded_key)?;

    let plaintext = crypto_core::decrypt_with_aes(&key_bytes, ciphertext)
        .map_err(|e| CryptoError::AesError(e))?;

    Ok(plaintext)
}

/// Change the password of an encrypted certificate
pub fn change_certificate_password(
    encrypted_cert_b64: &str,
    salt_b64: &str,
    old_password: &str,
    new_password: &str,
) -> Result<String, CryptoError> {
    // Convert AesError
    let cert = crypto_core::decrypt_certificate(encrypted_cert_b64, salt_b64, old_password)
        .map_err(|e| CryptoError::AesError(e))?;

    let mut cert_data = Vec::new();
    cert.as_tsk()
        .serialize(&mut cert_data)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    // Convert Box<dyn Error>
    let salt_array =
        crypto_core::get_salt_arr(salt_b64).map_err(|e| CryptoError::Other(e.to_string()))?;

    // Convert Box<dyn Error>
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
    // Convert AesError
    let cert = crypto_core::decrypt_certificate(enc_pvt_key, salt, passphrase)
        .map_err(|e| CryptoError::AesError(e))?;

    let mut armored = Vec::new();
    cert.as_tsk()
        .armored()
        .serialize(&mut armored)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    Ok(String::from_utf8(armored)?)
}
pub fn generate_and_encrypt_ed25519_key(
    user_public_key: &str,
) -> Result<(String, String), CryptoError> {
    // Generate Ed25519 key pair
    let device_key = SigningKey::generate(&mut OsRng);
    let verifying_key = device_key.verifying_key();

    // Convert keys to bytes
    let private_key_bytes = device_key.to_bytes();
    let public_key_bytes = verifying_key.to_bytes();

    // Encode keys as base64 for storage
    let private_key_b64 = general_purpose::STANDARD.encode(private_key_bytes);
    let public_key_b64 = general_purpose::STANDARD.encode(public_key_bytes);

    // Encrypt the private key using the user's PGP public key
    let encrypted_private_key = encrypt_string_with_public_key(&private_key_b64, user_public_key)?;

    Ok((encrypted_private_key, public_key_b64))
}

pub fn encrypt_string_with_public_key(data: &str, public_key: &str) -> Result<String, CryptoError> {
    // Get the recipient from the public key
    let recipient = crypto_core::get_recipient(public_key).map_err(|e| CryptoError::PgpError(e))?;

    // Encrypt the string directly with PGP
    let encrypted_data = crypto_core::encrypt_text_pgp(&recipient, data)
        .map_err(|e| CryptoError::Other(e.to_string()))?;
    info!("encrypted key {}", encrypted_data);

    Ok(encrypted_data)
}

pub fn derive_node_id_from_public_key(public_key_b64: &str) -> Result<[u8; 32], String> {
    // Decode the base64 public key
    let public_key_bytes = general_purpose::STANDARD
        .decode(public_key_b64)
        .map_err(|e| format!("Failed to decode public key: {}", e))?;

    if public_key_bytes.len() != 32 {
        return Err(format!(
            "Invalid public key length: expected 32 bytes, got {}",
            public_key_bytes.len()
        ));
    }

    // Convert the 32-byte array to NodeId using try_from
    let key_array: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| "Failed to convert to 32-byte array".to_string())?;

    Ok(key_array)
}
pub fn verify_signature(
    public_key: &str,
    message: &str,
    signature: &str,
) -> Result<bool, PgpError> {
    crypto_core::verify_signature(public_key, message, signature)
}
// Stateful Certificate Operations
// These operations require a loaded certificate
pub struct CryptoUtils {
    cert: Option<Cert>,
}

impl CryptoUtils {
    /// Create a new CryptoUtils instance with no loaded certificate
    pub fn new() -> Self {
        Self { cert: None }
    }

    /// Check if a certificate is loaded
    pub fn is_cert_loaded(&self) -> bool {
        self.cert.is_some()
    }

    /// Clear the loaded certificate
    pub fn clear_cert(&mut self) {
        self.cert = None;
    }

    /// Get a reference to the loaded certificate or return an error if none is loaded
    pub fn get_cert(&self) -> Result<&Cert, CryptoUtilsError> {
        self.cert
            .as_ref()
            .ok_or(CryptoUtilsError::NoCertificateError)
    }

    /// Decrypt and load a certificate
    pub fn decrypt_and_load_certificate(
        &mut self,
        encrypted_cert_b64: &str,
        salt_b64: &str,
        passphrase: &str,
    ) -> Result<(), CryptoUtilsError> {
        let cert = crypto_core::decrypt_certificate(encrypted_cert_b64, salt_b64, passphrase)
            .map_err(|e| CryptoUtilsError::CertificateDecryptionError(e.to_string()))?;
        self.cert = Some(cert);
        Ok(())
    }

    /// Sign a message using the loaded certificate
    pub fn sign_message(&self, message: &str) -> Result<String, CryptoUtilsError> {
        let cert = self.get_cert()?;

        let keypair = crypto_core::get_signing_keypair(cert)
            .map_err(|e| CryptoUtilsError::SigningKeyError(e.to_string()))?;

        let signature = crypto_core::sign_message(&keypair, message)
            .map_err(|e| CryptoUtilsError::SigningError(e.to_string()))?;

        Ok(general_purpose::STANDARD.encode(signature))
    }

    /// Sign and hash a message
    pub fn sign_and_hash_message(&self, message: &str) -> Result<String, CryptoError> {
        // Convert PgpError
        let hash_text =
            crypto_core::hash_text_sha512(message).map_err(|e| CryptoError::PgpError(e))?;

        let hash_base64 = general_purpose::STANDARD.encode(&hash_text);

        // Convert CryptoUtilsError
        let signature = self
            .sign_message(&hash_base64)
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        Ok(signature)
    }

    /// Get the public key of the loaded certificate
    pub fn get_public_key(&self) -> Result<String, CryptoError> {
        // Convert CryptoUtilsError to CryptoError
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        // Convert anyhow::Error to CryptoError
        crypto_core::get_public_key_armored(cert).map_err(|e| CryptoError::Other(e.to_string()))
    }

    /// Add a new resource encrypted with the loaded certificate's public key
    pub fn add_resource(&self, data: &str) -> Result<EncryptedResource, CryptoError> {
        // Convert CryptoUtilsError
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        // Convert anyhow::Error
        let public_key = crypto_core::get_public_key_armored(cert)
            .map_err(|e| CryptoError::Other(e.to_string()))?;

        let aes_key = crypto_core::generate_aes_key();

        // Convert AesError
        let encrypted_data =
            crypto_core::encrypt_with_aes(&aes_key, data).map_err(|e| CryptoError::AesError(e))?;

        // Handle potential errors from get_recipient and encrypt_text_pgp
        let recipient =
            crypto_core::get_recipient(&public_key).map_err(|e| CryptoError::PgpError(e))?;

        let encrypted_key = crypto_core::encrypt_text_pgp(
            &recipient,
            &general_purpose::STANDARD.encode(aes_key.as_slice()),
        )
        .map_err(|e| CryptoError::Other(e.to_string()))?;

        Ok(EncryptedResource {
            encrypted_data,
            encrypted_key,
        })
    }

    pub fn decrypt_resource(&self, data: &str, encrypted_key: &str) -> Result<String, CryptoError> {
        let policy = &StandardPolicy::new();

        // Get the certificate and decryption key
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        let decrypt_key =
            crypto_core::get_decryption_key(cert).map_err(|e| CryptoError::PgpError(e))?;

        // Decrypt the encrypted key
        let encrypted_key_bytes = encrypted_key.as_bytes();
        let decrypted_key =
            crypto_core::decrypt_text_pgp(policy, &decrypt_key, encrypted_key_bytes)
                .map_err(|e| CryptoError::PgpError(e))?;

        // Convert to UTF-8 string
        let aes_key = String::from_utf8(decrypted_key)?;

        // Decode from base64
        let key_bytes = general_purpose::STANDARD.decode(&aes_key)?;

        // Decrypt the data with AES
        let decrypted_data = crypto_core::decrypt_with_aes(&key_bytes, data)
            .map_err(|e| CryptoError::AesError(e))?;

        Ok(decrypted_data)
    }

    /// Update an existing resource using its encrypted key
    pub fn update_resource(&self, data: &str, encrypted_key: &str) -> Result<String, CryptoError> {
        // Decrypt the AES key using our helper function
        let key_bytes = self.decrypt_aes_key(encrypted_key)?;

        // Create an AES key from the bytes
        let aes_key = Aes_Key::<Aes256Gcm>::from_slice(&key_bytes);

        // Use the AES key to encrypt the new data
        let encrypted_data =
            crypto_core::encrypt_with_aes(aes_key, data).map_err(|e| CryptoError::AesError(e))?;

        Ok(encrypted_data)
    }

    fn decrypt_aes_key(&self, encrypted_key: &str) -> Result<Vec<u8>, CryptoError> {
        let policy = &StandardPolicy::new();

        // Get the certificate
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        // Get the decryption key from the certificate
        let decrypt_key =
            crypto_core::get_decryption_key(cert).map_err(|e| CryptoError::PgpError(e))?;

        // Decrypt the encrypted AES key using PGP
        let encrypted_key_bytes = encrypted_key.as_bytes();
        let decrypted_key =
            crypto_core::decrypt_text_pgp(policy, &decrypt_key, encrypted_key_bytes)
                .map_err(|e| CryptoError::PgpError(e))?;

        // Convert to UTF-8 string
        let utf8_key = String::from_utf8(decrypted_key)?;

        // Decode from base64
        let key_bytes = general_purpose::STANDARD.decode(&utf8_key)?;

        Ok(key_bytes)
    }

    pub fn encrypt_key_with_new_pub_key(
        &self,
        encrypted_key: &str,
        public_key: &str,
    ) -> Result<String, CryptoError> {
        // Decrypt the existing AES key using the loaded certificate
        let key_bytes = self.decrypt_aes_key(encrypted_key)?;

        // Get a recipient from the new public key
        let recipient =
            crypto_core::get_recipient(public_key).map_err(|e| CryptoError::PgpError(e))?;

        // Re-encrypt the AES key with the new public key
        let newly_encrypted_key = crypto_core::encrypt_text_pgp(
            &recipient,
            &general_purpose::STANDARD.encode(&key_bytes),
        )
        .map_err(|e| CryptoError::Other(e.to_string()))?;

        Ok(newly_encrypted_key)
    }
    pub fn get_node_keypair(&self, encrypted_private_key: &str) -> Result<SecretKey, CryptoError> {
        let policy = &StandardPolicy::new();

        // Get the certificate
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        // Get the decryption key from the certificate
        let decrypt_key =
            crypto_core::get_decryption_key(cert).map_err(|e| CryptoError::PgpError(e))?;

        // Decrypt the PGP-encrypted private key
        let enc_bytes = encrypted_private_key.as_bytes();
        let decrypted_bytes = crypto_core::decrypt_text_pgp(policy, &decrypt_key, enc_bytes)
            .map_err(|e| CryptoError::PgpError(e))?;

        // Convert decrypted bytes to UTF-8 string (this should be the base64 private key)
        let utf8_key = String::from_utf8(decrypted_bytes)?;

        // Decode from base64 to get raw key bytes
        let key_bytes = general_purpose::STANDARD.decode(&utf8_key)?;

        // Convert to 32-byte array (Ed25519 private keys are always 32 bytes)
        let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| {
            CryptoError::Other("Invalid private key length, expected 32 bytes".to_string())
        })?;
        Ok(key_array)
    }

    pub fn generate_and_encrypt_ucan_key(&self) -> Result<(String, String), CryptoError> {
        // 1. Get the loaded certificate
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;
        let pub_key = self.get_public_key()?;
        // 2. Derive UCAN keys from PGP key (deterministic)
        let (signing_key, verifying_key) =
            ucan_utils::derive_ucan_keys_from_pgp(cert).map_err(|e| CryptoError::UcanError(e))?;

        // 3. Convert to base64 for storage
        let private_key_b64 = general_purpose::STANDARD.encode(signing_key.to_bytes());
        let public_key_b64 = general_purpose::STANDARD.encode(verifying_key.to_bytes());

        // 4. Encrypt the private key using the user's PGP public key
        let encrypted_private_key = encrypt_string_with_public_key(&private_key_b64, &pub_key)
            .map_err(|e| {
                CryptoError::Other(format!("Failed to encrypt UCAN private key: {}", e))
            })?;

        // Return encrypted private key + public key (ready for DB storage)
        Ok((encrypted_private_key, public_key_b64))
    }

    /// Decrypt and load UCAN keys from stored encrypted private key
    pub fn decrypt_ucan_key(
        &self,
        encrypted_private_key: &str,
    ) -> Result<(SigningKey, VerifyingKey), CryptoError> {
        let policy = &StandardPolicy::new();

        // Get the certificate
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        // Get the decryption key from the certificate
        let decrypt_key =
            crypto_core::get_decryption_key(cert).map_err(|e| CryptoError::PgpError(e))?;

        // Decrypt the PGP-encrypted private key
        let enc_bytes = encrypted_private_key.as_bytes();
        let decrypted_bytes = crypto_core::decrypt_text_pgp(policy, &decrypt_key, enc_bytes)
            .map_err(|e| CryptoError::PgpError(e))?;

        // Convert decrypted bytes to UTF-8 string (this should be the base64 private key)
        let utf8_key = String::from_utf8(decrypted_bytes)?;

        // Decode from base64 to get raw key bytes
        let key_bytes = general_purpose::STANDARD.decode(&utf8_key)?;

        // Convert to 32-byte array (Ed25519 private keys are always 32 bytes)
        let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| {
            CryptoError::Other("Invalid UCAN private key length, expected 32 bytes".to_string())
        })?;

        // Create Ed25519 keys
        let signing_key = SigningKey::from_bytes(&key_array);
        let verifying_key = signing_key.verifying_key();

        Ok((signing_key, verifying_key))
    }
}

// Implement Default for CryptoUtils
impl Default for CryptoUtils {
    fn default() -> Self {
        Self::new()
    }
}
