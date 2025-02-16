use crate::domains::models::auth::{Certificate, User};
use crate::domains::models::sync_record::{self, SyncRecordSet};
use crate::domains::models::{device::Device, sync_record::SyncRecord};
use crate::domains::repositories::{
    DeviceRepository, RepositoryError, StoreRepository, SyncRepository,
};
use crypto_utils::CryptoUtils;
use rand::{rngs::OsRng, RngCore};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AuthService {
    store_repository: Arc<dyn StoreRepository>,
    crypto_utils: Arc<Mutex<CryptoUtils>>,
    device_repository: Arc<dyn DeviceRepository>,
    sync_repository: Arc<dyn SyncRepository>,
}

impl AuthService {
    pub fn new(
        store_repository: Arc<dyn StoreRepository>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
        device_repository: Arc<dyn DeviceRepository>,
        sync_repository: Arc<dyn SyncRepository>,
    ) -> Self {
        Self {
            store_repository,
            crypto_utils,
            device_repository,
            sync_repository,
        }
    }

    async fn create_device(
        &self,
        passphrase: &str,
        username: &str,
    ) -> Result<(Device, SyncRecordSet), String> {
        // Generate device keys and get device ID
        let (device_key, device_id) = {
            let crypto = self.crypto_utils.lock().await;
            let keys = crypto
                .generate_keys(passphrase, username)
                .map_err(|e| e.to_string())?;
            let id = CryptoUtils::get_key_id(&keys.public_key).map_err(|e| e.to_string())?;
            (keys, id)
        };

        // Create device certificate from generated keys
        let device_certificate = Certificate {
            private_key: device_key.private_key.clone(),
            public_key: device_key.public_key.clone(),
            salt: device_key.salt.clone(),
        };

        let device = Device::new(device_id.clone(), device_key.public_key);

        let sync_record = SyncRecord::create_signup_device(device.clone());

        // Save device information to repository
        self.device_repository
            .save(device.clone())
            .await
            .map_err(|e| e.to_string())?;
        self.sync_repository
            .add_sync_record_set(sync_record.clone())
            .await
            .map_err(|e| e.to_string())?;

        // Store device certificate
        self.store_repository
            .store_certificate(
                &device_certificate,
                "device_key".to_string(),
                "device_key_salt".to_string(),
            )
            .await
            .map_err(|e| e.to_string())?;
        self.store_repository
            .store_device_key(&device_id)
            .await
            .map_err(|e| e.to_string())?;

        Ok((device, sync_record))
    }

    pub async fn get_current_device(&self) -> Result<Device, RepositoryError> {
        let device_id = self.store_repository.get_device_key().await?;
        let device = self.device_repository.find_by_id(&device_id).await?;
        Ok(device)
    }

    pub async fn handle_sign_up(&self, username: &str, passphrase: &str) -> Result<User, String> {
        // Generate primary keys for the user
        let primary_key = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .generate_keys(passphrase, username)
                .map_err(|e| e.to_string())?
        };

        // Create primary certificate
        let certificate = Certificate {
            private_key: primary_key.private_key.clone(),
            public_key: primary_key.public_key.clone(),
            salt: primary_key.salt.clone(),
        };

        // Create device and get device certificate
        let _device_id = self.create_device(passphrase, username).await?;

        // Store primary certificate
        self.store_repository
            .store_certificate(
                &certificate,
                "primary_key".to_string(),
                "primary_key_salt".to_string(),
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(User {
            username: username.to_string(),
            certificate,
        })
    }

    pub async fn load_certificate(&self, passphrase: &str) -> Result<String, String> {
        let certificate = self
            .store_repository
            .get_certificate("primary_key".to_string(), "primary_key_salt".to_string())
            .await
            .map_err(|e| e.to_string())?;

        let mut crypto = CryptoUtils::new();
        crypto
            .decrypt_and_load_certificate(&certificate.private_key, &certificate.salt, passphrase)
            .map_err(|e| e.to_string())?;

        {
            let mut crypto_utils = self.crypto_utils.lock().await;
            *crypto_utils = crypto;
        }

        let public_key = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .get_public_key()
                .map_err(|e| format!("Failed to get public key: {}", e))?
        };

        Ok(public_key)
    }

    pub async fn is_signed_up(&self) -> Result<bool, String> {
        self.store_repository
            .is_signed_up()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn check_private_key_loaded(&self) -> Result<bool, String> {
        let crypto = self.crypto_utils.lock().await;
        Ok(crypto.is_cert_loaded())
    }

    pub async fn sign_challenge(&self, challenge: &str) -> Result<String, String> {
        let crypto = self.crypto_utils.lock().await;
        crypto
            .sign_message(challenge)
            .map_err(|e| format!("Signing error: {}", e))
    }

    pub async fn hash_and_sign(&self, message: &str) -> Result<String, String> {
        let crypto = self.crypto_utils.lock().await;
        crypto
            .sign_message(message)
            .map_err(|e| format!("Hash and sign error: {}", e))
    }

    pub async fn add_device(
        &self,
        certificate: String,
        passphrase: String,
    ) -> Result<(Device, SyncRecordSet), String> {
        let result = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .import_certificate(certificate, passphrase.clone())
                .map_err(|e| format!("Error importing certificate: {}", e))?
        };

        let certificate = Certificate {
            private_key: result.private_key,
            public_key: result.public_key,
            salt: result.salt,
        };
        let device_sync_data = self.create_device(&passphrase, "test").await?;

        self.store_repository
            .store_certificate(
                &certificate,
                "primary_key".to_string(),
                "primary_key_salt".to_string(),
            )
            .await
            .map_err(|e| e.to_string())?;
        self.load_certificate(&passphrase).await?;

        Ok(device_sync_data)
    }

    pub async fn export_certificate(&self, passphrase: String) -> Result<String, String> {
        let certificate = self
            .store_repository
            .get_certificate("primary_key".to_string(), "primary_key_salt".to_string())
            .await
            .map_err(|e| e.to_string())?;

        let crypto = self.crypto_utils.lock().await;
        crypto
            .export_certificate(&passphrase, &certificate.private_key, &certificate.salt)
            .map_err(|e| format!("Error exporting certificate: {}", e))
    }

    pub async fn change_passphrase(
        &self,
        old_password: String,
        new_password: String,
    ) -> Result<Certificate, String> {
        let certificate = self
            .store_repository
            .get_certificate("primary_key".to_string(), "primary_key_salt".to_string())
            .await
            .map_err(|e| e.to_string())?;

        let new_private_key = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .change_certificate_password(
                    &certificate.private_key,
                    &certificate.salt,
                    &old_password,
                    &new_password,
                )
                .map_err(|e| format!("Error changing certificate password: {}", e))?
        };

        let new_certificate = Certificate {
            private_key: new_private_key,
            public_key: certificate.public_key,
            salt: certificate.salt,
        };

        self.store_repository
            .store_certificate(
                &new_certificate,
                "primary_key".to_string(),
                "primary_key_salt".to_string(),
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(new_certificate)
    }

    pub async fn sign_random_challenge(&self) -> Result<(String, String), String> {
        let challenge = self.generate_challenge();
        let signature = self
            .crypto_utils
            .lock()
            .await
            .sign_message(&challenge)
            .map_err(|e| e.to_string())?;

        Ok((challenge, signature))
    }

    fn generate_challenge(&self) -> String {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        bytes
            .iter()
            .fold(String::with_capacity(64), |mut acc, byte| {
                use std::fmt::Write;
                write!(acc, "{:02x}", byte).unwrap();
                acc
            })
    }
}
