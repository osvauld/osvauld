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

    // Get the current user, returning an error if not present
    pub async fn get_user(&self) -> Result<User, String> {
        let guard = self.current_user.read().await;
        guard
            .user
            .clone()
            .ok_or_else(|| "No user found in state. Please log in.".to_string())
    }

    // Get the current device, returning an error if not present
    pub async fn get_device(&self) -> Result<Device, String> {
        let guard = self.current_user.read().await;
        guard
            .device
            .clone()
            .ok_or_else(|| "No device found in state. Please register a device first.".to_string())
    }
}
