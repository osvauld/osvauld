//! Files API - File storage operations
//!
//! Groups all file-related operations:
//! - Page files (app source code, encrypted with page AES key)
//! - Static files (binary blobs like images, fonts)

use crate::{Butler, ButlerError, Result};

/// Files API facade
///
/// Access via `butler.files()`
pub struct FilesApi<'a> {
    pub(crate) butler: &'a Butler,
}

impl<'a> FilesApi<'a> {

    /// Save an app file to page storage
    ///
    /// **Context**: User uploads a multi-file app (e.g., .slint, .lua files)
    /// **We do**: Encrypt with page AES key, store with "file:{path}" layer name
    pub async fn save(&self, page_id: &str, path: &str, content: &str) -> Result<()> {
        // Get page's AES key
        let (_decrypted, aes_key) = self.butler.pages().get_decrypted(page_id).await?;

        // Encrypt content with page's AES key
        let encrypted = herald::encrypt_symmetric(&aes_key, content.as_bytes())
            .map_err(|e| ButlerError::Encryption(e.to_string()))?;

        // Store with prefixed layer name to avoid collisions
        let layer_name = format!("file:{}", path);
        self.butler.store().put_layer(page_id, &layer_name, &encrypted)?;

        tracing::info!(page_id = %page_id, file_path = %path, size = content.len(), "Saved app file to page");
        Ok(())
    }

    /// Get an app file from page storage
    ///
    /// **Context**: Loading a multi-file app
    /// **Returns**: Decrypted file content or None if file doesn't exist
    pub async fn get(&self, page_id: &str, path: &str) -> Result<Option<String>> {
        let layer_name = format!("file:{}", path);

        // Check if layer exists
        let encrypted = match self.butler.store().get_layer(page_id, &layer_name)? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        // Get page's AES key to decrypt
        let (_decrypted, aes_key) = self.butler.pages().get_decrypted(page_id).await?;

        // Decrypt file
        let file_bytes = herald::decrypt_symmetric(&aes_key, &encrypted)
            .map_err(|e| ButlerError::Encryption(format!("Decryption failed: {}", e)))?;

        let content = String::from_utf8(file_bytes)
            .map_err(|e| ButlerError::Encryption(format!("Invalid UTF-8 in file: {}", e)))?;

        tracing::info!(page_id = %page_id, file_path = %path, size = content.len(), "Loaded app file from page");
        Ok(Some(content))
    }

    /// List all app files in a page
    ///
    /// **Context**: Need to enumerate files in a multi-file app
    /// **Returns**: List of file paths (without "file:" prefix)
    pub fn list(&self, page_id: &str) -> Result<Vec<String>> {
        let layers = self.butler.store().list_layer_names(page_id)?;
        let files: Vec<String> = layers
            .into_iter()
            .filter_map(|name| name.strip_prefix("file:").map(|s| s.to_string()))
            .collect();
        Ok(files)
    }

    /// Save a static binary file to page storage
    ///
    /// **Context**: Store binary assets (images, fonts) that don't need CRDT merging
    /// **We do**: Encrypt with page AES key, store with "static:{path}" layer name
    pub async fn save_static(&self, page_id: &str, path: &str, content: &[u8]) -> Result<()> {
        // Get page's AES key
        let (_decrypted, aes_key) = self.butler.pages().get_decrypted(page_id).await?;

        // Encrypt content with page's AES key
        let encrypted = herald::encrypt_symmetric(&aes_key, content)
            .map_err(|e| ButlerError::Encryption(e.to_string()))?;

        // Store with static: prefix
        let layer_name = format!("static:{}", path);
        self.butler.store().put_layer(page_id, &layer_name, &encrypted)?;

        tracing::info!(page_id = %page_id, file_path = %path, size = content.len(), "Saved static file");
        Ok(())
    }

    /// Get a static binary file from page storage
    ///
    /// **Context**: Load binary assets (images, fonts)
    /// **Returns**: Decrypted bytes or None if file doesn't exist
    pub async fn get_static(&self, page_id: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let layer_name = format!("static:{}", path);

        // Check if layer exists
        let encrypted = match self.butler.store().get_layer(page_id, &layer_name)? {
            Some(data) => data,
            None => return Ok(None),
        };

        // Get page's AES key
        let (_decrypted, aes_key) = self.butler.pages().get_decrypted(page_id).await?;

        // Decrypt
        let content = herald::decrypt_symmetric(&aes_key, &encrypted)
            .map_err(|e| ButlerError::Encryption(format!("Decryption failed: {}", e)))?;

        tracing::info!(page_id = %page_id, file_path = %path, size = content.len(), "Loaded static file");
        Ok(Some(content))
    }

    /// List all static files in a page
    ///
    /// **Context**: Enumerate binary assets in a page
    /// **Returns**: List of file paths (without "static:" prefix)
    pub fn list_static(&self, page_id: &str) -> Result<Vec<String>> {
        let layers = self.butler.store().list_layer_names(page_id)?;
        let files: Vec<String> = layers
            .into_iter()
            .filter_map(|name| name.strip_prefix("static:").map(|s| s.to_string()))
            .collect();
        Ok(files)
    }

    /// Delete a static file from page storage
    pub fn delete_static(&self, page_id: &str, path: &str) -> Result<()> {
        let layer_name = format!("static:{}", path);
        self.butler.store().delete_layer(page_id, &layer_name)?;
        tracing::info!(page_id = %page_id, file_path = %path, "Deleted static file");
        Ok(())
    }
}
