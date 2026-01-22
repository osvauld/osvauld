//! App Service - Import and manage app bundles
//!
//! Provides functions for importing app bundles from filesystem to Butler storage.
//!
//! ## Layer Naming Convention
//! - `app:{name}` - App layers (LoroMap of file paths -> content)
//! - `static:{path}` - Static binary files (not CRDT)
//! - No prefix - Data layers (LoroList/LoroMap from permit template)

use crate::{Butler, Page};
use crate::error::{ButlerError, Result};
use crate::models::Layer;
use std::path::Path;
use std::collections::HashMap;
use walkdir::WalkDir;

/// Prefix for app layers
pub const APP_LAYER_PREFIX: &str = "app:";

/// Prefix for static file layers
pub const STATIC_LAYER_PREFIX: &str = "static:";

/// Check if a layer name is an app layer
pub fn is_app_layer(layer_name: &str) -> bool {
    layer_name.starts_with(APP_LAYER_PREFIX)
}

/// Check if a layer name is a static file layer
pub fn is_static_layer(layer_name: &str) -> bool {
    layer_name.starts_with(STATIC_LAYER_PREFIX)
}

/// Check if a layer name is a data layer (not app, not static)
pub fn is_data_layer(layer_name: &str) -> bool {
    !is_app_layer(layer_name) && !is_static_layer(layer_name)
}

/// Get the app name from an app layer name
pub fn app_name_from_layer(layer_name: &str) -> Option<&str> {
    layer_name.strip_prefix(APP_LAYER_PREFIX)
}

/// Create an app layer name from an app name
pub fn app_layer_name(app_name: &str) -> String {
    format!("{}{}", APP_LAYER_PREFIX, app_name)
}

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

/// Collect app files from a directory
///
/// Returns a HashMap of relative_path -> content for text files.
fn collect_app_files(app_dir: &Path) -> Result<HashMap<String, String>> {
    let mut files = HashMap::new();

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
            if let Ok(content) = std::fs::read_to_string(path) {
                files.insert(relative_str, content);
            }
        }
    }

    Ok(files)
}

/// Import an app from a directory into Butler storage
///
/// **Context**: User wants to import a development app into Butler
/// **We read**: All app files from directory (manifest, .slint, .lua, .json)
/// **We create**: New page in space with:
///   - Data layers from permit template
///   - App layer `app:{app_name}` as LoroMap of files
/// **We return**: Created page metadata
///
/// # Layer Storage
/// - App files stored in `app:{app_name}` layer as LoroMap (CRDT, syncable)
/// - Data layers from permit template (CRDT, syncable)
///
/// # Arguments
/// * `butler` - Butler instance for storage operations
/// * `space_id` - Target space ID for the app/page
/// * `app_dir` - Directory containing app files
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

    // 3. Collect all app files
    let files = collect_app_files(app_dir)?;

    tracing::info!(
        app_name = %app_name,
        file_count = files.len(),
        space_id = %space_id,
        "Importing app to Butler storage"
    );

    // 4. Extract data layer names from permit template
    let mut layer_names = extract_layer_names_from_permit(&permit_template)?;

    // 5. Add app layer to the list
    let app_layer_name = app_layer_name(app_name);
    layer_names.push(app_layer_name.clone());

    tracing::debug!(
        data_layer_count = layer_names.len() - 1,
        app_layer = %app_layer_name,
        "Creating page with layers"
    );

    // 6. Create page with all layers (data + app)
    let page = butler.create_page(
        space_id,
        app_name,
        layer_names,
        &permit_template,
    ).await?;

    tracing::info!(page_id = %page.id, page_name = %page.name, "Created page for app");

    // 7. Populate app layer with files
    // Get AES key by decrypting page
    let (_decrypted, aes_key) = butler.get_decrypted_page(&page.id).await?;

    // Create app layer with files
    let app_layer = Layer::new();
    app_layer.set_all_files(&files)?;

    // Export, encrypt, and store
    let snapshot = app_layer.export_snapshot();
    let encrypted = herald::encrypt_symmetric(&aes_key, &snapshot)
        .map_err(|e| ButlerError::Encryption(e.to_string()))?;

    butler.store().put_layer(&page.id, &app_layer_name, &encrypted)?;

    tracing::info!(
        page_id = %page.id,
        app_name = %app_name,
        file_count = files.len(),
        "Successfully imported app with files in CRDT layer"
    );

    Ok(page)
}

