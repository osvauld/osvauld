//! Apps API - App layer operations
//!
//! Groups all app-related operations:
//! - App layer listing and file access
//! - App import/update from filesystem
//! - Data layer listing

use std::collections::HashMap;
use std::path::Path;

use crate::services;
use crate::{Butler, ButlerError, Layer, Page, Result};

/// Apps API facade
///
/// Access via `butler.apps()`
pub struct AppsApi<'a> {
    pub(crate) butler: &'a Butler,
}

impl<'a> AppsApi<'a> {
    /// List all apps in a page
    ///
    /// **Context**: Find all `app:*` layers in a page
    /// **Returns**: List of app names (without "app:" prefix)
    pub fn list(&self, page_id: &str) -> Result<Vec<String>> {
        let layers = self.butler.store().list_layer_names(page_id)?;
        let apps: Vec<String> = layers
            .into_iter()
            .filter_map(|name| name.strip_prefix("app:").map(|s| s.to_string()))
            .collect();
        Ok(apps)
    }

    /// List data layers in a page
    ///
    /// **Context**: Find layers that store app data (not app code, not files, not static assets)
    /// **Returns**: Layer names that don't have app:, file:, or static: prefix
    pub fn list_data_layers(&self, page_id: &str) -> Result<Vec<String>> {
        let layers = self.butler.store().list_layer_names(page_id)?;
        let data_layers: Vec<String> = layers
            .into_iter()
            .filter(|name| {
                !name.starts_with("app:")
                    && !name.starts_with("file:")
                    && !name.starts_with("static:")
            })
            .collect();
        Ok(data_layers)
    }

    /// Get all files from an app layer
    ///
    /// **Context**: Load app files from `app:{app_name}` LoroMap layer
    /// **Returns**: HashMap of file_path -> content
    pub async fn get_files(
        &self,
        page_id: &str,
        app_name: &str,
    ) -> Result<HashMap<String, String>> {
        let layer_name = format!("app:{}", app_name);

        // Get decrypted page data
        let (decrypted, _aes_key) = self.butler.pages().get_decrypted(page_id).await?;

        // Check if app layer exists
        let layer_bytes = decrypted.docs.get(&layer_name).ok_or_else(|| {
            ButlerError::NotFound(format!("App '{}' not found in page", app_name))
        })?;

        if layer_bytes.is_empty() {
            return Ok(HashMap::new());
        }

        // Reconstruct Layer from snapshot and extract files
        let layer = Layer::from_snapshot(layer_bytes)
            .map_err(|e| ButlerError::Internal(format!("Layer: {}", e)))?;

        let files = layer.get_all_files();

        tracing::debug!(
            page_id = %page_id,
            app_name = %app_name,
            file_count = files.len(),
            "Loaded app files from layer"
        );

        Ok(files)
    }

    /// Get a single file from an app layer
    ///
    /// **Context**: Load specific file from `app:{app_name}` LoroMap layer
    /// **Returns**: File content or None if not found
    pub async fn get_file(
        &self,
        page_id: &str,
        app_name: &str,
        path: &str,
    ) -> Result<Option<String>> {
        let files = self.get_files(page_id, app_name).await?;
        Ok(files.get(path).cloned())
    }

    /// Import an app from a directory
    ///
    /// **Context**: Development workflow - import sample app to Butler
    pub async fn import(&self, space_id: &str, app_dir: &Path) -> Result<Page> {
        services::app_service::import_app_from_directory(self.butler, space_id, app_dir).await
    }

    /// Update an existing app from a directory
    ///
    /// **Context**: Development workflow - update app with new version
    /// **Flow**: Read manifest -> collect files -> ensure Scribe open -> send RefreshAppFiles
    /// **Why Scribe**: Goes through Scribe so CRDT state, observers, and sync all fire correctly
    pub async fn update(&self, page_id: &str, app_dir: &Path) -> Result<()> {
        // Parse manifest to get app name
        let manifest_path = app_dir.join("manifest.json");
        let manifest_content = std::fs::read_to_string(&manifest_path)
            .map_err(|e| ButlerError::Database(format!("Failed to read manifest.json: {}", e)))?;
        let manifest: serde_json::Value = serde_json::from_str(&manifest_content)
            .map_err(|e| ButlerError::Serialization(format!("Invalid manifest.json: {}", e)))?;
        let app_name = manifest
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| ButlerError::Database("manifest.json missing 'name' field".to_string()))?
            .to_string();

        // Read files from disk
        let files = services::app_service::collect_app_files_from_directory(app_dir)?;

        // Ensure Scribe is open for this page (spawns if needed)
        let scribe_ref = self.butler.open_page(page_id).await?;

        // Send RefreshAppFiles to Scribe and await reply
        let (tx, rx) = tokio::sync::oneshot::channel();
        scribe_ref
            .cast(crate::ScribeMessage::RefreshAppFiles {
                app_name: app_name.clone(),
                files,
                reply: tx,
            })
            .map_err(|e| ButlerError::Internal(format!("Failed to send RefreshAppFiles: {}", e)))?;

        rx.await
            .map_err(|_| ButlerError::Internal("Scribe dropped reply channel".to_string()))?
            .map_err(|e| ButlerError::Internal(format!("RefreshAppFiles failed: {}", e)))?;

        tracing::info!(page_id = %page_id, app_name = %app_name, "App updated via Scribe");
        Ok(())
    }

    /// Import a page directory containing multiple apps
    ///
    /// **Context**: Development workflow - import page with multiple apps
    /// **Structure**:
    /// ```text
    /// page_dir/                     ← directory name = page name
    ///   ├── app.osv                 ← page policy declaration (data layers + access)
    ///   ├── shop-owner/             ← app 1
    ///   └── shop-customer/          ← app 2
    /// ```
    pub async fn import_page(&self, space_id: &str, page_dir: &Path) -> Result<Page> {
        services::app_service::import_page_from_directory(self.butler, space_id, page_dir).await
    }

    /// Reload a page from directory (sync from filesystem)
    ///
    /// **Context**: Development workflow - reload page after editing files
    /// **Behavior**:
    /// - Updates existing apps (files changed on disk)
    /// - Adds new apps (new subdirectories with manifest.json)
    pub async fn reload_page(
        &self,
        page_id: &str,
        page_dir: &Path,
    ) -> Result<services::app_service::PageReloadResult> {
        services::app_service::reload_page_from_directory(self.butler, page_id, page_dir).await
    }
}
