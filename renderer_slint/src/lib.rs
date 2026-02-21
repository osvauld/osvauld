//! Slint Renderer for User Apps.
//!
//! Renders UI apps (chat, survey, ecomm) using Slint interpreter.

pub mod asset_image;
mod launch;
mod page_runtime;
mod prepare;
mod slint_model_bindings;
mod slint_runtime;
mod types;
mod validation;
mod value_convert;

pub use launch::{create_slint_app, handle_asset_pick, launch_slint_app};
pub use page_runtime::{
    generate_page_shell, parse_exported_types, write_shell_slint, AppTab, ExportedTypes,
};
pub use prepare::{get_app_files_from_scribe, get_app_manifest, prepare_page};
pub use slint_model_bindings::LuaSlintModel;
pub use slint_runtime::{AssetPickRequest, SlintRuntime};
pub use types::{
    AppStatus, AppVersion, DebugEvalRequest, LaunchedApp, Manifest, PreparedPage, RunningSlintApp,
    WindowGeometry,
};
pub use validation::validate_slint_files;

/// Extract a human-readable message from a panic payload.
fn extract_panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