/// Refresh an app from directory (re-read files and update layer)
///
/// **Context**: Developer edited files on disk, wants to reload into Butler
/// **We do**: Re-read files, update the app layer, return changed files
///
/// # Arguments
/// * `butler` - Butler instance
/// * `page_id` - Page containing the app
/// * `app_name` - Name of the app (used for layer name `app:{app_name}`)
/// * `app_dir` - Directory with updated app files
///
/// # Returns
/// List of file paths that were changed
pub async fn refresh_app_from_directory(
    butler: &Butler,
    page_id: &str,
    app_name: &str,
    app_dir: &Path,
) -> Result<Vec<String>> {
    // Verify page exists
    butler.get_page(page_id)?
        .ok_or_else(|| ButlerError::NotFound(
            format!("Page {} not found", page_id)
        ))?;

    // Collect new files from disk
    let new_files = collect_app_files(app_dir)?;

    // Get AES key and current layer
    let (decrypted, aes_key) = butler.get_decrypted_page(page_id).await?;

    let app_layer_name = app_layer_name(app_name);

    // Load existing app layer or create new
    let app_layer = if let Some(layer_bytes) = decrypted.docs.get(&app_layer_name) {
        if layer_bytes.is_empty() {
            Layer::new()
        } else {
            Layer::from_snapshot(layer_bytes)
                .unwrap_or_else(|_| Layer::new())
        }
    } else {
        Layer::new()
    };

    // Get current files to compare
    let old_files = app_layer.get_all_files();

    // Find changed files
    let mut changed_files = Vec::new();
    for (path, content) in &new_files {
        if old_files.get(path) != Some(content) {
            changed_files.push(path.clone());
        }
    }
    // Also track deleted files
    for path in old_files.keys() {
        if !new_files.contains_key(path) {
            changed_files.push(path.clone());
        }
    }

    if changed_files.is_empty() {
        tracing::info!(page_id = %page_id, app_name = %app_name, "No changes detected");
        return Ok(changed_files);
    }

    // Update layer with new files
    app_layer.set_all_files(&new_files)?;

    // Export, encrypt, and store
    let snapshot = app_layer.export_snapshot();
    let encrypted = herald::encrypt_symmetric(&aes_key, &snapshot)
        .map_err(|e| ButlerError::Encryption(e.to_string()))?;

    butler.store().put_layer(page_id, &app_layer_name, &encrypted)?;

    tracing::info!(
        page_id = %page_id,
        app_name = %app_name,
        changed_count = changed_files.len(),
        files = ?changed_files,
        "Refreshed app layer"
    );

    Ok(changed_files)
}

/// List all apps in a page
///
/// **Context**: Find all `app:*` layers in a page
/// **Returns**: List of app names (without prefix)
pub fn list_apps_in_page(butler: &Butler, page_id: &str) -> Result<Vec<String>> {
    let (decrypted, _) = futures::executor::block_on(butler.get_decrypted_page(page_id))?;

    let apps: Vec<String> = decrypted.docs.keys()
        .filter_map(|name| app_name_from_layer(name).map(String::from))
        .collect();

    Ok(apps)
}

/// Add another app to an existing page
///
/// **Context**: Page already has data layers, adding another app UI
/// **We do**: Create new `app:{app_name}` layer with files
pub async fn add_app_to_page(
    butler: &Butler,
    page_id: &str,
    app_dir: &Path,
) -> Result<String> {
    // Verify page exists
    butler.get_page(page_id)?
        .ok_or_else(|| ButlerError::NotFound(
            format!("Page {} not found", page_id)
        ))?;

    // Read manifest to get app name
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

    let app_layer_name = app_layer_name(app_name);

    // Check if app layer already exists
    let (decrypted, aes_key) = butler.get_decrypted_page(page_id).await?;
    if decrypted.docs.contains_key(&app_layer_name) {
        return Err(ButlerError::Storage(
            format!("App '{}' already exists in page", app_name)
        ));
    }

    // Collect files
    let files = collect_app_files(app_dir)?;

    // Create app layer with files
    let app_layer = Layer::new();
    app_layer.set_all_files(&files)?;

    // Export, encrypt, and store
    let snapshot = app_layer.export_snapshot();
    let encrypted = herald::encrypt_symmetric(&aes_key, &snapshot)
        .map_err(|e| ButlerError::Encryption(e.to_string()))?;

    butler.store().put_layer(page_id, &app_layer_name, &encrypted)?;

    tracing::info!(
        page_id = %page_id,
        app_name = %app_name,
        file_count = files.len(),
        "Added app to page"
    );

    Ok(app_name.to_string())
}

