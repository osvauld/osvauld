use crate::domains::models::user::User;
use crate::domains::repositories::{RepositoryError, UserRepository};
use crypto_utils::{get_key_id, CryptoUtils};
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
}
impl UserService {
    pub fn new(
        user_repository: Arc<dyn UserRepository>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
    ) -> Self {
        Self {
            user_repository,
            crypto_utils,
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
}
