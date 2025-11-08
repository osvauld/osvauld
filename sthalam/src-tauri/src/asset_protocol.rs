use log::{error, info};
use std::path::PathBuf;
use tauri::{Manager, UriSchemeContext, UriSchemeResponder};

/// Handle asset:// protocol requests for WASM and other static files
///
/// In dev mode: serves from frontend/desktop/public/
/// In production: serves from Tauri's resource directory (bundled with app)
pub fn handle_asset_request<R: tauri::Runtime>(
    app: UriSchemeContext<R>,
    request: tauri::http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let uri = request.uri();
    let path = uri.path();

    // Remove leading slash
    let path_str = if path.starts_with('/') {
        path[1..].to_string()
    } else {
        path.to_string()
    };

    let app_handle = app.app_handle().clone();

    tauri::async_runtime::spawn(async move {
        let file_path = resolve_asset_path(&app_handle, &path_str);

        match file_path {
            Some(path) => {
                info!("[Asset Protocol] Resolved path: {}", path.display());

                match std::fs::read(&path) {
                    Ok(content) => {
                        let mime_type = get_mime_type(&path_str);

                        info!(
                            "[Asset Protocol] ✓ Serving {} ({} bytes, MIME: {})",
                            path_str,
                            content.len(),
                            mime_type
                        );

                        let response = tauri::http::Response::builder()
                            .status(200)
                            .header("Content-Type", mime_type)
                            .header("Access-Control-Allow-Origin", "*")
                            .header("Access-Control-Allow-Methods", "GET, OPTIONS")
                            .header("Access-Control-Allow-Headers", "*")
                            .body(content)
                            .unwrap();

                        responder.respond(response);
                    }
                    Err(e) => {
                        error!(
                            "[Asset Protocol] ✗ Failed to read file: {} - {}",
                            path.display(),
                            e
                        );
                        send_404(responder);
                    }
                }
            }
            None => {
                error!(
                    "[Asset Protocol] ✗ Could not resolve path for: {}",
                    path_str
                );
                send_404(responder);
            }
        }
    });
}

/// Resolve asset path based on dev/production mode
fn resolve_asset_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    path_str: &str,
) -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        // Dev mode: serve from public directory
        let current_dir = std::env::current_dir().ok()?;
        let workspace = current_dir.parent().unwrap_or(&current_dir);
        let dev_path = workspace.join("frontend/desktop/public").join(path_str);

        info!(
            "[Asset Protocol] Dev mode - workspace: {}, path: {}",
            workspace.display(),
            dev_path.display()
        );

        Some(dev_path)
    } else {
        // Production: serve from Tauri resource directory
        let resource_dir = app.path().resource_dir().ok()?;

        // Try direct path first
        let prod_path = resource_dir.join(path_str);

        if prod_path.exists() {
            info!(
                "[Asset Protocol] Production mode - found at: {}",
                prod_path.display()
            );
            return Some(prod_path);
        }

        // Try alternate path with _up_/frontend/desktop/dist/ prefix
        // (Tauri preserves ../ in resource paths)
        let alt_path = resource_dir
            .join("_up_/frontend/desktop/dist")
            .join(path_str);

        if alt_path.exists() {
            info!(
                "[Asset Protocol] Production mode - found at alternate path: {}",
                alt_path.display()
            );
            return Some(alt_path);
        }

        // File not found - log for debugging
        error!("[Asset Protocol] File not found in resource dir!");
        error!("[Asset Protocol] Tried paths:");
        error!("  1. {}", prod_path.display());
        error!("  2. {}", alt_path.display());

        // List what's actually in the resource directory
        if let Ok(entries) = std::fs::read_dir(&resource_dir) {
            error!("[Asset Protocol] Resource directory contents:");
            for entry in entries.flatten() {
                error!("  - {}", entry.path().display());
            }
        }

        None
    }
}

/// Get MIME type for file
fn get_mime_type(path: &str) -> String {
    if path.ends_with(".wasm") {
        "application/wasm".to_string()
    } else if path.ends_with(".js") {
        "application/javascript".to_string()
    } else {
        mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string()
    }
}

/// Send 404 response
fn send_404(responder: UriSchemeResponder) {
    let response = tauri::http::Response::builder()
        .status(404)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, OPTIONS")
        .header("Access-Control-Allow-Headers", "*")
        .body(Vec::new())
        .unwrap();

    responder.respond(response);
}