// ============================================================================
// Page-level import (directory containing multiple apps)
// ============================================================================

/// Result of reloading a page from directory
#[derive(Debug, Default)]
pub struct PageReloadResult {
    /// Apps that were updated (files changed)
    pub updated_apps: Vec<String>,
    /// Apps that were newly added
    pub added_apps: Vec<String>,
    /// Apps that were removed (no longer in directory)
    pub removed_apps: Vec<String>,
}

/// Scan a page directory for app subdirectories
///
/// **Structure:**
/// ```text
/// page_dir/
///   ├── permit_template.json    ← page-level permit
///   ├── shop-owner/             ← app (has manifest.json)
///   │   └── manifest.json
///   └── shop-customer/          ← app (has manifest.json)
///       └── manifest.json
/// ```
///
/// **Returns:** Vec<(app_name, app_dir_path)>
fn scan_app_subdirectories(page_dir: &Path) -> Result<Vec<(String, std::path::PathBuf)>> {
    let mut apps = Vec::new();

    let entries = std::fs::read_dir(page_dir)
        .map_err(|e| ButlerError::Storage(format!("Failed to read page directory: {}", e)))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Check if this subdirectory has a manifest.json
        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        // Read manifest to get app name
        let manifest_content = std::fs::read_to_string(&manifest_path)
            .map_err(|e| ButlerError::Storage(format!("Failed to read manifest: {}", e)))?;

        let manifest: serde_json::Value = serde_json::from_str(&manifest_content)
            .map_err(|e| ButlerError::Serialization(format!("Invalid manifest: {}", e)))?;

        let app_name = manifest.get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| ButlerError::Storage(
                format!("manifest.json in {:?} missing 'name' field", path)
            ))?;

        apps.push((app_name.to_string(), path));
    }

    Ok(apps)
}

/// Import a page directory containing multiple apps
///
/// **Structure:**
/// ```text
/// page_dir/                     ← directory name = page name
///   ├── permit_template.json    ← page-level permit (data layers)
///   ├── shop-owner/             ← app 1
///   │   ├── manifest.json
///   │   ├── app.slint
///   │   └── app.lua
///   └── shop-customer/          ← app 2
///       ├── manifest.json
///       ├── app.slint
///       └── app.lua
/// ```
///
/// **We create:**
/// - Page with name = directory name
/// - Data layers from permit_template.json
/// - App layers for each subdirectory with manifest.json
pub async fn import_page_from_directory(
    butler: &Butler,
    space_id: &str,
    page_dir: &Path,
) -> Result<Page> {
    // 1. Get page name from directory name
    let page_name = page_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| ButlerError::Storage("Invalid page directory name".to_string()))?;

    tracing::info!(
        page_name = %page_name,
        page_dir = %page_dir.display(),
        "Importing page from directory"
    );

    // 2. Read permit template from page root
    let permit_path = page_dir.join("permit_template.json");
    let permit_template = std::fs::read_to_string(&permit_path)
        .map_err(|e| ButlerError::Storage(
            format!("Failed to read permit_template.json: {}", e)
        ))?;

    // 3. Scan for app subdirectories
    let app_dirs = scan_app_subdirectories(page_dir)?;
    if app_dirs.is_empty() {
        return Err(ButlerError::Storage(
            "No apps found in page directory (subdirectories with manifest.json)".to_string()
        ));
    }

    tracing::info!(
        page_name = %page_name,
        app_count = app_dirs.len(),
        apps = ?app_dirs.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        "Found apps in page directory"
    );

    // 4. Extract data layer names from permit template
    let mut layer_names = extract_layer_names_from_permit(&permit_template)?;

    // 5. Add app layers to the list
    for (app_name, _) in &app_dirs {
        layer_names.push(app_layer_name(app_name));
    }

    // 6. Create page with all layers
    let page = butler.create_page(
        space_id,
        page_name,
        layer_names,
        &permit_template,
    ).await?;

    tracing::info!(page_id = %page.id, page_name = %page.name, "Created page");

    // 7. Get AES key for encrypting app layers
    let (_decrypted, aes_key) = butler.get_decrypted_page(&page.id).await?;

    // 8. Populate each app layer with files
    for (app_name, app_path) in &app_dirs {
        let files = collect_app_files(app_path)?;
        let layer_name = app_layer_name(app_name);

        let app_layer = Layer::new();
        app_layer.set_all_files(&files)?;

        let snapshot = app_layer.export_snapshot();
        let encrypted = herald::encrypt_symmetric(&aes_key, &snapshot)
            .map_err(|e| ButlerError::Encryption(e.to_string()))?;

        butler.store().put_layer(&page.id, &layer_name, &encrypted)?;

        tracing::info!(
            page_id = %page.id,
            app_name = %app_name,
            file_count = files.len(),
            "Added app to page"
        );
    }

    tracing::info!(
        page_id = %page.id,
        page_name = %page.name,
        app_count = app_dirs.len(),
        "Successfully imported page with all apps"
    );

    Ok(page)
}

