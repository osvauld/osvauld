use osvauld_core::models::auth::Certificate;
use osvauld_core::models::device::Device;
use osvauld_core::models::folder::Folder;
use osvauld_core::models::user::User;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::{
    DeviceRepository, FolderRepository, RepositoryError, ResourceKeyRepository, ResourceRepository,
    ShareRepository, StoreRepository, SyncRepository, UserRepository, VectorClockRepository,
};

use osvauld_core::models::resource::Resource;
use osvauld_core::models::resource_key::ResourceKey;
use osvauld_core::models::share_record::{ShareRecordSet, UserRecordSet};
use osvauld_core::models::sync_record::{SyncRecordSet, SyncUpdateData};
use std::sync::Arc;
pub struct TransactionService {
    resource_repository: Arc<dyn ResourceRepository>,
    resource_key_repository: Arc<dyn ResourceKeyRepository>,
    sync_repository: Arc<dyn SyncRepository>,
    share_repository: Arc<dyn ShareRepository>,
    store_repository: Arc<dyn StoreRepository>,
    user_repository: Arc<dyn UserRepository>,
    device_repository: Arc<dyn DeviceRepository>,
    folder_repository: Arc<dyn FolderRepository>,
    vector_clock_repository: Arc<dyn VectorClockRepository>,
}

impl TransactionService {
    pub fn new(
        resource_repository: Arc<dyn ResourceRepository>,
        resource_key_repository: Arc<dyn ResourceKeyRepository>,
        sync_repository: Arc<dyn SyncRepository>,
        share_repository: Arc<dyn ShareRepository>,
        store_repository: Arc<dyn StoreRepository>,
        user_repository: Arc<dyn UserRepository>,
        device_repository: Arc<dyn DeviceRepository>,
        folder_repository: Arc<dyn FolderRepository>,
        vector_clock_repository: Arc<dyn VectorClockRepository>,
    ) -> Self {
        Self {
            resource_repository,
            resource_key_repository,
            sync_repository,
            share_repository,
            store_repository,
            user_repository,
            device_repository,
            folder_repository,
            vector_clock_repository,
        }
    }

    pub async fn create_resource_with_sync(
        &self,
        resource: Resource,
        resource_key: ResourceKey,
        sync_record_set: SyncRecordSet,
        share_record_set: ShareRecordSet,
        vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError> {
        // Save resource and its key
        self.resource_repository.save(&resource).await?;
        self.resource_key_repository.save(&resource_key).await?;

        // Save sync records
        self.sync_repository
            .add_sync_record_set(sync_record_set)
            .await?;

        // Save share records
        self.share_repository
            .add_share_record_set(share_record_set)
            .await?;
        self.vector_clock_repository
            .save_vector_clocks(vector_clocks)
            .await?;
        Ok(())
    }

    pub async fn update_resource_with_sync_and_share(
        &self,
        resource_id: &str,
        encrypted_data: String,
        current_device: &Device,
    ) -> Result<(), RepositoryError> {
        // Use diesel transaction if your database supports it
        // For SQLite, you might need to implement your own transaction mechanism

        // 1. Update the resource data
        self.resource_repository
            .update_resource(encrypted_data, resource_id)
            .await?;
        self.vector_clock_repository
            .increment_vector_clock(&resource_id, &current_device.id)
            .await?;

        Ok(())
    }

    pub async fn share_resource(
        &self,
        resource_key: ResourceKey,
        vector_clock: ResourceVectorClock,
        user_record: UserRecordSet,
        resource_id: String,
    ) -> Result<(), RepositoryError> {
        log::info!("vecoor {:?}", vector_clock);
        self.resource_key_repository.save(&resource_key).await?;
        self.share_repository
            .update_user_record_set(user_record)
            .await?;
        // Update the resource's vector clock
        // self.resource_repository
        //     .update_resource_vector_clock(&resource_id, &vector_clock)
        //     .await?;
        Ok(())
    }
    pub async fn handle_add_folder_transaction(
        &self,
        folder: &Folder,
        folder_sync_record_set: &SyncRecordSet,
    ) -> Result<(), RepositoryError> {
        // Store primary certificate
        self.folder_repository.save(folder).await?;
        self.sync_repository
            .add_sync_record_set(folder_sync_record_set.clone())
            .await?;

        Ok(())
    }

    pub async fn handle_sign_up_transaction(
        &self,
        user: &User,
        primary_certificate: &Certificate,
        device: &Device,
        device_certificate: &Certificate,
        sync_record_set: &SyncRecordSet,
    ) -> Result<(), RepositoryError> {
        // Store primary certificate

        self.user_repository.add_known_user(user.clone()).await?;
        self.store_repository
            .store_certificate(
                primary_certificate,
                "primary_key".to_string(),
                "primary_key_salt".to_string(),
            )
            .await?;

        // Store device certificate
        self.store_repository
            .store_certificate(
                device_certificate,
                "device_key".to_string(),
                "device_key_salt".to_string(),
            )
            .await?;

        // Store device key ID
        self.store_repository.store_device_key(&device.id).await?;

        // Save device information to repository
        self.device_repository.save(device.clone()).await?;

        // Save sync record set
        self.sync_repository
            .add_sync_record_set(sync_record_set.clone())
            .await?;
        Ok(())
    }
}
