use super::super::transaction_service::transaction_service::TransactionService;
use osvauld_core::models::device::Device;
use osvauld_core::models::resource::Resource;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::{
    DeviceRepository, FolderRepository, RepositoryError, ResourceRepository, StoreRepository,
    SyncRepository, UserRepository, VectorClockRepository,
};

use std::sync::Arc;

pub enum SyncEvent {
    UpdateEvent {
        remote_resource: Resource,
        vector_clock: Vec<ResourceVectorClock>,
        device_id: String,
        user_id: String,
    },
}

pub struct SyncService {
    pub sync_repository: Arc<dyn SyncRepository>,
    pub folder_repository: Arc<dyn FolderRepository>,
    pub resource_repository: Arc<dyn ResourceRepository>,
    pub device_repository: Arc<dyn DeviceRepository>,
    pub store_repository: Arc<dyn StoreRepository>,
    pub vector_clock_repository: Arc<dyn VectorClockRepository>,
    pub user_repository: Arc<dyn UserRepository>,
    pub db: Arc<TransactionService>,
}

impl SyncService {
    pub fn new(
        sync_repository: Arc<dyn SyncRepository>,
        folder_repository: Arc<dyn FolderRepository>,
        resource_repository: Arc<dyn ResourceRepository>,
        device_repository: Arc<dyn DeviceRepository>,
        store_repository: Arc<dyn StoreRepository>,
        vector_clock_repository: Arc<dyn VectorClockRepository>,
        user_repository: Arc<dyn UserRepository>,
        db: Arc<TransactionService>,
    ) -> Self {
        Self {
            sync_repository,
            folder_repository,
            resource_repository,
            device_repository,
            store_repository,
            vector_clock_repository,
            user_repository,
            db,
        }
    }

    // A common utility method used across different modules
    pub async fn get_user_other_devices(
        &self,
        current_device_id: &str,
        user_id: &str,
    ) -> Result<Vec<Device>, RepositoryError> {
        let excluded_devices = vec![current_device_id.to_string()];
        self.device_repository
            .get_devices_by_user_except(user_id, &excluded_devices)
            .await
    }
}
