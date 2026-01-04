// Internal service modules - Butler calls these directly
pub mod space_service;
pub mod app_service;
pub(crate) mod node_service;
pub(crate) mod contact_service;
pub mod auth_service;

// Auth functions are standalone (don't need Butler instance)
pub use auth_service::{signup, login, is_signed_up, recover, change_passphrase, SignupResult, get_identity_data};
