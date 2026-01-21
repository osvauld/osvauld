//! PageService - Stateless functions for Page operations
//!
//! All functions take store as first parameter. Butler injects dependencies.

use crate::error::{ButlerError, Result};
use crate::storage::RedbStore;
use crate::models::{
    PageData, PageMeta, Page,
    Layer, DecryptedPage,
};
use herald::{generate_aes_key, encrypt, encrypt_symmetric};
use std::collections::HashMap;
use uuid::Uuid;

// =========================================================================
// Page Operations
// =========================================================================

/// Create a new page with layers from permit template
///
/// # Arguments
/// * `store` - RedbStore reference
/// * `space_id` - Parent space ID
/// * `name` - Page name
/// * `owner_did` - Owner's DID
/// * `owner_public_key` - Owner's X25519 public key for encrypting AES key
/// * `signing_key` - Owner's Ed25519 signing key for permit issuance
/// * `layer_names` - List of layer names from permit template (e.g., ["content", "comments"])
/// * `permit_template_json` - JSON template for the page permit
///
/// # Flow
/// 1. Generate random AES-256 key
/// 2. For each layer_name: create empty Layer, encrypt with AES, store at {page_id}/{layer_name}
/// 3. Encrypt AES key for owner using X25519 ECIES
/// 4. Issue page owner permit via gurkha
/// 5. Store page with encrypted_key and owner permit
pub async fn create_page(
    store: &RedbStore,
    space_id: String,
    name: String,
    owner_did: String,
    owner_public_key: &[u8; 32],
    signing_key: &[u8; 32],
    layer_names: Vec<String>,
    permit_template_json: &str,
) -> Result<Page> {
    // Verify space exists
    if store.get_space(&space_id)?.is_none() {
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

        // Strip {page_id}/ prefix from template layer names
        // Template uses "{page_id}/products", but put_layer adds page_id prefix
        // So we store just "products" which becomes "actual-uuid/products"
        let normalized_name = layer_name
            .strip_prefix("{page_id}/")
            .unwrap_or(layer_name);

        // Store at hierarchical key: {page_id}/{layer_name}
        store.put_layer(&page_id, normalized_name, &encrypted_layer)?;
    }

    // Encrypt AES key for owner using X25519 ECIES
    let encrypted_key = encrypt(owner_public_key, &aes_key)
        .map_err(|e| ButlerError::Encryption(e.to_string()))?;

    // Issue page owner permit via gurkha
    let (owner_permit, _cid) = gurkha::issue_page_owner_token(
        signing_key,
        &page_id,
        permit_template_json,
    ).await.map_err(|e| ButlerError::Permit(e.to_string()))?;

    // Create page metadata with encrypted key
    let mut meta = PageMeta::new(name, space_id, owner_did)
        .with_encrypted_key(encrypted_key);
    meta.id = page_id; // Use the ID we generated earlier

    // Store page with owner's permit
    let mut data = PageData::new(meta.clone());
    data.set_permit(owner_permit);
    store.put_page(&data)?;

    Ok(Page::from(meta))
}

/// Create a private page (for private conversations)
pub async fn create_private_page(
    store: &RedbStore,
    space_id: String,
    name: String,
    owner_did: String,
    owner_public_key: &[u8; 32],
    layer_names: Vec<String>,
) -> Result<Page> {
    // Verify space exists
    if store.get_space(&space_id)?.is_none() {
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

        // Strip {page_id}/ prefix from template layer names
        let normalized_name = layer_name
            .strip_prefix("{page_id}/")
            .unwrap_or(layer_name);

        store.put_layer(&page_id, normalized_name, &encrypted_layer)?;
    }

    let encrypted_key = encrypt(owner_public_key, &aes_key)
        .map_err(|e| ButlerError::Encryption(e.to_string()))?;

    let mut meta = PageMeta::new(name, space_id, owner_did)
        .with_encrypted_key(encrypted_key)
        .as_private();
    meta.id = page_id;

    let data = PageData::new(meta.clone());
    store.put_page(&data)?;

    Ok(Page::from(meta))
}

