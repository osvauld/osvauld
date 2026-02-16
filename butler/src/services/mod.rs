// Internal service modules - Butler calls these directly
pub mod app_service;
pub mod asset_service;
pub mod auth_service;
pub(crate) mod contact_service;
pub(crate) mod node_service;
pub mod page_service;
pub mod publish_service;
pub mod space_service;

// Auth functions are standalone (don't need Butler instance)
pub use auth_service::{
    change_passphrase, get_identity_data, is_signed_up, login, recover, signup, SignupResult,
};
