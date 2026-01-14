//! PublishService - Functions for preparing and storing published pages/spaces
//!
//! Handles the complex encryption/decryption flows for P2P sync.

use crate::error::{ButlerError, Result};
use crate::storage::RedbStore;
use crate::models::{
    SpaceMeta, PageMeta, Page, PageData, PreparedPage,
};

// =========================================================================
// Space Publishing Operations
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

// =========================================================================
// Page Publishing Operations
// =========================================================================

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

    // 2. Get owner's permit from the page (MY permit) - clone to avoid borrow issues
    let owner_permit = page_data.get_permit()
        .ok_or_else(|| ButlerError::PermitNotFound(
            format!("Owner permit not found for page {}", page_id)
        ))?
        .clone();

    // 3. Parse permit to get local_only layers (for filtering)
    let permit = gurkha::Permit::from_token(&owner_permit)
        .map_err(|e| ButlerError::Permit(format!("Failed to parse permit: {:?}", e)))?;
    let local_only_layers: Vec<String> = permit.local_only_layers();

    // 4. Issue page permit for node via gurkha
    let (node_permit, _cid) = gurkha::delegate_page(
        owner_signing_key,
        &owner_permit,
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
        if local_only_layers.iter().any(|l| l == &layer_name) {
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
        owner_permit,
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
/// * `sender_did` - DID of the peer sending this page (for state vector storage)
/// * `sender_device_id` - Device ID of the sender (for state vector storage)
pub fn store_published_page(
    store: &RedbStore,
    page_meta: PageMeta,
    permit: &str,
    ephemeral_public: &[u8; 32],
    transit_layers: Vec<(String, Vec<u8>)>,
    node_secret_key: &[u8; 32],
    node_public_key: &[u8; 32],
    source_node_did: Option<&str>,
    sender_did: &str,
    sender_device_id: &str,
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

    // 4. Re-encrypt layers with new AES key and store sender's state vectors
    log::info!("store_published_page: page_id={} storing {} layers",
        page_meta.id, decrypted_layers.len());
    for (layer_name, plaintext) in &decrypted_layers {
        log::info!("store_published_page: storing layer {} ({} bytes plaintext)",
            layer_name, plaintext.len());
        let encrypted = herald::encrypt_symmetric(&aes_key, plaintext)
            .map_err(|e| ButlerError::Encryption(
                format!("Failed to encrypt layer {}: {}", layer_name, e)
            ))?;
        store.put_layer(&page_meta.id, layer_name, &encrypted)?;

        // Store sender's state vector so we can send incremental updates later
        let temp_doc = loro::LoroDoc::new();
        if temp_doc.import(plaintext).is_ok() {
            let vector = temp_doc.oplog_vv().encode();
            let _ = store.put_peer_vector_for_layer(
                &page_meta.id,
                sender_did,
                sender_device_id,
                layer_name,
                vector,
            );
        }
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

    // Register sender (owner) as authorized user for this page
    // This allows can_write() in Scribe to accept SyncOffer from owner
    // Compute CID from permit for revocation tracking
    let permit_cid = gurkha::crypto::get_permit_cid(permit)
        .unwrap_or_else(|_| "unknown".to_string());
    store.put_permit_cid(&meta.id, sender_did, &permit_cid)?;

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

    // 2. Get node's permit from the page - clone to avoid borrow issues
    let node_permit = page_data.get_permit()
        .ok_or_else(|| ButlerError::PermitNotFound(
            format!("Node permit not found for page {}", page_id)
        ))?
        .clone();

    // 3. Parse permit to get local_only layers (for filtering)
    let permit = gurkha::Permit::from_token(&node_permit)
        .map_err(|e| ButlerError::Permit(format!("Failed to parse permit: {:?}", e)))?;
    let local_only_layers: Vec<String> = permit.local_only_layers();

    // 4. Issue page permit for viewer via gurkha
    let (viewer_permit, _cid) = gurkha::delegate_page(
        node_signing_key,
        &node_permit,
        "viewer",  // template_key from PAGE_TEMPLATE.delegation.viewer
        viewer_public_key,
    ).await.map_err(|e| ButlerError::Permit(format!("Failed to issue viewer permit: {}", e)))?;

    // 4a. Parse viewer permit for pattern-based filtering
    let viewer_permit_parsed = gurkha::Permit::from_token(&viewer_permit)
        .map_err(|e| ButlerError::Permit(format!("Failed to parse viewer permit: {:?}", e)))?;

    // 5. Decrypt AES key using node's secret key
    let aes_key = herald::decrypt(node_secret_key, &page_data.meta.encrypted_key)
        .map_err(|e| ButlerError::Encryption(format!("Failed to decrypt AES key: {}", e)))?;

    let aes_key_arr: [u8; 32] = aes_key.try_into()
        .map_err(|_| ButlerError::Encryption("Invalid AES key length".to_string()))?;

    // 5a. Auto-create viewer's namespace layers from layer_patterns with create=true
    // Pattern like "{page_id}/orders/{aud}" with create=true means viewer gets their own layer
    let viewer_aud_b64 = viewer_permit_parsed.core().audience().unwrap_or("");
    // Convert base64 audience to DID format for layer naming consistency
    let viewer_did = herald::Identity::did_from_base64_pubkey(viewer_aud_b64)
        .unwrap_or_else(|_| viewer_aud_b64.to_string());

    for (pattern, config) in viewer_permit_parsed.layer_patterns() {
        if config.create {
            // Expand pattern with actual values
            // Use DID format for {aud} so layer names are human-readable
            let expanded = pattern
                .replace("{page_id}", page_id)
                .replace("{aud}", &viewer_did);

            // Only create if pattern doesn't have wildcards after expansion
            // e.g., "{page_id}/orders/{aud}" → "shop123/orders/did:key:xyz" (createable)
            // e.g., "{page_id}/*/*" → "shop123/*/*" (not createable, has wildcards)
            if !expanded.contains('*') && !expanded.contains('{') {
                // Check if layer already exists
                if store.get_layer(page_id, &expanded)?.is_none() {
                    tracing::info!(
                        "Auto-creating viewer namespace layer '{}' for viewer {}",
                        expanded, viewer_did
                    );
                    // Create empty LoroDoc for the layer
                    let empty_doc = loro::LoroDoc::new();
                    let empty_bytes = empty_doc.export(loro::ExportMode::Snapshot)
                        .map_err(|e| ButlerError::Storage(format!("Failed to create empty layer: {}", e)))?;
                    let encrypted = herald::encrypt_symmetric(&aes_key_arr, &empty_bytes)
                        .map_err(|e| ButlerError::Encryption(
                            format!("Failed to encrypt auto-created layer {}: {}", expanded, e)
                        ))?;
                    store.put_layer(page_id, &expanded, &encrypted)?;
                }
            }
        }
    }

    // 6. Get and decrypt all layers, filtering by viewer's permit access
    let encrypted_layers = store.get_all_layers(page_id)?;
    let mut decrypted_layers = Vec::new();

    for (layer_name, encrypted_bytes) in encrypted_layers {
        // Skip local_only layers (e.g., user_content_doc) - not synced to viewer
        if local_only_layers.iter().any(|l| l == &layer_name) {
            tracing::debug!("Filtering out local_only layer '{}'", layer_name);
            continue;
        }

        // NEW: Filter by viewer's permit patterns
        // Only send layers the viewer has access to sync
        if !gurkha::can_access_layer(&viewer_permit_parsed, &layer_name, "sync") {
            tracing::debug!(
                "Filtering out layer '{}' - viewer {} doesn't have sync access",
                layer_name, viewer_did
            );
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
        owner_permit: node_permit,  // Node's permit - used for sync authorization
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
