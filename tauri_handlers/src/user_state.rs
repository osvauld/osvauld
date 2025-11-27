//! UserState - Current user session state
//!
//! Stores the logged-in user's identity for use in handlers.

use herald::Identity;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Minimal user info needed for P2P operations
#[derive(Clone, Debug)]
pub struct UserInfo {
    pub did: String,
    pub username: String,
    pub public_key: Vec<u8>,
}

#[derive(Clone)]
pub struct CurrentUserState {
    /// Herald identity (signing + encryption keys)
    pub identity: Option<Identity>,
    /// Cached user info
    pub user_info: Option<UserInfo>,
}

impl Default for CurrentUserState {
    fn default() -> Self {
        Self {
            identity: None,
            user_info: None,
        }
    }
}

#[derive(Clone)]
pub struct UserState {
    pub current_user: Arc<RwLock<CurrentUserState>>,
}

impl UserState {
    pub fn new() -> Self {
        Self {
            current_user: Arc::new(RwLock::new(CurrentUserState::default())),
        }
    }

    pub async fn get_user(&self) -> Result<UserInfo, String> {
        let state = self.current_user.read().await;
        state
            .user_info
            .clone()
            .ok_or_else(|| "No user loaded".to_string())
    }

    pub async fn get_identity(&self) -> Result<Identity, String> {
        let state = self.current_user.read().await;
        state
            .identity
            .clone()
            .ok_or_else(|| "No identity loaded".to_string())
    }

    pub async fn set_identity(&self, identity: Identity, username: String) {
        let mut state = self.current_user.write().await;
        let user_info = UserInfo {
            did: identity.did().to_string(),
            username,
            public_key: identity.public_signing_key().to_vec(),
        };
        state.identity = Some(identity);
        state.user_info = Some(user_info);
    }

    pub async fn clear(&self) {
        let mut state = self.current_user.write().await;
        state.identity = None;
        state.user_info = None;
    }

    pub async fn is_logged_in(&self) -> bool {
        let state = self.current_user.read().await;
        state.identity.is_some()
    }
}

impl Default for UserState {
    fn default() -> Self {
        Self::new()
    }
}
