use osvauld_core::models::vectorClock::VectorClock;
use osvauld_core::repositories::{
    RepositoryError, ResourceKeyRepository, ResourceRepository, ShareRepository, SyncRepository,
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
}

impl TransactionService {
    pub fn new(
        resource_repository: Arc<dyn ResourceRepository>,
        resource_key_repository: Arc<dyn ResourceKeyRepository>,
        sync_repository: Arc<dyn SyncRepository>,
        share_repository: Arc<dyn ShareRepository>,
    ) -> Self {
        Self {
            resource_repository,
            resource_key_repository,
            sync_repository,
            share_repository,
        }
    }

    pub async fn create_resource_with_sync(
        &self,
        resource: Resource,
        resource_key: ResourceKey,
        sync_record_set: SyncRecordSet,
        share_record_set: ShareRecordSet,
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

        Ok(())
    }

    pub async fn update_resource_with_sync_and_share(
        &self,
        resource_id: String,
        encrypted_data: String,
        sync_data: Option<SyncUpdateData>,
        share_data: Option<UserRecordSet>,
    ) -> Result<(), RepositoryError> {
        // Use diesel transaction if your database supports it
        // For SQLite, you might need to implement your own transaction mechanism

        // 1. Update the resource data
        self.resource_repository
            .update_resource(encrypted_data, resource_id)
            .await?;

        // 2. Handle sync records if present
        if let Some(sync_update) = sync_data {
            match sync_update {
                SyncUpdateData::FullSyncSet(sync_record_set) => {
                    // Write a full sync record set
                    self.sync_repository
                        .add_sync_record_set(sync_record_set)
                        .await?;
                }
                SyncUpdateData::DeviceRecordSet(device_record_set) => {
                    // Write only device record set
                    self.sync_repository
                        .update_device_record_set(device_record_set)
                        .await?;
                }
            }
        }

        // 3. Handle share records if present
        if let Some(user_record_set) = share_data {
            self.share_repository
                .update_user_record_set(user_record_set)
                .await?;
        }

        Ok(())
    }

    pub async fn share_resource(
        &self,
        resource_key: ResourceKey,
        vector_clock: VectorClock,
        user_record: UserRecordSet,
        resource_id: String,
    ) -> Result<(), RepositoryError> {
        log::info!("vecoor {:?}", vector_clock);
        self.resource_key_repository.save(&resource_key).await?;
        self.share_repository
            .update_user_record_set(user_record)
            .await?;
        // Update the resource's vector clock
        self.resource_repository
            .update_resource_vector_clock(&resource_id, &vector_clock)
            .await?;
        Ok(())
    }
}
