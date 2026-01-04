//! App Service - Import and manage app bundles
//!
//! Provides functions for importing app bundles from filesystem to Butler storage.

use crate::{Butler, Page};
use crate::error::{ButlerError, Result};
use std::path::Path;
use walkdir::WalkDir;

/// Extract layer names from permit template JSON
///
/// **Context**: Permit templates define layers in `owner_template.layers`
/// **We extract**: Layer names to create CRDT storage during page creation
fn extract_layer_names_from_permit(permit_json: &str) -> Result<Vec<String>> {
    let permit: serde_json::Value = serde_json::from_str(permit_json)
        .map_err(|e| ButlerError::Serialization(
            format!("Failed to parse permit template: {}", e)
        ))?;

    let layers = permit.get("owner_template")
        .and_then(|t| t.get("layers"))
        .and_then(|l| l.as_object())
        .ok_or_else(|| ButlerError::Permit(
            "Permit template missing owner_template.layers".to_string()
        ))?;

    Ok(layers.keys().map(|k| k.to_string()).collect())
}

/// Import an app from a directory into Butler storage
///
/// **Context**: User wants to import a development app into Butler
/// **We read**: All app files from directory (manifest, .slint, .lua, .json)
/// **We create**: New page in space with permit template from app
/// **We store**: All files as encrypted layers via save_page_file()
/// **We return**: Created page metadata
///
/// # File Discovery
/// Walks directory recursively and imports:
/// - manifest.json (required)
/// - All .slint files (UI definitions)
/// - All .lua files (logic scripts)
/// - All .json files (config, permit templates)
///
/// # Arguments
/// * `butler` - Butler instance for storage operations
/// * `space_id` - Target space ID for the app/page
/// * `app_dir` - Directory containing app files
///
/// # Errors
/// Returns error if:
/// - manifest.json not found or invalid
/// - permit_template.json not found
/// - Page creation fails
/// - File storage fails (partial state possible - retry with update_app)
pub async fn import_app_from_directory(
    butler: &Butler,
    space_id: &str,
    app_dir: &Path,
) -> Result<Page> {
    // 1. Read and parse manifest
    let manifest_path = app_dir.join("manifest.json");
    let manifest_content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| ButlerError::Storage(
            format!("Failed to read manifest.json: {}", e)
        ))?;

    let manifest: serde_json::Value = serde_json::from_str(&manifest_content)
        .map_err(|e| ButlerError::Serialization(
            format!("Invalid manifest.json: {}", e)
        ))?;

    let app_name = manifest.get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| ButlerError::Storage(
            "manifest.json missing 'name' field".to_string()
        ))?;

    // 2. Read permit template
    let template_filename = manifest.get("permit_template")
        .and_then(|p| p.as_str())
        .unwrap_or("permit_template.json");

    let template_path = app_dir.join(template_filename);
    let permit_template = std::fs::read_to_string(&template_path)
        .map_err(|e| ButlerError::Storage(
            format!("Failed to read {}: {}", template_filename, e)
        ))?;

    // 3. Collect all app files to import
    let mut files_to_store: Vec<(String, String)> = Vec::new();

    // Always include manifest
    files_to_store.push(("manifest.json".to_string(), manifest_content));

    // Walk directory for app files
    for entry in WalkDir::new(app_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !["slint", "lua", "json"].contains(&ext) {
            continue;
        }

        if let Ok(relative_path) = path.strip_prefix(app_dir) {
            let relative_str = relative_path.to_string_lossy().to_string();

            // Skip manifest (already added)
            if relative_str == "manifest.json" {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(path) {
                files_to_store.push((relative_str, content));
            }
        }
    }

    tracing::info!(
        app_name = %app_name,
        file_count = files_to_store.len(),
        space_id = %space_id,
        "Importing app to Butler storage"
    );

    // 4. Extract layer names from permit template
    let layer_names = extract_layer_names_from_permit(&permit_template)?;

    tracing::debug!(
        layer_count = layer_names.len(),
        layers = ?layer_names,
        "Extracted layers from permit template"
    );

    // 5. Create page with CRDT layers
    let page = butler.create_page(
        space_id,
        app_name,
        layer_names,
        &permit_template,
    ).await?;

    tracing::info!(page_id = %page.id, page_name = %page.name, "Created page for app");

    // 6. Store all files as encrypted layers
    for (file_path, content) in &files_to_store {
        butler.save_page_file(&page.id, file_path, content).await?;
        tracing::debug!(file_path = %file_path, "Stored app file");
    }

    tracing::info!(
        page_id = %page.id,
        app_name = %app_name,
        "Successfully imported app"
    );

    Ok(page)
}

/// Update an existing app by overwriting its files
///
/// **Context**: User wants to update an app with new version from directory
/// **We do**: Same as import but targeting existing page_id
/// **Warning**: Overwrites all files - no merge logic
///
/// # Arguments
/// * `butler` - Butler instance
/// * `page_id` - Existing page ID to update
/// * `app_dir` - Directory with new app files
pub async fn update_app_from_directory(
    butler: &Butler,
    page_id: &str,
    app_dir: &Path,
) -> Result<()> {
    // Verify page exists
    butler.get_page(page_id)?
        .ok_or_else(|| ButlerError::NotFound(
            format!("Page {} not found", page_id)
        ))?;

    // 1. Read manifest (required)
    let manifest_path = app_dir.join("manifest.json");
    let manifest_content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| ButlerError::Storage(
            format!("Failed to read manifest.json: {}", e)
        ))?;

    // 2. Collect all files
    let mut files_to_store: Vec<(String, String)> = Vec::new();
    files_to_store.push(("manifest.json".to_string(), manifest_content));

    for entry in WalkDir::new(app_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !["slint", "lua", "json"].contains(&ext) {
            continue;
        }

        if let Ok(relative_path) = path.strip_prefix(app_dir) {
            let relative_str = relative_path.to_string_lossy().to_string();
            if relative_str == "manifest.json" {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(path) {
                files_to_store.push((relative_str, content));
            }
        }
    }

    tracing::info!(
        page_id = %page_id,
        file_count = files_to_store.len(),
        "Updating app files"
    );

    // 3. Overwrite all files
    for (file_path, content) in &files_to_store {
        butler.save_page_file(page_id, file_path, content).await?;
        tracing::debug!(file_path = %file_path, "Updated app file");
    }

    tracing::info!(page_id = %page_id, "Successfully updated app");

    Ok(())
}