/// Get a page by ID
pub fn get_page(store: &RedbStore, space_id: &str, page_id: &str) -> Result<Option<Page>> {
    Ok(store.get_page(space_id, page_id)?.map(Page::from))
}

/// Get page with its shares
pub fn get_page_with_shares(
    store: &RedbStore,
    space_id: &str,
    page_id: &str,
) -> Result<Option<PageData>> {
    store.get_page(space_id, page_id)
}

/// List pages in a space
pub fn list_pages(store: &RedbStore, space_id: &str) -> Result<Vec<Page>> {
    Ok(store.list_pages_in_space(space_id)?
        .into_iter()
        .map(Page::from)
        .collect())
}

/// List all pages
pub fn list_all_pages(store: &RedbStore) -> Result<Vec<Page>> {
    Ok(store.list_all_pages()?
        .into_iter()
        .map(Page::from)
        .collect())
}

/// Delete a page and its layers
pub fn delete_page(store: &RedbStore, space_id: &str, page_id: &str) -> Result<bool> {
    // Delete all layers for this page (hierarchical keys: {page_id}/{layer_name})
    store.delete_all_layers(page_id)?;

    // Delete the page itself
    store.delete_page(space_id, page_id)
}

/// Track that page was shared with a user (stores pubkey only)
///
/// Note: Actual permits are stored elsewhere (Contact.shares on Node-side)
pub fn share_page(
    store: &RedbStore,
    space_id: &str,
    page_id: &str,
    user_pubkey: String,
) -> Result<()> {
    let mut page = store.get_page(space_id, page_id)?
        .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;
    page.add_share(user_pubkey);
    store.put_page(&page)?;
    Ok(())
}

/// Remove share tracking for a user from page
pub fn unshare_page(
    store: &RedbStore,
    space_id: &str,
    page_id: &str,
    user_pubkey: &str,
) -> Result<bool> {
    let mut page = store.get_page(space_id, page_id)?
        .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;
    let removed = page.remove_share(user_pubkey);
    store.put_page(&page)?;
    Ok(removed)
}

/// Set permit for a page (stores MY permit)
pub fn set_page_permit(
    store: &RedbStore,
    space_id: &str,
    page_id: &str,
    permit: String,
) -> Result<()> {
    let mut page = store.get_page(space_id, page_id)?
        .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;
    page.set_permit(permit);
    store.put_page(&page)?;
    Ok(())
}

// =========================================================================
// Source Node Queries (for lazy sync)
// =========================================================================

/// Get the source node for a page (looks up space's source_node_id)
///
/// **Context**: Scribe needs to know which node to sync to for lazy subscription
/// **We do**: Find page → get space_id → get space → return source_node_id
pub fn get_source_node_for_page(store: &RedbStore, page_id: &str) -> Result<Option<String>> {
    // Find the page to get its space_id
    let page = match store.find_page_by_id(page_id)? {
        Some(p) => p,
        None => return Ok(None),
    };

    // Get the space to get source_node_id
    let space = match store.get_space(&page.meta.space_id)? {
        Some(s) => s,
        None => return Ok(None),
    };

    Ok(space.source_node_id)
}

// =========================================================================
// Access Queries
// =========================================================================

/// Get all pages (no filtering - if stored, user has access)
///
/// On viewer side: if page is in DB, viewer received it from an authorized node
/// On owner side: if page is in DB, owner created it or has permit for it
pub fn get_pages(store: &RedbStore) -> Result<Vec<Page>> {
    let all_pages = store.list_all_pages()?;
    Ok(all_pages.into_iter().map(Page::from).collect())
}

/// Check if a user has access to a page
pub fn has_page_access(
    store: &RedbStore,
    space_id: &str,
    page_id: &str,
    user_did: &str,
) -> Result<bool> {
    if let Some(page) = store.get_page(space_id, page_id)? {
        Ok(page.meta.owner_did == user_did || page.has_share(user_did))
    } else {
        Ok(false)
    }
}

// =========================================================================
// Page Retrieval (for reading)
// =========================================================================

/// Find a page by ID without knowing the space_id
pub fn find_page_by_id(store: &RedbStore, page_id: &str) -> Result<Option<PageData>> {
    store.find_page_by_id(page_id)
}

