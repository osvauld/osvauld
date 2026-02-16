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

// Shell and TabInfo are automatically exported by include_modules!()
// Use sthalam_shell::Shell and sthalam_shell::TabInfo to access them

pub mod callbacks;
pub mod utils;

// Re-export callbacks
pub use callbacks::*;

// Re-export utils
pub use utils::*;

/// Callback type for when user selects an app to open
/// Parameters: (page_id, app_name, renderer_type)
pub type OnSelectApp = Box<dyn Fn(&str, &str, &str) + Send + 'static>;
