use tokio::sync::Mutex;

use crate::domains::models::share_record::{ShareRecord, ShareRecordSet};
use crate::domains::models::share_types::ShareOperation;
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
}
