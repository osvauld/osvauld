use tokio::sync::Mutex;

use crypto_utils::{CryptoUtils, get_key_id};
use osvauld_core::models::p2p::SharePayload;
use osvauld_core::models::share_record::{
    SharePayloadResult, ShareRecord, ShareRecordSet, UserRecordSet,
};
use osvauld_core::models::share_types::{ShareOperation, ShareStatus};
use osvauld_core::repositories::{
    RepositoryError, ResourceRepository, ShareRepository, StoreRepository, UserRepository,
};
use std::sync::Arc;
pub struct ShareService {
    share_repository: Arc<dyn ShareRepository>,
    user_repository: Arc<dyn UserRepository>,
    crypto_utils: Arc<Mutex<CryptoUtils>>,
    resource_repo: Arc<dyn ResourceRepository>,
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
        user_repository: Arc<dyn UserRepository>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
        resource_repo: Arc<dyn ResourceRepository>,
    ) -> Self {
        Self {
            share_repository,
            user_repository,
            crypto_utils,
            resource_repo,
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
    pub async fn get_pending_shares(
        &self,
        user_id: &str,
    ) -> Result<Option<SharePayload>, ShareServiceError> {
        // Try to get pending share records first
        let pending_share = self
            .share_repository
            .get_pending_share_by_user(user_id)
            .await
            .map_err(ShareServiceError::RepositoryError)?;

        if let Some((share_record, user_records, user_record_statuses)) = pending_share {
            // Only process "share" operations (not revoke or update operations)
            if share_record.operation_type == ShareOperation::Share {
                // Get the associated resource and key for this user
                let resource_key_pair = self
                    .resource_repo
                    .find_resource_with_key(&share_record.resource_id, user_id)
                    .await
                    .map_err(ShareServiceError::RepositoryError)?;

                // Create the payload with resource data
                return Ok(Some(SharePayload {
                    share_record: Some(share_record),
                    user_records,
                    user_record_statuses,
                    data: Some(resource_key_pair),
                }));
            }

            // For other operation types, just return the records without the resource
            return Ok(Some(SharePayload {
                share_record: Some(share_record),
                user_records,
                user_record_statuses,
                data: None,
            }));
        }

        // Check for any unsynced user records if there are no pending share records
        let unsynced_records = self
            .share_repository
            .get_unsynced_user_share_records(user_id)
            .await
            .map_err(ShareServiceError::RepositoryError)?;

        if !unsynced_records.is_empty() {
            // Collect all user records and their statuses
            let mut all_user_records = Vec::new();
            let mut all_statuses = Vec::new();

            for (user_record, statuses) in unsynced_records {
                all_user_records.push(user_record);
                all_statuses.extend(statuses);
            }

            return Ok(Some(SharePayload {
                share_record: None,
                user_records: all_user_records,
                user_record_statuses: all_statuses,
                data: None,
            }));
        }

        // No pending shares or unsynced records found
        Ok(None)
    }

    pub async fn process_incoming_payload(
        &self,
        payload: SharePayload,
    ) -> Result<SharePayloadResult, String> {
        let current_user_id = self.get_current_user_id().await?;
        let mut result = SharePayloadResult {
            data: None,
            user_records: vec![],
            user_record_statuses: vec![],
            share_record: None,
        };

        if let Some(share_record) = &payload.share_record {
            //TODO: handle case where share record only and no resource_pair
            if let Some(resource_pair) = &payload.data {
                let resource_exists = match self
                    .resource_repo
                    .find_by_id(&resource_pair.resource.id, &current_user_id)
                    .await
                {
                    Ok(_) => true,
                    Err(RepositoryError::NotFound) => {
                        let (updated_user_records, updated_statuses) =
                            ShareRecord::process_payload_for_new_resource(
                                &payload.user_records,
                                &payload.user_record_statuses,
                                &current_user_id,
                            );
                        result.data = payload.data.clone();
                        result.share_record = payload.share_record.clone();
                        result.user_records = updated_user_records;
                        result.user_record_statuses = updated_statuses;
                        return Ok(result);
                    }
                    Err(e) => return Err(format!("Error checking resource: {}", e)),
                };
                if resource_exists {
                    let share_record_exists = match self
                        .share_repository
                        .get_share_record_by_id(&share_record.id)
                        .await
                    {
                        Ok(_) => true,
                        Err(RepositoryError::NotFound) => {
                            let (updated_user_records, updated_statuses) =
                                ShareRecord::process_payload_for_new_resource(
                                    &payload.user_records,
                                    &payload.user_record_statuses,
                                    &current_user_id,
                                );
                            result.share_record = payload.share_record.clone();
                            result.user_records = updated_user_records;
                            result.user_record_statuses = updated_statuses;
                            result.data = Some(resource_pair.clone());
                            return Ok(result);
                        }
                        Err(e) => return Err(format!("Error checking resource: {}", e)),
                    };
                }
            }
        }
        todo!()
    }
}
