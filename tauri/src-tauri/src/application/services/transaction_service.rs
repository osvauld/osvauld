use crate::domains::repositories::{
    RepositoryError, ResourceKeyRepository, ResourceRepository, ShareRepository, SyncRepository,
};

use crate::domains::models::resource::{Resource, ResourceWithKey};
use crate::domains::models::resource_key::ResourceKey;
use crate::domains::models::share_record::ShareRecordSet;
use crate::domains::models::sync_record::SyncRecordSet;
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
        // This could use a database transaction if supported
        // For now, we'll do separate writes

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
}
