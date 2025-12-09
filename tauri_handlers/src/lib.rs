// Shared Tauri command handlers for osvauld projects
// Can be used directly in tauri::generate_handler![] across sthalam, livnote, libremot, etc.
//
// All handlers use Butler as the single entry point for storage and identity.

pub mod config;
pub mod events;
pub mod handlers;
pub mod types;

// Re-export commonly used items
pub use config::HandlerConfig;
pub use handlers::p2p::P2PState;
