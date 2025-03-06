use tokio::sync::Mutex;

use crate::domains::models::share_record::{self, ShareRecord, ShareRecordSet, UserRecordSet};
use crate::domains::models::share_types::{ShareOperation, ShareStatus};
use crate::domains::repositories::{
    RepositoryError, ShareRepository, StoreRepository, UserRepository,
};
use crypto_utils::{get_key_id, CryptoUtils};
use std::sync::Arc;
pub struct ShareService {
    share_repository: Arc<dyn ShareRepository>,
    store_repository: Arc<dyn StoreRepository>,
    user_repository: Arc<dyn UserRepository>,
    crypto_utils: Arc<Mutex<CryptoUtils>>,
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShareServiceError {
    #[error("Repository error: {0}")]
    RepositoryError(#[from] RepositoryError),
    #[error("Crypto error: {0}")]
    CryptoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

impl ShareService {
    pub fn new(
        share_repository: Arc<dyn ShareRepository>,
        store_repository: Arc<dyn StoreRepository>,
        user_repository: Arc<dyn UserRepository>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
    ) -> Self {
        Self {
            share_repository,
            store_repository,
            user_repository,
            crypto_utils,
        }
    }

    async fn get_current_user_id(&self) -> Result<String, String> {
        let public_key = {
            let crypto = self.crypto_utils.lock().await;
            crypto.get_public_key().map_err(|e| e.to_string())?
        };

        get_key_id(&public_key).map_err(|e| e.to_string())
    }

    pub async fn prepare_owner_share_record(
        &self,
        resource_id: String,
    ) -> Result<ShareRecordSet, ShareServiceError> {
        let current_user_id = self
            .get_current_user_id()
            .await
            .map_err(|e| ShareServiceError::CryptoError(e))?;

        let current_user = self
            .user_repository
            .get_user_by_id(&current_user_id)
            .await?;

        let share_record_set = ShareRecord::create_share_record(
            resource_id,
            current_user_id.clone(),
            &[current_user], // The owner is both the sharer and target
            ShareOperation::Share,
        );

        Ok(share_record_set)
    }
    pub async fn prepare_content_update_records(
        &self,
        resource_id: String,
    ) -> Result<Option<UserRecordSet>, ShareServiceError> {
        // Get the current user ID
        let current_user_id = self
            .get_current_user_id()
            .await
            .map_err(|e| ShareServiceError::CryptoError(e))?;

        // Find effective share records for this resource (only active shares, not revoked)
        let share_records = self
            .share_repository
            .get_effective_share_records(&resource_id)
            .await
            .map_err(ShareServiceError::RepositoryError)?;

        // If no effective share records exist, we're done
        if share_records.is_empty() {
            return Ok(None);
        }

        // Get the first share record - we only need one to work with
        let share_record = &share_records[0];

        // Get all user records for just this one share record
        let user_records = self
            .share_repository
            .get_user_records_by_share_id(&share_record.id)
            .await
            .map_err(ShareServiceError::RepositoryError)?;

        // If there's only one user record and it's for the current user, no updates needed
        if user_records.len() == 1 && user_records[0].user_id == current_user_id {
            return Ok(None);
        }

        // Process the user records to get all involved users
        let mut all_involved_users = Vec::new();
        let mut users_needing_updates = Vec::new();

        for user_record in &user_records {
            // Get the user associated with this record
            let user = self
                .user_repository
                .get_user_by_id(&user_record.user_id)
                .await
                .map_err(ShareServiceError::RepositoryError)?;

            // Add to the list of all involved users
            all_involved_users.push(user.clone());

            // If the user record is completed and not for the current user,
            // this user needs an update record
            if user_record.status == ShareStatus::Completed
                && user_record.user_id != current_user_id
            {
                users_needing_updates.push(user.clone());
            }
        }

        // If no users need updates, we're done
        if users_needing_updates.is_empty() {
            return Ok(None);
        }

        // Create the user record set for content update
        let user_record_set = ShareRecord::create_user_update_records(
            share_record.id.clone(),
            &users_needing_updates,
            current_user_id,
            &all_involved_users,
        );

        Ok(Some(user_record_set))
    }

    pub async fn prepare_share_records(
        &self,
        resource_id: String,
        recipient_public_key: String,
    ) -> Result<UserRecordSet, ShareServiceError> {
        let current_user_id = self
            .get_current_user_id()
            .await
            .map_err(|e| ShareServiceError::CryptoError(e))?;
        let recipient_id = get_key_id(&recipient_public_key)
            .map_err(|e| ShareServiceError::CryptoError(e.to_string()))?;
        let recipient_user = self.user_repository.get_user_by_id(&recipient_id).await?;
        let share_records = self
            .share_repository
            .get_effective_share_records(&resource_id)
            .await?;
        let share_record = &share_records[0];
        let existing_user_records = self
            .share_repository
            .get_user_records_by_share_id(&share_record.id)
            .await?;
        let user_record_set = ShareRecord::generate_share_user_records(
            recipient_user,
            current_user_id,
            &existing_user_records,
        );
        Ok(user_record_set)
    }
}
