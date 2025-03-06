mod crypto_core;
mod errors;
pub mod types;

use crate::errors::{AesError, CryptoUtilsError, PgpError};
use crate::types::{
    EncryptedDataWithAccess, EncryptedResource, GeneratedKeys, ResourceWithEncryptedKey,
    UserAccess, UserPublicKey,
};
use aes_gcm::{Aes256Gcm, Key as Aes_Key};
use anyhow::Result;
use argon2::password_hash::rand_core::OsRng;
use base64::{decode, encode};
use openpgp::{policy::StandardPolicy, serialize::Marshal, Cert};
use rand::RngCore;
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

    #[error("Other error: {0}")]
    Other(String),
}

// Public API - Stateless Functions
// These functions don't require any persistent state and can be called directly

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
        salt: encode(salt),
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

    let encoded_private_key = encode(cert_data);
    let public_key = crypto_core::get_public_key_armored(&cert)
        .map_err(|e| CryptoError::Other(e.to_string()))?;

    Ok(GeneratedKeys {
        private_key: encoded_private_key,
        public_key,
        salt: encode("".as_bytes()),
    })
}

/// Import an existing certificate and encrypt it with a password
pub fn import_certificate(
    cert_string: &str,
    passphrase: &str,
) -> Result<GeneratedKeys, CryptoError> {
    let cert = Cert::from_str(cert_string).map_err(|e| CryptoError::CertError(e.to_string()))?;

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
        salt: encode(salt),
    })
}

/// Encrypt data for multiple users using their public keys
pub fn encrypt_data_for_users(
    data: &str,
    users: &[UserPublicKey],
) -> Result<EncryptedDataWithAccess, CryptoError> {
    // Generate AES key
    let aes_key = crypto_core::generate_aes_key();

    // Encrypt data with AES key - convert AesError
    let encrypted_data =
        crypto_core::encrypt_with_aes(&aes_key, data).map_err(|e| CryptoError::AesError(e))?;

    // Encrypt AES key for each user
    let mut access_list = Vec::new();
    for user in users {
        let recipient =
            crypto_core::get_recipient(&user.public_key).map_err(|e| CryptoError::PgpError(e))?;

        let encrypted_key = crypto_core::encrypt_text_pgp(&recipient, &encode(aes_key.as_slice()))
            .map_err(|e| CryptoError::Other(e.to_string()))?;

        access_list.push(UserAccess {
            user_id: user.user_id.clone(),
            access: user.access.clone(),
            encrypted_key,
        });
    }

    Ok(EncryptedDataWithAccess {
        encrypted_data,
        access_list,
    })
}

/// Get key ID from a public key
pub fn get_key_id(public_key: &str) -> Result<String, CryptoError> {
    crypto_core::generate_key_id(public_key).map_err(|e| CryptoError::Other(e.to_string()))
}

/// Decrypt AES-encrypted data using a provided key
pub fn decrypt_with_aes(ciphertext: &str, encoded_key: &str) -> Result<String, CryptoError> {
    let key_bytes = decode(encoded_key)?;

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

        Ok(encode(signature))
    }

    /// Sign and hash a message
    pub fn sign_and_hash_message(&self, message: &str) -> Result<String, CryptoError> {
        // Convert PgpError
        let hash_text =
            crypto_core::hash_text_sha512(message).map_err(|e| CryptoError::PgpError(e))?;

        let hash_base64 = encode(&hash_text);

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

        let encrypted_key = crypto_core::encrypt_text_pgp(&recipient, &encode(aes_key.as_slice()))
            .map_err(|e| CryptoError::Other(e.to_string()))?;

        Ok(EncryptedResource {
            encrypted_data,
            encrypted_key,
        })
    }

    /// Decrypt resources using the loaded certificate
    pub fn decrypt_resources(
        &self,
        resources: &[ResourceWithEncryptedKey],
    ) -> Result<Vec<ResourceWithEncryptedKey>, CryptoError> {
        let policy = &StandardPolicy::new();

        // Convert CryptoUtilsError
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        // Convert PgpError
        let decrypt_key =
            crypto_core::get_decryption_key(cert).map_err(|e| CryptoError::PgpError(e))?;

        let mut decrypted_resources = Vec::new();

        for resource in resources {
            // Decrypt the encrypted key - Convert PgpError
            let encrypted_key_bytes = resource.encrypted_key.as_bytes();
            let decrypted_key =
                crypto_core::decrypt_text_pgp(policy, &decrypt_key, encrypted_key_bytes)
                    .map_err(|e| CryptoError::PgpError(e))?;

            // Convert UTF-8 error using From trait
            let aes_key = String::from_utf8(decrypted_key)?;

            // Convert base64 decode error
            let key_bytes = decode(&aes_key)?;

            // Convert AesError
            let decrypted_data = crypto_core::decrypt_with_aes(&key_bytes, &resource.data)
                .map_err(|e| CryptoError::AesError(e))?;

            // Create a new ResourceWithEncryptedKey with decrypted data
            let decrypted_resource = ResourceWithEncryptedKey {
                id: resource.id.clone(),
                resource_type: resource.resource_type.clone(),
                data: decrypted_data,
                signature: resource.signature.clone(),
                encrypted_key: resource.encrypted_key.clone(),
                last_accessed: resource.last_accessed,
                favourite: resource.favourite,
                folder_id: resource.folder_id.clone(),
            };

            decrypted_resources.push(decrypted_resource);
        }

        Ok(decrypted_resources)
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
        let key_bytes = decode(&utf8_key)?;

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
        let newly_encrypted_key = crypto_core::encrypt_text_pgp(&recipient, &encode(&key_bytes))
            .map_err(|e| CryptoError::Other(e.to_string()))?;

        Ok(newly_encrypted_key)
    }
}

// Implement Default for CryptoUtils
impl Default for CryptoUtils {
    fn default() -> Self {
        Self::new()
    }
}
