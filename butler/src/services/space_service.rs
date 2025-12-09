//! SpaceService - Stateless functions for Space, Page, and Layer operations
//!
//! All functions take store (and layer_cache where needed) as first parameters.
//! Butler injects these dependencies.
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
    Layer, DecryptedPage, PreparedPage,
};
use herald::{generate_aes_key, encrypt, encrypt_symmetric};
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

// =========================================================================
// Space Operations
// =========================================================================

/// Create a new space with owner permit
///
/// **Context**: Owner creates a new space
/// **We do**: Create space metadata, issue owner permit via gurkha, store with permit
/// **We return**: Space domain object
pub async fn create_space(
    store: &RedbStore,
    name: String,
    owner_did: String,
    signing_key: &[u8; 32],
    permit_template_json: &str,
) -> Result<Space> {
    let meta = SpaceMeta::new(name, owner_did);

    // Issue space owner permit via gurkha
    let (owner_permit, _cid) = gurkha::issue_space_owner_token(
        signing_key,
        &meta.id,
        permit_template_json,
    ).await.map_err(|e| ButlerError::Permit(e.to_string()))?;

    // Store space with owner's permit
    let mut data = SpaceData::new(meta.clone());
    data.set_permit(owner_permit);
    store.put_space(&data)?;

    Ok(Space::from(meta))
}

/// Create a space with a parent
pub fn create_child_space(
    store: &RedbStore,
    name: String,
    parent_id: String,
    owner_did: String,
) -> Result<Space> {
    // Verify parent exists
    if store.get_space(&parent_id)?.is_none() {
        return Err(ButlerError::SpaceNotFound(parent_id));
    }

    let meta = SpaceMeta::new(name, owner_did).with_parent(parent_id);
    let data = SpaceData::new(meta.clone());
    store.put_space(&data)?;
    Ok(Space::from(meta))
}

/// Get a space by ID
pub fn get_space(store: &RedbStore, space_id: &str) -> Result<Option<Space>> {
    Ok(store.get_space(space_id)?.map(Space::from))
}

/// Get space with its shares
pub fn get_space_with_shares(store: &RedbStore, space_id: &str) -> Result<Option<SpaceData>> {
    store.get_space(space_id)
}

/// List all root spaces
pub fn list_root_spaces(store: &RedbStore) -> Result<Vec<Space>> {
    Ok(store.list_root_spaces()?
        .into_iter()
        .map(Space::from)
        .collect())
}

/// List child spaces of a parent
pub fn list_child_spaces(store: &RedbStore, parent_id: &str) -> Result<Vec<Space>> {
    Ok(store.list_child_spaces(parent_id)?
        .into_iter()
        .map(Space::from)
        .collect())
}

/// List all spaces
pub fn list_all_spaces(store: &RedbStore) -> Result<Vec<Space>> {
    Ok(store.list_spaces()?
        .into_iter()
        .map(Space::from)
        .collect())
}

/// Delete a space (and optionally its pages)
pub fn delete_space(store: &RedbStore, space_id: &str, delete_pages: bool) -> Result<bool> {
    if delete_pages {
        // Delete all pages in this space
        let pages = store.list_pages_in_space(space_id)?;
        for page in pages {
            delete_page(store, space_id, &page.meta.id)?;
        }
    }
    store.delete_space(space_id)
}

/// Track that space was shared with a user (stores pubkey only)
///
/// Note: Actual permits are stored elsewhere (Contact.shares on Node-side)
pub fn share_space(store: &RedbStore, space_id: &str, user_pubkey: String) -> Result<()> {
    let mut space = store.get_space(space_id)?
        .ok_or_else(|| ButlerError::SpaceNotFound(space_id.to_string()))?;
    space.add_share(user_pubkey);
    store.put_space(&space)?;
    Ok(())
}

/// Remove share tracking for a user from space
pub fn unshare_space(store: &RedbStore, space_id: &str, user_pubkey: &str) -> Result<bool> {
    let mut space = store.get_space(space_id)?
        .ok_or_else(|| ButlerError::SpaceNotFound(space_id.to_string()))?;
    let removed = space.remove_share(user_pubkey);
    store.put_space(&space)?;
    Ok(removed)
}

/// Set permit for a space (stores MY permit)
pub fn set_space_permit(store: &RedbStore, space_id: &str, permit: String) -> Result<()> {
    let mut space = store.get_space(space_id)?
        .ok_or_else(|| ButlerError::SpaceNotFound(space_id.to_string()))?;
    space.set_permit(permit);
    store.put_space(&space)?;
    Ok(())
}