/// Reload a page from directory (sync from filesystem)
///
/// **Behavior:**
/// - Updates existing apps (files changed on disk)
/// - Adds new apps (new subdirectories with manifest.json)
/// - Does NOT remove apps (to preserve data; use explicit delete)
///
/// **Returns:** PageReloadResult with updated/added/removed app lists
pub async fn reload_page_from_directory(
    butler: &Butler,
    page_id: &str,
    page_dir: &Path,
) -> Result<PageReloadResult> {
    // Verify page exists
    let page = butler.get_page(page_id)?
        .ok_or_else(|| ButlerError::NotFound(format!("Page {} not found", page_id)))?;

    tracing::info!(
        page_id = %page_id,
        page_name = %page.meta.name,
        page_dir = %page_dir.display(),
        "Reloading page from directory"
    );

    // Get current apps in page
    let existing_apps = list_apps_in_page(butler, page_id)?;
    let existing_set: std::collections::HashSet<_> = existing_apps.iter().collect();

    // Scan directory for apps
    let app_dirs = scan_app_subdirectories(page_dir)?;
    let dir_app_names: std::collections::HashSet<_> = app_dirs.iter().map(|(name, _)| name).collect();

    let mut result = PageReloadResult::default();

    // Get AES key
    let (_decrypted, aes_key) = butler.get_decrypted_page(page_id).await?;

    // Process each app in directory
    for (app_name, app_path) in &app_dirs {
        if existing_set.contains(app_name) {
            // Update existing app
            let changed = refresh_app_from_directory(butler, page_id, app_name, app_path).await?;
            if !changed.is_empty() {
                result.updated_apps.push(app_name.clone());
            }
        } else {
            // Add new app
            let files = collect_app_files(app_path)?;
            let layer_name = app_layer_name(app_name);

            let app_layer = Layer::new();
            app_layer.set_all_files(&files)?;

            let snapshot = app_layer.export_snapshot();
            let encrypted = herald::encrypt_symmetric(&aes_key, &snapshot)
                .map_err(|e| ButlerError::Encryption(e.to_string()))?;

            butler.store().put_layer(page_id, &layer_name, &encrypted)?;

            result.added_apps.push(app_name.clone());
            tracing::info!(page_id = %page_id, app_name = %app_name, "Added new app to page");
        }
    }

    // Track removed apps (in page but not in directory)
    // Note: We don't actually remove them to preserve data
    for app_name in &existing_apps {
        if !dir_app_names.contains(app_name) {
            result.removed_apps.push(app_name.clone());
            tracing::warn!(
                page_id = %page_id,
                app_name = %app_name,
                "App exists in page but not in directory (not removed, data preserved)"
            );
        }
    }

    tracing::info!(
        page_id = %page_id,
        updated = result.updated_apps.len(),
        added = result.added_apps.len(),
        removed = result.removed_apps.len(),
        "Page reload complete"
    );

    Ok(result)
}
