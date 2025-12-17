//! Tauri HUML Plugin
//!
//! Enables native GPU-rendered HUML template windows in Tauri applications.
//! Uses winit for window creation to avoid Wayland conflicts with GTK/GDK.
//!
//! **Architecture:**
//! - Each HUML window runs in its own thread with its own winit event loop
//! - Vello renders directly to winit surfaces (GPU-accelerated)
//! - Data changes trigger reactive re-rendering (no immediate mode overhead)
//!
//! **Usage:**
//! ```ignore
//! // In Tauri setup
//! tauri_huml_plugin::init(&app);
//!
//! // Later, to create a HUML window
//! use tauri_huml_plugin::AppHandleExt;
//! app.create_huml_window(
//!     "viewer",
//!     "My Template",
//!     800,
//!     600,
//!     template,       // ParsedTemplate
//!     cel_evaluator,  // Arc<CelEvaluator>
//! )?;
//!
//! // Update data in the window
//! app.send_huml_data("viewer", "count", serde_json::json!(42))?;
//!
//! // Navigate to a different screen
//! app.navigate_huml_screen("viewer", "settings")?;
//! ```

mod plugin;

pub use plugin::{
    init, create_window, close_window, send_data, navigate_screen,
    AppHandleExt, HumlPluginState, SharedState, WindowCommand,
};

// Re-export key types
pub use huml_renderer::{ParsedTemplate, HumlRenderer};