/// Store a space received from owner via publish (Node mode)
///
/// **Context**: Node received PublishSpace from owner
/// **We do**: Store space metadata with the delegated permit
pub fn store_published_space(store: &RedbStore, space: &Space, permit: &str) -> Result<()> {
    store_published_space_with_source(store, space, permit, None)
}

/// Store a space received from another peer with source tracking
///
/// **Context**: Viewer received SpaceData from node, or Node received PublishSpace from owner
/// **We do**: Store space metadata with the delegated permit and source node
/// **Note**: source_node_id is set for viewers to know which node to sync back to
pub fn store_published_space_with_source(
    store: &RedbStore,
    space: &Space,
    permit: &str,
    source_node_id: Option<&str>,
) -> Result<()> {
    let meta = SpaceMeta {
        id: space.id.clone(),
        name: space.name.clone(),
        parent_space_id: space.parent_space_id.clone(),
        owner_did: space.owner_did.clone(),
        is_default: space.is_default,
        description: space.description.clone(),
        created_at: space.created_at,
        updated_at: space.updated_at,
    };

    let mut data = if let Some(node_id) = source_node_id {
        SpaceData::with_source(meta, node_id.to_string())
    } else {
        SpaceData::new(meta)
    };
    data.set_permit(permit.to_string());
    store.put_space(&data)?;
    Ok(())
}

/// Mark a space as published to a specific node
///
/// **Context**: Owner received ack that space was published
/// **We do**: Update space metadata to track which nodes have it
pub fn mark_space_published(store: &RedbStore, space_id: &str, node_id: &str) -> Result<()> {
    let mut space = store.get_space(space_id)?
        .ok_or_else(|| ButlerError::SpaceNotFound(space_id.to_string()))?;

    // Add node_id to published_to list (using shares for now - could add separate field)
    // For simplicity, store as "published:{node_id}" in shares
    space.add_share(format!("published:{}", node_id));
    store.put_space(&space)?;
    Ok(())
}

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
/// * `page_type` - Type of page
/// * `layer_names` - List of layer names from permit template (e.g., ["content", "comments"])
/// * `permit_template_json` - JSON template for the page permit (PAGE_TEMPLATE)
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
    page_type: PageType,
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

        // Store at hierarchical key: {page_id}/{layer_name}
        store.put_layer(&page_id, layer_name, &encrypted_layer)?;
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
        .with_type(page_type)
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

        store.put_layer(&page_id, layer_name, &encrypted_layer)?;
    }

    let encrypted_key = encrypt(owner_public_key, &aes_key)
        .map_err(|e| ButlerError::Encryption(e.to_string()))?;

    let mut meta = PageMeta::new(name, space_id, owner_did)
        .with_type(PageType::PrivateChat)
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

/// Get all spaces a user has access to
pub fn get_accessible_spaces(store: &RedbStore, user_did: &str) -> Result<Vec<Space>> {
    let all_spaces = store.list_spaces()?;
    Ok(all_spaces
        .into_iter()
        .filter(|s| s.meta.owner_did == user_did || s.has_share(user_did))
        .map(Space::from)
        .collect())
}

/// Get all pages (no filtering - if stored, user has access)
///
/// On viewer side: if page is in DB, viewer received it from an authorized node
/// On owner side: if page is in DB, owner created it or has permit for it
pub fn get_pages(store: &RedbStore) -> Result<Vec<Page>> {
    let all_pages = store.list_all_pages()?;
    Ok(all_pages.into_iter().map(Page::from).collect())
}

