use crypto_utils::{CryptoUtils, get_key_id};
use log::info;
use osvauld_core::models::device::Device;
use osvauld_core::models::user::User;
use osvauld_core::repositories::{
    DeviceRepository, RepositoryError, SyncRepository, UserRepository, VectorClockRepository,
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
    vector_clock_repo: Arc<dyn VectorClockRepository>,
}
impl UserService {
    pub fn new(
        user_repository: Arc<dyn UserRepository>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
        sync_repository: Arc<dyn SyncRepository>,
        device_repository: Arc<dyn DeviceRepository>,
        vector_clock_repo: Arc<dyn VectorClockRepository>,
    ) -> Self {
        Self {
            user_repository,
            crypto_utils,
            sync_repository,
            device_repository,
            vector_clock_repo,
        }
    }

    pub async fn add_known_user(
        &self,
        username: String,
        user_public_key: String,
        device_public_key: String,
    ) -> Result<(User, Device), String> {
        let user_id = get_key_id(&user_public_key.clone()).map_err(|e| e.to_string())?;
        let device_key_id = get_key_id(&device_public_key).map_err(|e| e.to_string())?;

        let signature = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .sign_message(&user_public_key)
                .map_err(|e| e.to_string())?
        };
        let user = User::new(
            username,
            user_id.clone(),
            user_public_key,
            signature,
            false,
            false,
        );

        let device = Device::new(device_key_id, device_public_key, user_id);
        Ok((user, device))
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
        current_device_id: &str,
    ) -> Result<HashMap<String, Vec<Device>>, String> {
        // Get devices with unsynced records from sync repository
        let devices_with_unsynced_records = self
            .sync_repository
            .get_users_with_unsynced_devices()
            .await
            .map_err(|e| e.to_string())?;

        // Extract device IDs from unsynced records for exclusion
        let unsynced_device_ids: Vec<String> = devices_with_unsynced_records
            .iter()
            .map(|device| device.id.clone())
            .collect();

        // Create exclude list for device repository query
        let mut exclude_devices = unsynced_device_ids;
        exclude_devices.push(current_device_id.to_string());

        // Get all devices except the excluded ones
        let devices_to_check = self
            .device_repository
            .get_all_devices_except(&exclude_devices)
            .await
            .map_err(|e| e.to_string())?;

        // Check which devices need updates based on vector clocks
        let mut additional_devices = Vec::new();

        for device in devices_to_check {
            if let Some(last_synced_at) = device.last_synced_at {
                // Check directly in the repository if device needs update
                let resource_ids = self
                    .sync_repository
                    .get_resource_ids_for_device(&device.id)
                    .await
                    .map_err(|e| e.to_string())?;
                let needs_update = self
                    .vector_clock_repo
                    .check_if_device_needs_update(&resource_ids, last_synced_at, &device.id)
                    .await
                    .map_err(|e| e.to_string())?;

                info!("needs update{} ", needs_update);
                if needs_update {
                    additional_devices.push(device);
                }
            }
        }

        // Combine both sets of devices
        let mut all_devices = devices_with_unsynced_records;
        all_devices.extend(additional_devices);

        // Group devices by user ID
        let mut result: HashMap<String, Vec<Device>> = HashMap::new();
        for device in all_devices {
            result
                .entry(device.user_id.clone())
                .or_insert_with(Vec::new)
                .push(device);
        }

        Ok(result)
    }

    pub async fn update_device_last_synced(&self, device_id: &str) -> Result<(), RepositoryError> {
        self.device_repository
            .update_last_synced_at(device_id)
            .await
    }

    pub async fn get_username(&self, user_id: &str) -> Result<String, RepositoryError> {
        let user = self.user_repository.get_user_by_id(user_id).await?;
        Ok(user.username)
    }
}
