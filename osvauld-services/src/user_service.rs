use crypto_utils::{CryptoUtils, get_key_id};
use osvauld_core::models::device::Device;
use osvauld_core::models::user::User;
use osvauld_core::repositories::{
    DeviceRepository, RepositoryError, SyncRepository, UserRepository,
};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Error, Debug)]
pub enum UserServiceError {
    #[error("Repository error: {0}")]
    RepositoryError(#[from] RepositoryError),
    // #[error("Invalid input: {0}")]
    // ValidationError(String),
}

pub struct UserService {
    user_repository: Arc<dyn UserRepository>,
    crypto_utils: Arc<Mutex<CryptoUtils>>,
    sync_repository: Arc<dyn SyncRepository>,
    device_repository: Arc<dyn DeviceRepository>,
}
impl UserService {
    pub fn new(
        user_repository: Arc<dyn UserRepository>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
        sync_repository: Arc<dyn SyncRepository>,
        device_repository: Arc<dyn DeviceRepository>,
    ) -> Self {
        Self {
            user_repository,
            crypto_utils,
            sync_repository,
            device_repository,
        }
    }

    pub async fn add_known_user(
        &self,
        username: String,
        public_key: String,
        owner: bool,
    ) -> Result<User, String> {
        log::info!("public key {:?}", public_key);
        let key_id = get_key_id(&public_key.clone()).map_err(|e| e.to_string())?;
        let signature = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .sign_message(&public_key)
                .map_err(|e| e.to_string())?
        };
        let user = User::new(username, key_id, public_key, signature, owner);

        log::info!("adding to users table {:?}", user);
        self.user_repository
            .add_known_user(user.clone())
            .await
            .map_err(|e| e.to_string())?;
        Ok(user)
    }

    pub async fn get_known_users(&self) -> Result<Vec<User>, String> {
        self.user_repository
            .get_known_users()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_user_by_id(&self, user_id: &str) -> Result<User, String> {
        self.user_repository
            .get_user_by_id(user_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_current_user(&self) -> Result<User, String> {
        let public_key = {
            let crypto = self.crypto_utils.lock().await;
            crypto.get_public_key().map_err(|e| e.to_string())?
        };

        let user_id = get_key_id(&public_key).map_err(|e| e.to_string())?;
        self.get_user_by_id(&user_id).await
    }

    pub async fn get_users_with_pending_syncs(
        &self,
    ) -> Result<HashMap<String, Vec<Device>>, String> {
        // Get all devices with unsynced records
        let devices_with_unsynced_records = self
            .sync_repository
            .get_users_with_unsynced_devices()
            .await
            .map_err(|e| e.to_string())?;

        // Group devices by user ID
        let mut result: HashMap<String, Vec<Device>> = HashMap::new();

        for device in devices_with_unsynced_records {
            // Add the device to the map under its user's ID
            result
                .entry(device.user_id.clone())
                .or_insert_with(Vec::new)
                .push(device);
        }

        Ok(result)
    }
    pub async fn update_device_last_synced(&self, device_id: &str) -> Result<(), RepositoryError> {
        self.device_repository
            .udpate_last_synced_at(device_id)
            .await
    }
}
