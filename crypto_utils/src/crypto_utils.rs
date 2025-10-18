use crate::crypto_core;
use crate::errors::{CryptoError, CryptoUtilsError, UcanError};
use crate::key_management::{encrypt_string_with_public_key, get_key_id};
use crate::signature_utils;
use crate::types::EncryptedResource;
use crate::ucan_utils;
use aes_gcm::{Aes256Gcm, Key as Aes_Key};
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{SecretKey, SigningKey, VerifyingKey};
use sequoia_openpgp::{policy::StandardPolicy, Cert};
use std::future::Future;
use std::time::Duration;

/// Stateful Certificate Operations
/// These operations require a loaded certificate
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
    pub fn sign_message(&self, message: &str) -> Result<String, CryptoError> {
        let cert = self.get_cert()?;

        let keypair = crypto_core::get_signing_keypair(cert)
            .map_err(|e| CryptoUtilsError::SigningKeyError(e.to_string()))?;

        let signature = crypto_core::sign_message(&keypair, message)
            .map_err(|e| CryptoUtilsError::SigningError(e.to_string()))?;

        Ok(general_purpose::STANDARD.encode(signature))
    }

    /// Sign and hash a message
    pub fn sign_and_hash_message(&self, message: &str) -> Result<String, CryptoError> {
        let hash_text =
            crypto_core::hash_text_sha512(message).map_err(|e| CryptoError::PgpError(e))?;

        let hash_base64 = general_purpose::STANDARD.encode(&hash_text);

        let signature = self.sign_message(&hash_base64)?;

        Ok(signature)
    }

    /// Get the public key of the loaded certificate
    pub fn get_public_key(&self) -> Result<String, CryptoError> {
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        crypto_core::get_public_key_armored(cert).map_err(|e| CryptoError::Other(e.to_string()))
    }

    /// Add a new resource encrypted with the loaded certificate's public key
    pub fn add_resource(&self, data: &str) -> Result<EncryptedResource, CryptoError> {
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        let public_key = crypto_core::get_public_key_armored(cert)
            .map_err(|e| CryptoError::Other(e.to_string()))?;

        let aes_key = crypto_core::generate_aes_key();

        let encrypted_data =
            crypto_core::encrypt_with_aes(&aes_key, data).map_err(|e| CryptoError::AesError(e))?;

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

    /// Decrypt a resource using its encrypted key
    pub fn decrypt_resource(&self, data: &str, encrypted_key: &str) -> Result<String, CryptoError> {
        let policy = &StandardPolicy::new();

        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        let decrypt_key =
            crypto_core::get_decryption_key(cert).map_err(|e| CryptoError::PgpError(e))?;

        let encrypted_key_bytes = encrypted_key.as_bytes();
        let decrypted_key =
            crypto_core::decrypt_text_pgp(policy, &decrypt_key, encrypted_key_bytes)
                .map_err(|e| CryptoError::PgpError(e))?;

        let aes_key = String::from_utf8(decrypted_key)?;
        let key_bytes = general_purpose::STANDARD.decode(&aes_key)?;

        let decrypted_data = crypto_core::decrypt_with_aes(&key_bytes, data)
            .map_err(|e| CryptoError::AesError(e))?;

        Ok(decrypted_data)
    }

    /// Update an existing resource using its encrypted key
    pub fn update_resource(&self, data: &str, encrypted_key: &str) -> Result<String, CryptoError> {
        let key_bytes = self.decrypt_aes_key(encrypted_key)?;
        let aes_key = Aes_Key::<Aes256Gcm>::from_slice(&key_bytes);

        let encrypted_data =
            crypto_core::encrypt_with_aes(aes_key, data).map_err(|e| CryptoError::AesError(e))?;

        Ok(encrypted_data)
    }

    /// Helper function to decrypt an AES key using the loaded certificate
    fn decrypt_aes_key(&self, encrypted_key: &str) -> Result<Vec<u8>, CryptoError> {
        let policy = &StandardPolicy::new();

        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        let decrypt_key =
            crypto_core::get_decryption_key(cert).map_err(|e| CryptoError::PgpError(e))?;

        let encrypted_key_bytes = encrypted_key.as_bytes();
        let decrypted_key =
            crypto_core::decrypt_text_pgp(policy, &decrypt_key, encrypted_key_bytes)
                .map_err(|e| CryptoError::PgpError(e))?;

        let utf8_key = String::from_utf8(decrypted_key)?;
        let key_bytes = general_purpose::STANDARD.decode(&utf8_key)?;

        Ok(key_bytes)
    }

    /// Encrypt an existing AES key with a new public key
    pub fn encrypt_key_with_new_pub_key(
        &self,
        encrypted_key: &str,
        public_key: &str,
    ) -> Result<String, CryptoError> {
        let key_bytes = self.decrypt_aes_key(encrypted_key)?;

        let recipient =
            crypto_core::get_recipient(public_key).map_err(|e| CryptoError::PgpError(e))?;

        let newly_encrypted_key = crypto_core::encrypt_text_pgp(
            &recipient,
            &general_purpose::STANDARD.encode(&key_bytes),
        )
        .map_err(|e| CryptoError::Other(e.to_string()))?;

        Ok(newly_encrypted_key)
    }

    /// Get node keypair from encrypted private key
    pub fn get_node_keypair(&self, encrypted_private_key: &str) -> Result<SecretKey, CryptoError> {
        let policy = &StandardPolicy::new();

        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        let decrypt_key =
            crypto_core::get_decryption_key(cert).map_err(|e| CryptoError::PgpError(e))?;

        let enc_bytes = encrypted_private_key.as_bytes();
        let decrypted_bytes = crypto_core::decrypt_text_pgp(policy, &decrypt_key, enc_bytes)
            .map_err(|e| CryptoError::PgpError(e))?;

        let utf8_key = String::from_utf8(decrypted_bytes)?;
        let key_bytes = general_purpose::STANDARD.decode(&utf8_key)?;

        let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| {
            CryptoError::Other("Invalid private key length, expected 32 bytes".to_string())
        })?;

        Ok(key_array)
    }

    /// Generate and encrypt UCAN keys
    pub fn generate_and_encrypt_ucan_key(&self) -> Result<(String, String), CryptoError> {
        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;
        let pub_key = self.get_public_key()?;

        let (signing_key, verifying_key) =
            ucan_utils::derive_ucan_keys_from_pgp(cert).map_err(|e| CryptoError::UcanError(e))?;

        let private_key_b64 = general_purpose::STANDARD.encode(signing_key.to_bytes());
        let public_key_b64 = general_purpose::STANDARD.encode(verifying_key.to_bytes());

        let encrypted_private_key = encrypt_string_with_public_key(&private_key_b64, &pub_key)
            .map_err(|e| {
                CryptoError::Other(format!("Failed to encrypt UCAN private key: {}", e))
            })?;

        Ok((encrypted_private_key, public_key_b64))
    }

    /// Decrypt and load UCAN keys from stored encrypted private key
    pub fn decrypt_ucan_key(
        &self,
        encrypted_private_key: &str,
    ) -> Result<(SigningKey, VerifyingKey), CryptoError> {
        let policy = &StandardPolicy::new();

        let cert = self
            .get_cert()
            .map_err(|e| CryptoError::CryptoUtilsError(e))?;

        let decrypt_key =
            crypto_core::get_decryption_key(cert).map_err(|e| CryptoError::PgpError(e))?;

        let enc_bytes = encrypted_private_key.as_bytes();
        let decrypted_bytes = crypto_core::decrypt_text_pgp(policy, &decrypt_key, enc_bytes)
            .map_err(|e| CryptoError::PgpError(e))?;

        let utf8_key = String::from_utf8(decrypted_bytes)?;
        let key_bytes = general_purpose::STANDARD.decode(&utf8_key)?;

        let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| {
            CryptoError::Other("Invalid UCAN private key length, expected 32 bytes".to_string())
        })?;

        let signing_key = SigningKey::from_bytes(&key_array);
        let verifying_key = signing_key.verifying_key();

        Ok((signing_key, verifying_key))
    }

    /// Generate a one-time user connect token with specified role
    pub async fn generate_one_time_user_connect_token(
        &self,
        encrypted_private_key: &str,
        domain: &str,
        role: &str,
    ) -> Result<(String, String), CryptoError> {
        let public_key = self.get_public_key()?;
        let user_id = get_key_id(&public_key)?;
        let capability = format!("{}:user-connect:{}", domain, user_id);
        let (signing_key, verifying_key) = self.decrypt_ucan_key(encrypted_private_key)?;

        let token = ucan_utils::generate_one_time_connection_token(
            &signing_key,
            &verifying_key,
            &capability,
            role,
        )
        .await?;

        let public_key_b64 = general_purpose::STANDARD.encode(verifying_key.to_bytes());
        Ok((token, public_key_b64))
    }

    /// Issue connect and share user token
    pub async fn issue_connect_and_share_user_token(
        &self,
        encrypted_private_key: &str,
        domain: &str,
        audience_ucan_pub_key: &str,
        role: &str,
    ) -> Result<String, CryptoError> {
        let (signing_key, verifying_key) = self.decrypt_ucan_key(encrypted_private_key)?;
        let public_key = self.get_public_key()?;
        let user_id = get_key_id(&public_key)?;
        let lifetime = 30 * 365 * 24 * 60 * 60; // 30 years in seconds
        let token = ucan_utils::generate_delegation_and_connection_token(
            &signing_key,
            &verifying_key,
            &user_id,
            audience_ucan_pub_key,
            domain,
            lifetime,
            role,
        )
        .await?;
        Ok(token)
    }

    /// Issue a delegated user connection token with embedded proof chain
    pub async fn issue_delegated_user_connect_token(
        &self,
        encrypted_private_key: &str,
        domain: &str,
        target_user_id: &str,
        audience_ucan_pub_key: &str,
        parent_token: &str,
    ) -> Result<String, CryptoError> {
        let (signing_key, verifying_key) = self.decrypt_ucan_key(encrypted_private_key)?;

        let token = ucan_utils::generate_delegated_user_connection_token(
            &signing_key,
            &verifying_key,
            target_user_id,
            audience_ucan_pub_key,
            domain,
            parent_token,
        )
        .await?;

        Ok(token)
    }

    /// Get public UCAN key from encrypted private key
    pub async fn get_public_ucan_key(
        &self,
        encrypted_ucan_private_key: &str,
    ) -> Result<String, CryptoError> {
        let (_signing_key, verifying_key) = self.decrypt_ucan_key(encrypted_ucan_private_key)?;
        let public_key_b64 = general_purpose::STANDARD.encode(verifying_key.to_bytes());
        Ok(public_key_b64)
    }

    /// Sign a cleartext message with expiry
    pub fn sign_clear_text_message(&self, message: &str) -> Result<String, CryptoError> {
        let cert = self.get_cert()?;
        let keypair = crypto_core::get_signing_keypair(cert)
            .map_err(|e| CryptoUtilsError::SigningKeyError(e.to_string()))?;
        let expiry_duration = Duration::from_secs(10 * 60); // 10 minutes
        let signed_message =
            signature_utils::sign_message_cleartext(&keypair, message, Some(expiry_duration))
                .map_err(|e| CryptoError::from(e))?;
        Ok(signed_message)
    }

    /// Generate resource owner UCAN
    pub async fn generate_resource_owner_ucan(
        &self,
        encrypted_ucan_private_key: &str,
        resource_id: &str,
        capability_prefix: &str,
    ) -> Result<(String, String), CryptoError> {
        let (signing_key, verifying_key) = self.decrypt_ucan_key(encrypted_ucan_private_key)?;
        let (token, cid) = ucan_utils::generate_resource_owner_ucan(
            &signing_key,
            &verifying_key,
            resource_id,
            capability_prefix,
        )
        .await?;
        Ok((token, cid))
    }
    pub async fn generate_folder_owner_ucan(
        &self,
        encrypted_ucan_private_key: &str,
        folder_id: &str,
        capability_prefix: &str,
    ) -> Result<(String, String), CryptoError> {
        let (signing_key, verifying_key) = self.decrypt_ucan_key(encrypted_ucan_private_key)?;
        let (token, cid) = ucan_utils::generate_folder_owner_ucan(
            &signing_key,
            &verifying_key,
            folder_id,
            capability_prefix,
        )
        .await?;
        Ok((token, cid))
    }

    /// Generate public folder view token
    pub async fn generate_public_folder_view_token(
        &self,
        encrypted_ucan_private_key: &str,
        folder_id: &str,
        capability_prefix: &str,
    ) -> Result<String, CryptoError> {
        let (signing_key, verifying_key) = self.decrypt_ucan_key(encrypted_ucan_private_key)?;
        let token = ucan_utils::generate_public_folder_view_token(
            &signing_key,
            &verifying_key,
            folder_id,
            capability_prefix,
        )
        .await?;
        Ok(token)
    }

    /// Issue a delegated folder UCAN after validating permissions
    pub async fn issue_delegated_folder_ucan<F, Fut>(
        &self,
        encrypted_delegator_private_key: &str,
        proof_folder_ucan_string: &str,
        verifier_ucan_pub_b64: &str,
        folder_id: &str,
        recipient_ucan_pub_key: &str,
        permissions_to_grant: Vec<(String, String)>,
        proof_resolver: &F,
    ) -> Result<(String, String), CryptoError>
    where
        F: Fn(&str) -> Fut + Send + Sync,
        Fut: Future<Output = Result<String, UcanError>> + Send + 'static,
    {
        // 1. Validate the proof folder UCAN structure
        let folder_ucan_to_prove = ucan_utils::validate_structure(proof_folder_ucan_string).await?;

        // 2. Extract domain from the parent UCAN's capabilities
        let domain = ucan_utils::extract_domain_from_ucan(&folder_ucan_to_prove, "folder")?;
        
        // 3. Construct the full folder URI for validation
        let folder_resource = format!("{}:folder:{}", domain, folder_id);
        
        ucan_utils::validate_ucan_permission(
            &folder_ucan_to_prove,
            verifier_ucan_pub_b64,
            proof_resolver,
            &folder_resource,
            &"share_folder".to_string(),
        )
        .await?;

        // 4. Decrypt the delegator's UCAN keys
        let (delegator_signing_key, delegator_verifying_key) =
            self.decrypt_ucan_key(encrypted_delegator_private_key)?;

        // 5. Generate the delegated UCAN using existing function
        let (new_token, new_cid) = ucan_utils::generate_delegated_ucan(
            &delegator_signing_key,
            &delegator_verifying_key,
            recipient_ucan_pub_key,
            permissions_to_grant,
            proof_folder_ucan_string,
        )
        .await?;

        Ok((new_token, new_cid))
    }

    /// Issue a delegated resource UCAN after validating permissions
    pub async fn issue_delegated_resource_ucan<F, Fut>(
        &self,
        encrypted_delegator_private_key: &str,
        proof_ucan_string: &str,
        verifier_ucan_pub_b64: &str,
        resource_id: &str,
        recipient_ucan_pub_key: &str,
        permissions_to_grant: Vec<(String, String)>,
        proof_resolver: &F,
    ) -> Result<(String, String), CryptoError>
    where
        F: Fn(&str) -> Fut + Send + Sync,
        Fut: Future<Output = Result<String, UcanError>> + Send + 'static,
    {
        let ucan_to_prove = ucan_utils::validate_structure(proof_ucan_string).await?;

        // Extract domain from the parent UCAN's capabilities
        let domain = ucan_utils::extract_domain_from_ucan(&ucan_to_prove, "resource")?;
        
        // Construct the full resource URI for validation
        let resource_uri = format!("{}:resource:{}", domain, resource_id);

        ucan_utils::validate_ucan_permission(
            &ucan_to_prove,
            verifier_ucan_pub_b64,
            proof_resolver,
            &resource_uri,
            &"ucan/share".to_string(),
        )
        .await?;

        let (delegator_signing_key, delegator_verifying_key) =
            self.decrypt_ucan_key(encrypted_delegator_private_key)?;

        let (new_token, new_cid) = ucan_utils::generate_delegated_ucan(
            &delegator_signing_key,
            &delegator_verifying_key,
            recipient_ucan_pub_key,
            permissions_to_grant,
            proof_ucan_string,
        )
        .await?;

        Ok((new_token, new_cid))
    }
}

/// Implement Default for CryptoUtils
impl Default for CryptoUtils {
    fn default() -> Self {
        Self::new()
    }
}