/// Check if a user has access to a space
pub fn has_space_access(store: &RedbStore, space_id: &str, user_did: &str) -> Result<bool> {
    if let Some(space) = store.get_space(space_id)? {
        Ok(space.meta.owner_did == user_did || space.has_share(user_did))
    } else {
        Ok(false)
    }
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

// =========================================================================
// Publishing Operations
// =========================================================================

/// Prepare space for publishing to node
///
/// **Context**: Owner wants to publish space to their node
/// **We do**: Get space, get owner's permit, issue node permit via gurkha
/// **We return**: (SpaceMeta, permit) - Courier transforms to transport types
///
/// # Arguments
/// * `store` - RedbStore reference
/// * `space_id` - Space to publish
/// * `_owner_did` - Owner's DID (unused, permit is on space directly)
/// * `node_public_key` - Node's public key for permit audience
/// * `owner_signing_key` - Owner's Ed25519 signing key
pub async fn prepare_space_for_publish(
    store: &RedbStore,
    space_id: &str,
    _owner_did: &str,
    node_public_key: &str,
    owner_signing_key: &[u8; 32],
) -> Result<(SpaceMeta, String)> {
    // 1. Get space data
    let space_data = store.get_space(space_id)?
        .ok_or_else(|| ButlerError::SpaceNotFound(space_id.to_string()))?;

    // 2. Get owner's permit from the space (MY permit)
    let owner_permit = space_data.get_permit()
        .ok_or_else(|| ButlerError::PermitNotFound(
            format!("Owner permit not found for space {}", space_id)
        ))?;

    // 3. Issue space permit for node via gurkha
    let (node_permit, _cid) = gurkha::delegate_space(
        owner_signing_key,
        owner_permit,
        "node",  // template_key from SPACE_TEMPLATE.delegation.node
        node_public_key,
    ).await.map_err(|e| ButlerError::Permit(format!("Failed to issue space permit: {}", e)))?;

    Ok((space_data.meta, node_permit))
}

/// Prepare page for publishing to node
///
/// **Context**: Owner wants to publish page to their node
/// **We do**:
///   1. Get page and owner's permit
///   2. Delegate permit to node
///   3. Parse permit to get local_only layers
///   4. Decrypt all layers with AES key
///   5. Filter out local_only layers
///   6. Re-encrypt for transit using ephemeral ECDH
/// **We return**: PreparedPage with transit-encrypted layers
///
/// # Arguments
/// * `store` - RedbStore reference
/// * `page_id` - Page to publish
/// * `_owner_did` - Owner's DID (unused, permit is on page directly)
/// * `node_public_key` - Node's Ed25519 signing public key (for permit audience)
/// * `node_encryption_key` - Node's X25519 encryption public key (for transit encryption)
/// * `owner_signing_key` - Owner's Ed25519 signing key
/// * `owner_secret_key` - Owner's X25519 secret key for decrypting AES key
pub async fn prepare_page_for_publish(
    store: &RedbStore,
    page_id: &str,
    _owner_did: &str,
    node_public_key: &str,
    node_encryption_key: &[u8; 32],
    owner_signing_key: &[u8; 32],
    owner_secret_key: &[u8; 32],
) -> Result<PreparedPage> {
    // 1. Find the page
    let page_data = store.find_page_by_id(page_id)?
        .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;

    // 2. Get owner's permit from the page (MY permit)
    let owner_permit = page_data.get_permit()
        .ok_or_else(|| ButlerError::PermitNotFound(
            format!("Owner permit not found for page {}", page_id)
        ))?;

    // 3. Parse permit to get local_only layers (for filtering)
    let permit = gurkha::Permit::from_token(owner_permit)
        .map_err(|e| ButlerError::Permit(format!("Failed to parse permit: {:?}", e)))?;
    let local_only_layers: Vec<&String> = permit.sync_facts().local_only.iter().collect();

    // 4. Issue page permit for node via gurkha
    let (node_permit, _cid) = gurkha::delegate_page(
        owner_signing_key,
        owner_permit,
        "node",  // template_key from PAGE_TEMPLATE.delegation.node
        node_public_key,
    ).await.map_err(|e| ButlerError::Permit(format!("Failed to issue page permit: {}", e)))?;

    // 5. Decrypt AES key using owner's secret key
    let aes_key = herald::decrypt(owner_secret_key, &page_data.meta.encrypted_key)
        .map_err(|e| ButlerError::Encryption(format!("Failed to decrypt AES key: {}", e)))?;

    let aes_key_arr: [u8; 32] = aes_key.try_into()
        .map_err(|_| ButlerError::Encryption("Invalid AES key length".to_string()))?;

    // 6. Get and decrypt all layers, filtering out local_only
    let encrypted_layers = store.get_all_layers(page_id)?;
    let mut decrypted_layers = Vec::new();

    for (layer_name, encrypted_bytes) in encrypted_layers {
        // Skip local_only layers (e.g., user_content_doc)
        if local_only_layers.iter().any(|l| *l == &layer_name) {
            // Skip local_only layers (e.g., user_content_doc) - not synced to node
            continue;
        }

        let decrypted_bytes = herald::decrypt_symmetric(&aes_key_arr, &encrypted_bytes)
            .map_err(|e| ButlerError::Encryption(
                format!("Failed to decrypt layer {}: {}", layer_name, e)
            ))?;
        decrypted_layers.push((layer_name, decrypted_bytes));
    }

    // 7. Generate ephemeral keypair and encrypt layers for transit
    let (ephemeral_secret, ephemeral_public) = herald::generate_ephemeral_keypair();

    // Derive transit key from ECDH
    let shared_secret = herald::ecdh(&ephemeral_secret, node_encryption_key);
    let transit_key = herald::derive_key(&shared_secret, None, b"herald-transit-v1");

    // Encrypt each layer with transit key
    let mut transit_layers = Vec::with_capacity(decrypted_layers.len());
    for (layer_name, plaintext) in decrypted_layers {
        let encrypted = herald::encrypt_symmetric(&transit_key, &plaintext)
            .map_err(|e| ButlerError::Encryption(
                format!("Failed to transit-encrypt layer {}: {}", layer_name, e)
            ))?;
        transit_layers.push((layer_name, encrypted));
    }

    Ok(PreparedPage {
        meta: page_data.meta,
        permit: node_permit,
        ephemeral_public,
        layers: transit_layers,
    })
}

/// Store a page received via publish (Node mode)
///
/// **Context**: Node received PublishPage from owner
/// **We do**:
///   1. Decrypt layers using ECDH (ephemeral_public + our secret key)
///   2. Generate new AES key for storage
///   3. Re-encrypt layers with new AES key
///   4. Encrypt AES key with Node's encryption public key (ECIES)
///   5. Store page metadata with encrypted_key and permit
///   6. Store all layers
///
/// # Arguments
/// * `store` - RedbStore reference
/// * `page_meta` - Page metadata from owner
/// * `permit` - Node's permit for this page
/// * `ephemeral_public` - Owner's ephemeral X25519 public key
/// * `transit_layers` - Transit-encrypted layers
/// * `node_secret_key` - Node's X25519 secret key for decryption
/// * `node_public_key` - Node's X25519 public key for AES key encryption
/// * `source_node_did` - Optional: DID of node that sent this page (for viewer tracking)
pub fn store_published_page(
    store: &RedbStore,
    page_meta: PageMeta,
    permit: &str,
    ephemeral_public: &[u8; 32],
    transit_layers: Vec<(String, Vec<u8>)>,
    node_secret_key: &[u8; 32],
    node_public_key: &[u8; 32],
    source_node_did: Option<&str>,
) -> Result<Page> {
    // 1. Derive transit key from ECDH to decrypt
    let shared_secret = herald::ecdh(node_secret_key, ephemeral_public);
    let transit_key = herald::derive_key(&shared_secret, None, b"herald-transit-v1");

    // 2. Decrypt all transit layers
    let mut decrypted_layers = Vec::with_capacity(transit_layers.len());
    for (layer_name, encrypted) in transit_layers {
        let decrypted = herald::decrypt_symmetric(&transit_key, &encrypted)
            .map_err(|e| ButlerError::Encryption(
                format!("Failed to decrypt transit layer {}: {}", layer_name, e)
            ))?;
        decrypted_layers.push((layer_name, decrypted));
    }

    // 3. Generate new AES key for storage on Node
    let aes_key = herald::generate_aes_key();

    // 4. Re-encrypt layers with new AES key
    for (layer_name, plaintext) in &decrypted_layers {
        let encrypted = herald::encrypt_symmetric(&aes_key, plaintext)
            .map_err(|e| ButlerError::Encryption(
                format!("Failed to encrypt layer {}: {}", layer_name, e)
            ))?;
        store.put_layer(&page_meta.id, layer_name, &encrypted)?;
    }

    // 5. Encrypt AES key with Node's encryption public key (ECIES)
    let encrypted_key = herald::encrypt(node_public_key, &aes_key)
        .map_err(|e| ButlerError::Encryption(format!("Failed to encrypt AES key: {}", e)))?;

    // 6. Create page metadata with encrypted key
    let mut meta = page_meta;
    meta.encrypted_key = encrypted_key;

    // 7. Store page with permit
    let mut data = PageData::new(meta.clone());
    data.set_permit(permit.to_string());

    // Track source node in shares (viewer uses this to know where to get updates)
    if let Some(node_did) = source_node_did {
        data.add_share(node_did.to_string());
    }

    store.put_page(&data)?;

    Ok(Page::from(meta))
}

/// Prepare page for viewer (Node mode)
///
/// **Context**: Node is responding to viewer's SpaceRequest
/// **We do**:
///   1. Get page and node's permit
///   2. Delegate permit to viewer using "viewer" template
///   3. Filter out local_only layers
///   4. Encrypt layers for transit using ephemeral ECDH
///
/// # Arguments
/// * `store` - RedbStore reference
/// * `page_id` - Page to prepare
/// * `viewer_public_key` - Viewer's Ed25519 signing public key (for permit audience)
/// * `viewer_encryption_key` - Viewer's X25519 encryption public key (for transit encryption)
/// * `node_signing_key` - Node's Ed25519 signing key
/// * `node_secret_key` - Node's X25519 secret key for decrypting AES key
pub async fn prepare_page_for_viewer(
    store: &RedbStore,
    page_id: &str,
    viewer_public_key: &str,
    viewer_encryption_key: &[u8; 32],
    node_signing_key: &[u8; 32],
    node_secret_key: &[u8; 32],
) -> Result<PreparedPage> {
    // 1. Find the page
    let page_data = store.find_page_by_id(page_id)?
        .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;

    // 2. Get node's permit from the page
    let node_permit = page_data.get_permit()
        .ok_or_else(|| ButlerError::PermitNotFound(
            format!("Node permit not found for page {}", page_id)
        ))?;

    // 3. Parse permit to get local_only layers (for filtering)
    let permit = gurkha::Permit::from_token(node_permit)
        .map_err(|e| ButlerError::Permit(format!("Failed to parse permit: {:?}", e)))?;
    let local_only_layers: Vec<&String> = permit.sync_facts().local_only.iter().collect();

    // 4. Issue page permit for viewer via gurkha
    let (viewer_permit, _cid) = gurkha::delegate_page(
        node_signing_key,
        node_permit,
        "viewer",  // template_key from PAGE_TEMPLATE.delegation.viewer
        viewer_public_key,
    ).await.map_err(|e| ButlerError::Permit(format!("Failed to issue viewer permit: {}", e)))?;

    // 5. Decrypt AES key using node's secret key
    let aes_key = herald::decrypt(node_secret_key, &page_data.meta.encrypted_key)
        .map_err(|e| ButlerError::Encryption(format!("Failed to decrypt AES key: {}", e)))?;

    let aes_key_arr: [u8; 32] = aes_key.try_into()
        .map_err(|_| ButlerError::Encryption("Invalid AES key length".to_string()))?;

    // 6. Get and decrypt all layers, filtering out local_only
    let encrypted_layers = store.get_all_layers(page_id)?;
    let mut decrypted_layers = Vec::new();

    for (layer_name, encrypted_bytes) in encrypted_layers {
        // Skip local_only layers (e.g., user_content_doc) - not synced to viewer
        if local_only_layers.iter().any(|l| *l == &layer_name) {
            continue;
        }

        let decrypted_bytes = herald::decrypt_symmetric(&aes_key_arr, &encrypted_bytes)
            .map_err(|e| ButlerError::Encryption(
                format!("Failed to decrypt layer {}: {}", layer_name, e)
            ))?;
        decrypted_layers.push((layer_name, decrypted_bytes));
    }

    // 7. Generate ephemeral keypair and encrypt layers for transit
    let (ephemeral_secret, ephemeral_public) = herald::generate_ephemeral_keypair();

    // Derive transit key from ECDH
    let shared_secret = herald::ecdh(&ephemeral_secret, viewer_encryption_key);
    let transit_key = herald::derive_key(&shared_secret, None, b"herald-transit-v1");

    // Encrypt each layer with transit key
    let mut transit_layers = Vec::with_capacity(decrypted_layers.len());
    for (layer_name, plaintext) in decrypted_layers {
        let encrypted = herald::encrypt_symmetric(&transit_key, &plaintext)
            .map_err(|e| ButlerError::Encryption(
                format!("Failed to transit-encrypt layer {}: {}", layer_name, e)
            ))?;
        transit_layers.push((layer_name, encrypted));
    }

    Ok(PreparedPage {
        meta: page_data.meta,
        permit: viewer_permit,
        ephemeral_public,
        layers: transit_layers,
    })
}

/// Mark a page as published to a specific node
///
/// **Context**: Owner received ack that page was published
/// **We do**: Update page metadata to track which nodes have it
pub fn mark_page_published(store: &RedbStore, page_id: &str, node_id: &str) -> Result<()> {
    let mut page = store.find_page_by_id(page_id)?
        .ok_or_else(|| ButlerError::PageNotFound(page_id.to_string()))?;

    // Add node_id to shares list to track publishing (using "published:{node_id}" prefix)
    page.add_share(format!("published:{}", node_id));
    store.put_page(&page)?;
    Ok(())
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
