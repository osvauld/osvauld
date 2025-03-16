// Add this to your types.rs file
use osvauld_core::models::device::Device;
use osvauld_core::models::user::User;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct CurrentUserState {
    pub user: Option<User>,
    pub device: Option<Device>,
}

pub struct UserState {
    pub current_user: Arc<RwLock<CurrentUserState>>,
}

impl UserState {
    pub fn new() -> Self {
        Self {
            current_user: Arc::new(RwLock::new(CurrentUserState::default())),
        }
    }
    pub async fn get_user(&self) -> Option<User> {
        let guard = self.current_user.read().await;
        guard.user.clone()
    }

    // Get the current device, if any
    pub async fn get_device(&self) -> Option<Device> {
        let guard = self.current_user.read().await;
        guard.device.clone()
    }
}
