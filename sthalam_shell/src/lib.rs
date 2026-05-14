//! Sthalam Shell - Platform Shell UI
//!
//! Provides the main window shell for Sthalam, including:
//! - Authentication screens (initiation, login, password setup)
//! - Space management
//! - Page management
//! - Node management
//!
//! This crate does NOT contain app rendering - that's in renderer_slint/renderer_raylib.
//!
//! ## Architecture
//!
//! - Uses compiled Slint (slint-build, not slint-interpreter)
//! - Contains only shell callbacks
//! - App rendering is delegated to renderer crates

slint::include_modules!();

pub mod callbacks;
pub mod utils;

pub use callbacks::*;
pub use utils::*;

/// Callback type for when user selects an app to open
/// Parameters: (page_id, app_name, renderer_type)
pub type OnSelectApp = Box<dyn Fn(&str, &str, &str) + Send + 'static>;
