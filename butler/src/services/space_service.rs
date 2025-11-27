//! SpaceService - Unified service for Space, Page, and Layer operations
//!
//! Terminology:
//! - Space: Container for Pages (like a project or workspace)
//! - Page: A sub-application instance with its own template
//! - Layer: CRDT data containers within a Page

use crate::error::{ButlerError, Result};
use crate::storage::{RedbStore, LayerCache};
use crate::models::{
    SpaceData, SpaceMeta, Space,
    PageData, PageMeta, Page, PageType,
    Layer, DecryptedPage,
};
use herald::{generate_aes_key, encrypt, encrypt_symmetric};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Unified service for Space, Page, and Layer operations
///
/// - Uses RedbStore directly (no repository layer)
/// - Manages LayerCache for layer operations
/// - Handles Space/Page CRUD and sharing
pub struct SpaceService {
    store: Arc<RedbStore>,
    layer_cache: Arc<RwLock<LayerCache>>,
}

impl SpaceService {
    pub fn new(store: Arc<RedbStore>, layer_cache: Arc<RwLock<LayerCache>>) -> Self {
        Self { store, layer_cache }
    }

    // =========================================================================
    // Space Operations
    // =========================================================================

    /// Create a new space
    pub fn create_space(&self, name: String, owner_did: String) -> Result<Space> {
        let meta = SpaceMeta::new(name, owner_did);
        let data = SpaceData::new(meta.clone());
        self.store.put_space(&data)?;
        Ok(Space::from(meta))
    }

    /// Create a space with a parent
    pub fn create_child_space(
        &self,
        name: String,
        parent_id: String,
        owner_did: String,
    ) -> Result<Space> {
        // Verify parent exists
        if self.store.get_space(&parent_id)?.is_none() {
            return Err(ButlerError::SpaceNotFound(parent_id));
        }

        let meta = SpaceMeta::new(name, owner_did).with_parent(parent_id);
        let data = SpaceData::new(meta.clone());
        self.store.put_space(&data)?;
        Ok(Space::from(meta))
    }

    /// Get a space by ID
    pub fn get_space(&self, space_id: &str) -> Result<Option<Space>> {
        Ok(self.store.get_space(space_id)?.map(Space::from))
    }

    /// Get space with its shares
    pub fn get_space_with_shares(&self, space_id: &str) -> Result<Option<SpaceData>> {
        self.store.get_space(space_id)
    }

    /// List all root spaces
    pub fn list_root_spaces(&self) -> Result<Vec<Space>> {
        Ok(self.store.list_root_spaces()?
            .into_iter()
            .map(Space::from)
            .collect())
    }

    /// List child spaces of a parent
    pub fn list_child_spaces(&self, parent_id: &str) -> Result<Vec<Space>> {
        Ok(self.store.list_child_spaces(parent_id)?
            .into_iter()
            .map(Space::from)
            .collect())
    }

    /// List all spaces
    pub fn list_all_spaces(&self) -> Result<Vec<Space>> {
        Ok(self.store.list_spaces()?
            .into_iter()
            .map(Space::from)
            .collect())
    }

    /// Delete a space (and optionally its pages)
    pub fn delete_space(&self, space_id: &str, delete_pages: bool) -> Result<bool> {
        if delete_pages {
            // Delete all pages in this space
            let pages = self.store.list_pages_in_space(space_id)?;
            for page in pages {
                self.delete_page(space_id, &page.meta.id)?;
            }
        }
        self.store.delete_space(space_id)
    }

    /// Add a share to a space
    pub fn share_space(&self, space_id: &str, user_did: String, ucan: String) -> Result<()> {
        let mut space = self.store.get_space(space_id)?
            .ok_or_else(|| ButlerError::SpaceNotFound(space_id.to_string()))?;
        space.add_share(user_did, ucan);
        self.store.put_space(&space)?;
        Ok(())
    }

    /// Remove a share from a space
    pub fn unshare_space(&self, space_id: &str, user_did: &str) -> Result<Option<String>> {
        let mut space = self.store.get_space(space_id)?
            .ok_or_else(|| ButlerError::SpaceNotFound(space_id.to_string()))?;
        let removed = space.remove_share(user_did);
        self.store.put_space(&space)?;
        Ok(removed)
    }

    // =========================================================================
    // Page Operations
    // =========================================================================

