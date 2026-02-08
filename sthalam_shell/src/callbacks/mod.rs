//! Shell callback registration modules
//!
//! Each module provides a `register()` function to wire up shell callbacks.
//! Note: App rendering is NOT handled here - that's in the main binary using renderer_slint/renderer_raylib.

pub mod auth;
pub mod nodes;
pub mod pages;
pub mod publish;
pub mod spaces;
pub mod viewer;
