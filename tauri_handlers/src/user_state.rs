use osvauld_core::models::{Device, User};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct CurrentUserState {
    pub user: Option<User>,
    pub device: Option<Device>,
}

impl Default for CurrentUserState {
    fn default() -> Self {
        Self {
            user: None,
            device: None,
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

    pub async fn get_user(&self) -> Result<User, String> {
        let state = self.current_user.read().await;
        state
            .user
            .clone()
            .ok_or_else(|| "No user loaded".to_string())
    }

    pub async fn get_device(&self) -> Result<Device, String> {
        let state = self.current_user.read().await;
        state
            .device
            .clone()
            .ok_or_else(|| "No device loaded".to_string())
    }
}

impl Default for UserState {
    fn default() -> Self {
        Self::new()
    }
}