    /// Create a new page with layers from permit template
    ///
    /// # Arguments
    /// * `space_id` - Parent space ID
    /// * `name` - Page name
    /// * `owner_did` - Owner's DID
    /// * `owner_public_key` - Owner's X25519 public key for encrypting AES key
    /// * `page_type` - Type of page
    /// * `layer_names` - List of layer names from permit template (e.g., ["content", "comments"])
    ///
    /// # Encryption Flow
    /// 1. Generate random AES-256 key
    /// 2. For each layer_name: create empty Layer, encrypt with AES, store at {page_id}/{layer_name}
    /// 3. Encrypt AES key for owner using X25519 ECIES
    /// 4. Store page with encrypted_key
    pub async fn create_page(
        &self,
        space_id: String,
        name: String,
        owner_did: String,
        owner_public_key: &[u8; 32],
        page_type: PageType,
        layer_names: Vec<String>,
    ) -> Result<Page> {
        // Verify space exists
        if self.store.get_space(&space_id)?.is_none() {
            return Err(ButlerError::SpaceNotFound(space_id));
        }

        // Generate page ID first (needed for layer keys)
        let page_id = Uuid::new_v4().to_string();

        // Generate random AES-256 key for this page
        let aes_key = generate_aes_key();

        // Create and encrypt layers for each layer_name
        for layer_name in &layer_names {
            let layer = Layer::new();
            let snapshot = layer.export_snapshot();

            // Encrypt layer with AES key
            let encrypted_layer = encrypt_symmetric(&aes_key, &snapshot)
                .map_err(|e| ButlerError::Encryption(e.to_string()))?;

            // Store at hierarchical key: {page_id}/{layer_name}
            self.store.put_layer(&page_id, layer_name, &encrypted_layer)?;
        }

        // Encrypt AES key for owner using X25519 ECIES
        let encrypted_key = encrypt(owner_public_key, &aes_key)
            .map_err(|e| ButlerError::Encryption(e.to_string()))?;

        // Create page metadata with encrypted key
        let mut meta = PageMeta::new(name, space_id, owner_did)
            .with_type(page_type)
            .with_encrypted_key(encrypted_key);
        meta.id = page_id; // Use the ID we generated earlier

        let data = PageData::new(meta.clone());
        self.store.put_page(&data)?;

        Ok(Page::from(meta))
    }

    /// Create a private page (for private conversations)
    pub async fn create_private_page(
        &self,
        space_id: String,
        name: String,
        owner_did: String,
        owner_public_key: &[u8; 32],
        layer_names: Vec<String>,
    ) -> Result<Page> {
        // Verify space exists
        if self.store.get_space(&space_id)?.is_none() {
            return Err(ButlerError::SpaceNotFound(space_id));
        }

        let page_id = Uuid::new_v4().to_string();
        let aes_key = generate_aes_key();

        // Create and encrypt layers
        for layer_name in &layer_names {
            let layer = Layer::new();
            let snapshot = layer.export_snapshot();

            let encrypted_layer = encrypt_symmetric(&aes_key, &snapshot)
                .map_err(|e| ButlerError::Encryption(e.to_string()))?;

            self.store.put_layer(&page_id, layer_name, &encrypted_layer)?;
        }

        let encrypted_key = encrypt(owner_public_key, &aes_key)
            .map_err(|e| ButlerError::Encryption(e.to_string()))?;

        let mut meta = PageMeta::new(name, space_id, owner_did)
            .with_type(PageType::PrivateChat)
            .with_encrypted_key(encrypted_key)
            .as_private();
        meta.id = page_id;

        let data = PageData::new(meta.clone());
        self.store.put_page(&data)?;

        Ok(Page::from(meta))
    }

    /// Get a page by ID
    pub fn get_page(&self, space_id: &str, page_id: &str) -> Result<Option<Page>> {
        Ok(self.store.get_page(space_id, page_id)?.map(Page::from))
    }

    /// Get page with its shares
    pub fn get_page_with_shares(
        &self,
        space_id: &str,
        page_id: &str,
    ) -> Result<Option<PageData>> {
        self.store.get_page(space_id, page_id)
    }

    /// List pages in a space
    pub fn list_pages(&self, space_id: &str) -> Result<Vec<Page>> {
        Ok(self.store.list_pages_in_space(space_id)?
            .into_iter()
            .map(Page::from)
            .collect())
    }

    /// List all pages
    pub fn list_all_pages(&self) -> Result<Vec<Page>> {
        Ok(self.store.list_all_pages()?
            .into_iter()
            .map(Page::from)
            .collect())
    }

    /// Delete a page and its layers
    pub fn delete_page(&self, space_id: &str, page_id: &str) -> Result<bool> {
        // Delete all layers for this page (hierarchical keys: {page_id}/{layer_name})
        self.store.delete_all_layers(page_id)?;

        // Delete the page itself
        self.store.delete_page(space_id, page_id)
    }

