//! Sthalam Shell - Slint-based UI shell
//!
//! This crate provides the main window shell for Sthalam, including:
//! - Tab management
//! - Authentication screens (initiation, login, password setup)
//! - App content area where user apps are displayed

slint::include_modules!();

// Shell and TabInfo are automatically exported by include_modules!()
// Use slint_shell::Shell and slint_shell::TabInfo to access them

// Re-export modules for main.rs
pub mod app_runner;
pub mod callbacks;
pub mod debug_logger;
pub mod debug_server;
pub mod events;
pub mod setup;
pub mod ui_automation;
pub mod utils;
