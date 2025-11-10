// Core modules
mod aes_utils;
mod crypto_core;
mod pgp_utils;

// Public API modules
pub mod crypto_utils;
pub mod data_encryption;
pub mod errors;
pub mod key_management;
pub mod signature_operations;
pub mod signature_utils;
pub mod types;
pub mod ucan_extractors;
pub mod ucan_utils;

// Re-export the main public API
pub use crypto_utils::CryptoUtils;

// Re-export UCAN type for services to use directly
pub use ucan::Ucan;
pub use data_encryption::{decrypt_with_aes, encrypt_data_for_user};
pub use key_management::{
    change_certificate_password, derive_node_id_from_public_key, encrypt_string_with_public_key,
    export_certificate, generate_and_encrypt_ed25519_key, generate_keys,
    generate_keys_without_password, get_key_id, import_certificate,
};
pub use signature_operations::{verify_clear_text_message, verify_signature};