    /// Add a share to a page
    pub fn share_page(
        &self,
        space_id: &str,
        page_id: &str,
        user_did: String,
        ucan: String,
    ) -> Result<()> {
        let mut page = self.store.get_page(space_id, page_id)?
            .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;
        page.add_share(user_did, ucan);
        self.store.put_page(&page)?;
        Ok(())
    }

    /// Remove a share from a page
    pub fn unshare_page(
        &self,
        space_id: &str,
        page_id: &str,
        user_did: &str,
    ) -> Result<Option<String>> {
        let mut page = self.store.get_page(space_id, page_id)?
            .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;
        let removed = page.remove_share(user_did);
        self.store.put_page(&page)?;
        Ok(removed)
    }

    // =========================================================================
    // Access Queries
    // =========================================================================

    /// Get all spaces a user has access to
    pub fn get_accessible_spaces(&self, user_did: &str) -> Result<Vec<Space>> {
        let all_spaces = self.store.list_spaces()?;
        Ok(all_spaces
            .into_iter()
            .filter(|s| s.meta.owner_did == user_did || s.has_share(user_did))
            .map(Space::from)
            .collect())
    }

    /// Get all pages a user has access to
    pub fn get_accessible_pages(&self, user_did: &str) -> Result<Vec<Page>> {
        let all_pages = self.store.list_all_pages()?;
        Ok(all_pages
            .into_iter()
            .filter(|p| p.meta.owner_did == user_did || p.has_share(user_did))
            .map(Page::from)
            .collect())
    }

    /// Check if a user has access to a space
    pub fn has_space_access(&self, space_id: &str, user_did: &str) -> Result<bool> {
        if let Some(space) = self.store.get_space(space_id)? {
            Ok(space.meta.owner_did == user_did || space.has_share(user_did))
        } else {
            Ok(false)
        }
    }

    /// Check if a user has access to a page
    pub fn has_page_access(
        &self,
        space_id: &str,
        page_id: &str,
        user_did: &str,
    ) -> Result<bool> {
        if let Some(page) = self.store.get_page(space_id, page_id)? {
            Ok(page.meta.owner_did == user_did || page.has_share(user_did))
        } else {
            Ok(false)
        }
    }

    // =========================================================================
    // Page Retrieval (for reading)
    // =========================================================================

    /// Find a page by ID without knowing the space_id
    pub fn find_page_by_id(&self, page_id: &str) -> Result<Option<PageData>> {
        self.store.find_page_by_id(page_id)
    }

    /// Get a page and decrypt its layers
    ///
    /// # Arguments
    /// * `page_id` - The page ID to fetch
    /// * `private_key` - User's X25519 private key for decrypting the AES key
    ///
    /// # Returns
    /// A map of layer_name -> decrypted Layer data as JSON
    pub fn get_page_decrypted(
        &self,
        page_id: &str,
        private_key: &[u8; 32],
    ) -> Result<(PageData, std::collections::HashMap<String, serde_json::Value>)> {
        // 1. Find the page
        let page = self.store.find_page_by_id(page_id)?
            .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;

        // 2. Decrypt the AES key using user's private key
        let aes_key = herald::decrypt(private_key, &page.meta.encrypted_key)
            .map_err(|e| ButlerError::Encryption(format!("Failed to decrypt AES key: {}", e)))?;

        // 3. Get all encrypted layers for this page
        let encrypted_layers = self.store.get_all_layers(page_id)?;

        // 4. Decrypt each layer and convert to JSON
        let mut decrypted_layers = std::collections::HashMap::new();
        for (layer_name, encrypted_bytes) in encrypted_layers {
            // Convert aes_key to fixed array
            let aes_key_arr: [u8; 32] = aes_key.clone().try_into()
                .map_err(|_| ButlerError::Encryption("Invalid AES key length".to_string()))?;

            let decrypted_bytes = herald::decrypt_symmetric(&aes_key_arr, &encrypted_bytes)
                .map_err(|e| ButlerError::Encryption(format!("Failed to decrypt layer {}: {}", layer_name, e)))?;

            // Parse as Loro document and export as JSON
            let layer = Layer::from_snapshot(&decrypted_bytes)
                .map_err(|e| ButlerError::Layer(format!("Failed to parse layer {}: {}", layer_name, e)))?;

            // Export the layer state to JSON value
            let json_value = layer.to_json_value();
            decrypted_layers.insert(layer_name, json_value);
        }

        Ok((page, decrypted_layers))
    }

    /// Get just the page metadata by ID
    pub fn get_page_metadata(&self, page_id: &str) -> Result<Option<Page>> {
        Ok(self.store.find_page_by_id(page_id)?.map(Page::from))
    }

