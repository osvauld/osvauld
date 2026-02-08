//! Assets API - Asset upload operations
//!
//! Handles asset upload for pages (images, PDFs, etc.)
//! Assets are encrypted with the page's AES key and stored in the filesystem.

use crate::{Butler, ButlerError, Result, ScribeMessage};
use crate::services;

/// Assets API facade
///
/// Access via `butler.assets()`
pub struct AssetsApi<'a> {
    pub(crate) butler: &'a Butler,
}

impl<'a> AssetsApi<'a> {
    /// Upload an asset to local storage (async version)
    ///
    /// **Context**: User uploads a file (image, PDF, etc.) via file picker
    /// **Flow**:
    ///   1. Get page's AES key for encryption
    ///   2. Get user's identity for signing
    ///   3. Encrypt and store asset
    ///   4. Add metadata to page's assets layer
    ///   5. Return the content hash
    ///
    /// **Security**: Called from Slint button click only (user-initiated)
    ///
    /// # Arguments
    /// * `page_id` - The page to associate the asset with
    /// * `data` - Raw plaintext bytes
    /// * `filename` - Original filename
    /// * `mime_type` - MIME type (e.g., "image/png")
    ///
    /// # Returns
    /// Content hash of the uploaded asset
    pub async fn upload(&self, page_id: &str, data: &[u8], filename: &str, mime_type: &str) -> Result<String> {
        // Get page's AES key
        let (_decrypted, aes_key) = self.butler.pages().get_decrypted(page_id).await?;

        // Get user's identity for signing
        let identity = self.butler.get_identity().await?;

        // Upload via asset_service
        let metadata = services::asset_service::upload_asset(
            self.butler.asset_store(),
            &aes_key,
            &identity,
            filename,
            mime_type,
            data,
        )?;

        // Add metadata to {page_id}/assets layer via Scribe
        let assets_layer_name = format!("{}/assets", page_id);
        let scribe = self.butler.open_page(page_id).await?;

        // Serialize metadata to JSON for MapInsert
        let metadata_json = serde_json::to_value(&metadata)
            .map_err(|e| ButlerError::Database(format!("Failed to serialize metadata: {}", e)))?;

        // Send MapInsert to add metadata to assets layer (key = hash)
        scribe.cast(ScribeMessage::MapInsert {
            layer_name: assets_layer_name,
            path: "root".to_string(),
            key: metadata.hash.clone(),
            value: metadata_json,
        }).map_err(|e| ButlerError::Database(format!("Failed to send to scribe: {}", e)))?;

        tracing::info!(
            hash = %metadata.hash,
            filename = %filename,
            page_id = %page_id,
            "Asset uploaded and metadata added to layer"
        );

        Ok(metadata.hash)
    }

    /// Upload an asset to local storage (sync version)
    ///
    /// **Note**: Requires a tokio runtime to be available on current thread.
    /// Use `upload()` if calling from an async context or spawned task.
    pub fn upload_sync(&self, page_id: &str, data: &[u8], filename: &str, mime_type: &str) -> Result<String> {
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|_| ButlerError::Database("No tokio runtime".to_string()))?;

        rt.block_on(self.upload(page_id, data, filename, mime_type))
    }
}