/// Get a page and decrypt its layers
///
/// # Arguments
/// * `store` - RedbStore reference
/// * `page_id` - The page ID to fetch
/// * `private_key` - User's X25519 private key for decrypting the AES key
///
/// # Returns
/// A map of layer_name -> decrypted Layer data as JSON
pub fn get_page_decrypted(
    store: &RedbStore,
    page_id: &str,
    private_key: &[u8; 32],
) -> Result<(PageData, std::collections::HashMap<String, serde_json::Value>)> {
    // 1. Find the page
    let page = store.find_page_by_id(page_id)?
        .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;

    // 2. Decrypt the AES key using user's private key
    let aes_key = herald::decrypt(private_key, &page.meta.encrypted_key)
        .map_err(|e| ButlerError::Encryption(format!("Failed to decrypt AES key: {}", e)))?;

    // 3. Get all encrypted layers for this page
    let encrypted_layers = store.get_all_layers(page_id)?;

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
pub fn get_page_metadata(store: &RedbStore, page_id: &str) -> Result<Option<Page>> {
    Ok(store.find_page_by_id(page_id)?.map(Page::from))
}

/// Get a fully decrypted Page with all layer data
///
/// This is the primary method for fetching page data. It:
/// 1. Loads page metadata
/// 2. Decrypts all layers in parallel using futures
/// 3. Returns a DecryptedPage domain object
///
/// # Arguments
/// * `store` - RedbStore reference
/// * `page_id` - The page ID to fetch
/// * `user_did` - The user's DID (to get their UCAN token)
/// * `private_key` - User's X25519 secret key for decryption
///
/// # Returns
/// `(DecryptedPage, [u8; 32])` - page data and the decrypted AES key for re-encryption
pub async fn get_decrypted_page(
    store: &RedbStore,
    page_id: &str,
    _user_did: &str,
    private_key: &[u8; 32],
) -> Result<(DecryptedPage, [u8; 32])> {
    // 1. Find the page
    let page_data = store.find_page_by_id(page_id)?
        .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;

    // 2. Get my permit for this page (stored directly on the page)
    let permit = page_data.permit.clone();

    // 3. Decrypt the AES key using user's private key
    let aes_key = herald::decrypt(private_key, &page_data.meta.encrypted_key)
        .map_err(|e| ButlerError::Encryption(format!("Failed to decrypt AES key: {}", e)))?;

    let aes_key_arr: [u8; 32] = aes_key.try_into()
        .map_err(|_| ButlerError::Encryption("Invalid AES key length".to_string()))?;

    // 4. Get all encrypted layers for this page
    let encrypted_layers: Vec<(String, Vec<u8>)> = store.get_all_layers(page_id)?
        .into_iter()
        .collect();

    log::info!("get_decrypted_page: page_id={} found {} layers: {:?}",
        page_id,
        encrypted_layers.len(),
        encrypted_layers.iter().map(|(n, b)| (n.as_str(), b.len())).collect::<Vec<_>>()
    );

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

    // 7. Build and return DecryptedPage with AES key
    let page = Page::from(page_data);
    Ok((DecryptedPage::new(page, permit, docs), aes_key_arr))
}

/// Update a page's layers with new snapshot data
///
/// # Arguments
/// * `store` - RedbStore reference
/// * `page_id` - The page ID to update
/// * `secret_key` - User's X25519 secret key for decrypting the AES key
/// * `layer_updates` - Map of layer_name -> snapshot bytes (as JSON array of u8)
///
/// The data format matches the old Resource format:
/// `{"layer_name": [1, 2, 3, ...], "other_layer": [4, 5, 6, ...]}`
/// where each value is a Loro snapshot as a JSON array of bytes.
pub fn update_page_layers(
    store: &RedbStore,
    page_id: &str,
    secret_key: &[u8; 32],
    layer_updates: &std::collections::HashMap<String, serde_json::Value>,
) -> Result<Page> {
    // 1. Find the page
    let mut page = store.find_page_by_id(page_id)?
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
        store.put_layer(page_id, layer_name, &encrypted_layer)?;
    }

    // 4. Update page timestamp
    page.meta.updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    store.put_page(&page)?;

    Ok(Page::from(page.meta))
}