    /// Get a fully decrypted Page with all layer data
    ///
    /// This is the primary method for fetching page data. It:
    /// 1. Loads page metadata
    /// 2. Decrypts all layers in parallel using futures
    /// 3. Returns a DecryptedPage domain object
    ///
    /// # Arguments
    /// * `page_id` - The page ID to fetch
    /// * `user_did` - The user's DID (to get their UCAN token)
    /// * `private_key` - User's X25519 secret key for decryption
    ///
    /// # Returns
    /// `DecryptedPage` containing metadata and decrypted layer bytes
    pub async fn get_decrypted_page(
        &self,
        page_id: &str,
        user_did: &str,
        private_key: &[u8; 32],
    ) -> Result<DecryptedPage> {
        // 1. Find the page
        let page_data = self.store.find_page_by_id(page_id)?
            .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;

        // 2. Get user's permit for this page (if they have one)
        let permit = page_data.shares.get(user_did).cloned();

        // 3. Decrypt the AES key using user's private key
        let aes_key = herald::decrypt(private_key, &page_data.meta.encrypted_key)
            .map_err(|e| ButlerError::Encryption(format!("Failed to decrypt AES key: {}", e)))?;

        let aes_key_arr: [u8; 32] = aes_key.try_into()
            .map_err(|_| ButlerError::Encryption("Invalid AES key length".to_string()))?;

        // 4. Get all encrypted layers for this page
        let encrypted_layers: Vec<(String, Vec<u8>)> = self.store.get_all_layers(page_id)?
            .into_iter()
            .collect();

        // 5. Decrypt all layers in parallel using futures
        let decrypt_futures: Vec<_> = encrypted_layers
            .into_iter()
            .map(|(layer_name, encrypted_bytes)| {
                let aes_key = aes_key_arr;
                async move {
                    let decrypted_bytes = herald::decrypt_symmetric(&aes_key, &encrypted_bytes)
                        .map_err(|e| ButlerError::Encryption(
                            format!("Failed to decrypt layer {}: {}", layer_name, e)
                        ))?;
                    Ok::<_, ButlerError>((layer_name, decrypted_bytes))
                }
            })
            .collect();

        let results = futures::future::join_all(decrypt_futures).await;

        // 6. Collect results, propagating any errors
        let mut docs = HashMap::new();
        for result in results {
            let (name, bytes) = result?;
            docs.insert(name, bytes);
        }

        // 7. Build and return DecryptedPage
        let page = Page::from(page_data);
        Ok(DecryptedPage::new(page, permit, docs))
    }

    /// Update a page's layers with new snapshot data
    ///
    /// # Arguments
    /// * `page_id` - The page ID to update
    /// * `secret_key` - User's X25519 secret key for decrypting the AES key
    /// * `layer_updates` - Map of layer_name -> snapshot bytes (as JSON array of u8)
    ///
    /// The data format matches the old Resource format:
    /// `{"layer_name": [1, 2, 3, ...], "other_layer": [4, 5, 6, ...]}`
    /// where each value is a Loro snapshot as a JSON array of bytes.
    pub fn update_page_layers(
        &self,
        page_id: &str,
        secret_key: &[u8; 32],
        layer_updates: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Page> {
        // 1. Find the page
        let mut page = self.store.find_page_by_id(page_id)?
            .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;

        // 2. Decrypt the AES key using user's secret key
        let aes_key = herald::decrypt(secret_key, &page.meta.encrypted_key)
            .map_err(|e| ButlerError::Encryption(format!("Failed to decrypt AES key: {}", e)))?;

        let aes_key_arr: [u8; 32] = aes_key.try_into()
            .map_err(|_| ButlerError::Encryption("Invalid AES key length".to_string()))?;

        // 3. For each layer update, convert JSON array to bytes, encrypt, and store
        for (layer_name, json_data) in layer_updates {
            // Skip static_assets for now (handled separately)
            if layer_name == "static_assets" {
                continue;
            }

            // Convert JSON array [1, 2, 3, ...] to Vec<u8>
            let snapshot_bytes: Vec<u8> = if let Some(arr) = json_data.as_array() {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect()
            } else {
                // If not an array, skip this layer
                continue;
            };

            if snapshot_bytes.is_empty() {
                continue;
            }

            // Encrypt the snapshot
            let encrypted_layer = herald::encrypt_symmetric(&aes_key_arr, &snapshot_bytes)
                .map_err(|e| ButlerError::Encryption(format!("Failed to encrypt layer {}: {}", layer_name, e)))?;

            // Store the updated layer
            self.store.put_layer(page_id, layer_name, &encrypted_layer)?;
        }

        // 4. Update page timestamp
        page.meta.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        self.store.put_page(&page)?;

        Ok(Page::from(page.meta))
    }
}
