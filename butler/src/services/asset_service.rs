//! Asset Service - Upload, storage, and sync operations for encrypted assets
//!
//! Assets are:
//! - Stored encrypted at rest in AssetStore (`~/.osvauld/assets/{hash}.enc`)
//! - Tracked via metadata in Loro `{page_id}/assets` layer
//! - Signed by uploader for integrity verification
//! - Re-signed by node when relaying to viewers

use crate::error::{ButlerError, Result};
use crate::models::AssetMetadata;
use crate::storage::AssetStore;
use herald::Identity;
use tracing::{info, instrument};

/// Derive an asset-specific encryption key from page key and asset hash
///
/// Uses HKDF to derive a unique key per asset, so each asset has its own encryption key.
/// This prevents issues if the same plaintext is uploaded to different pages.
#[instrument(skip_all)]
fn derive_asset_key(page_key: &[u8; 32], asset_hash: &str) -> [u8; 32] {
    let info = format!("osvauld-asset-key-v1:{}", asset_hash);
    herald::derive_key(page_key, None, info.as_bytes())
}

/// Upload an asset to local storage
///
/// **Context**: User uploads a file (image, PDF, etc.) to a page
/// **Flow**:
///   1. Compute blake3 hash of plaintext
///   2. Sign the hash with user's signing key
///   3. Derive asset-specific encryption key from page key
///   4. Encrypt and store in AssetStore
///   5. Return metadata (caller stores in Loro layer)
///
/// # Arguments
/// * `asset_store` - Filesystem storage for encrypted assets
/// * `page_key` - The page's AES key (already decrypted by caller)
/// * `identity` - User's identity for signing
/// * `filename` - Original filename
/// * `mime_type` - MIME type (e.g., "image/png")
/// * `data` - Raw plaintext bytes
///
/// # Returns
/// AssetMetadata ready to be stored in Loro layer
#[instrument(skip(asset_store, page_key, identity, data), fields(filename = %filename, mime_type = %mime_type))]
pub fn upload_asset(
    asset_store: &AssetStore,
    page_key: &[u8; 32],
    identity: &Identity,
    filename: &str,
    mime_type: &str,
    data: &[u8],
) -> Result<AssetMetadata> {
    // 1. Compute blake3 hash of plaintext
    let hash = blake3::hash(data).to_hex().to_string();

    info!(
        hash = %hash,
        filename = %filename,
        size = data.len(),
        "Uploading asset"
    );

    // 2. Sign the hash with user's signing key
    let hash_bytes = hash.as_bytes();
    let signature_bytes = identity.sign(hash_bytes);
    let signature =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, signature_bytes);

    // 3. Derive asset-specific encryption key
    let asset_key = derive_asset_key(page_key, &hash);

    // 4. Encrypt and store
    let ciphertext = herald::encrypt_symmetric(&asset_key, data)
        .map_err(|e| ButlerError::Encryption(e.to_string()))?;
    asset_store.put(&hash, &ciphertext)?;

    // 5. Create and return metadata
    let metadata = AssetMetadata::new(
        hash,
        signature,
        filename.to_string(),
        mime_type.to_string(),
        data.len() as u64,
        identity.did().to_string(),
    );

    info!(
        hash = %metadata.hash,
        created_by = %metadata.created_by,
        "Asset uploaded successfully"
    );

    Ok(metadata)
}

/// Get an asset's plaintext from local storage
///
/// **Context**: User/app wants to display or use an asset
/// **Flow**:
///   1. Read encrypted bytes from AssetStore
///   2. Derive asset-specific key from page key
///   3. Decrypt and return plaintext
///
/// # Arguments
/// * `asset_store` - Filesystem storage for encrypted assets
/// * `page_key` - The page's AES key
/// * `hash` - Blake3 hash of the asset (also its ID)
///
/// # Returns
/// Decrypted plaintext bytes
#[instrument(skip(asset_store, page_key), fields(hash = %hash))]
pub fn get_asset(asset_store: &AssetStore, page_key: &[u8; 32], hash: &str) -> Result<Vec<u8>> {
    // 1. Read encrypted bytes
    let ciphertext = asset_store
        .get(hash)?
        .ok_or_else(|| ButlerError::NotFound(format!("Asset not found: {}", hash)))?;

    // 2. Derive asset-specific key
    let asset_key = derive_asset_key(page_key, hash);

    // 3. Decrypt and return
    let plaintext = herald::decrypt_symmetric(&asset_key, &ciphertext)
        .map_err(|e| ButlerError::Encryption(format!("Failed to decrypt asset: {}", e)))?;

    Ok(plaintext)
}

/// Get asset plaintext for transfer (iroh-blobs)
///
/// **Context**: Peer requests asset, we decrypt for transfer
/// **Note**: Same as get_asset - the peer will encrypt with their own key after receiving
#[instrument(skip(asset_store, page_key), fields(hash = %hash))]
pub fn get_for_transfer(
    asset_store: &AssetStore,
    page_key: &[u8; 32],
    hash: &str,
) -> Result<Vec<u8>> {
    get_asset(asset_store, page_key, hash)
}

/// Store a received asset (after transfer from peer)
///
/// **Context**: We received plaintext via iroh-blobs, now store encrypted
/// **Flow**:
///   1. Verify blake3 hash matches
///   2. Verify signature from metadata
///   3. Derive our encryption key and store
///
/// # Arguments
/// * `asset_store` - Filesystem storage for encrypted assets
/// * `page_key` - Our page's AES key
/// * `metadata` - Asset metadata (from Loro sync)
/// * `plaintext` - Received plaintext bytes
///
/// # Returns
/// () on success, error if verification fails
#[instrument(skip_all)]
pub fn store_received(
    asset_store: &AssetStore,
    page_key: &[u8; 32],
    metadata: &AssetMetadata,
    plaintext: &[u8],
) -> Result<()> {
    // 1. Verify hash
    let computed_hash = blake3::hash(plaintext).to_hex().to_string();
    if computed_hash != metadata.hash {
        return Err(ButlerError::verification_failed(format!(
            "Hash mismatch: expected {}, got {}",
            metadata.hash, computed_hash
        )));
    }

    // 2. Verify signature
    if !verify_asset_signature(metadata)? {
        return Err(ButlerError::verification_failed("Invalid asset signature"));
    }

    info!(
        hash = %metadata.hash,
        created_by = %metadata.created_by,
        "Storing received asset"
    );

    // 3. Derive our key and store
    let asset_key = derive_asset_key(page_key, &metadata.hash);
    let ciphertext = herald::encrypt_symmetric(&asset_key, plaintext)
        .map_err(|e| ButlerError::Encryption(e.to_string()))?;
    asset_store.put(&metadata.hash, &ciphertext)?;

    Ok(())
}

/// Verify an asset's signature against the creator's public key
///
/// **Context**: Validate that the asset was signed by the claimed creator
#[instrument(skip_all)]
fn verify_asset_signature(metadata: &AssetMetadata) -> Result<bool> {
    // Decode signature from base64
    let signature_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &metadata.signature,
    )
    .map_err(|e| ButlerError::verification_failed(format!("Invalid signature encoding: {}", e)))?;

    if signature_bytes.len() != 64 {
        return Err(ButlerError::verification_failed("Invalid signature length"));
    }

    let signature: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| ButlerError::verification_failed("Invalid signature length"))?;

    // Get public key from creator's DID
    let public_key = Identity::public_key_from_did(&metadata.created_by)
        .map_err(|e| ButlerError::verification_failed(format!("Invalid creator DID: {}", e)))?;

    // Verify signature
    let hash_bytes = metadata.hash.as_bytes();
    Ok(Identity::verify_with_key(
        &public_key,
        hash_bytes,
        &signature,
    ))
}

/// Re-sign an asset with node's key (for relaying to viewers)
///
/// **Context**: Node relaying asset to viewer needs to sign with its own key
/// **Flow**:
///   1. Verify original signature first
///   2. Sign hash with node's key
///   3. Return updated metadata with new signature
///
/// # Arguments
/// * `metadata` - Original asset metadata
/// * `node_identity` - Node's identity for signing
///
/// # Returns
/// New AssetMetadata with node's signature
#[instrument(skip_all)]
pub fn resign_asset(metadata: &AssetMetadata, node_identity: &Identity) -> Result<AssetMetadata> {
    // Verify original signature first
    if !verify_asset_signature(metadata)? {
        return Err(ButlerError::verification_failed(
            "Cannot re-sign: original signature invalid",
        ));
    }

    // Sign with node's key
    let hash_bytes = metadata.hash.as_bytes();
    let signature_bytes = node_identity.sign(hash_bytes);
    let new_signature =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, signature_bytes);

    // Create new metadata with node's signature
    Ok(AssetMetadata {
        hash: metadata.hash.clone(),
        signature: new_signature,
        filename: metadata.filename.clone(),
        mime_type: metadata.mime_type.clone(),
        size: metadata.size,
        created_by: node_identity.did().to_string(),
        created_at: metadata.created_at,
    })
}

/// Check if an asset exists in local storage
///
/// **Context**: Quick check before requesting transfer from peer
#[instrument(skip(asset_store), fields(hash = %hash))]
pub fn asset_exists(asset_store: &AssetStore, hash: &str) -> bool {
    asset_store.exists(hash)
}

/// Find assets that are in metadata but not in local storage
///
/// **Context**: After syncing metadata via Loro, find which assets need to be fetched
/// **Flow**:
///   1. Takes list of asset hashes from metadata
///   2. Checks each against local AssetStore
///   3. Returns hashes that need to be fetched
///
/// # Arguments
/// * `asset_store` - Filesystem storage for encrypted assets
/// * `metadata_hashes` - List of asset hashes from synced Loro metadata
///
/// # Returns
/// List of hashes that are missing locally
#[instrument(skip_all)]
pub fn list_missing(asset_store: &AssetStore, metadata_hashes: &[String]) -> Vec<String> {
    metadata_hashes
        .iter()
        .filter(|hash| !asset_store.exists(hash))
        .cloned()
        .collect()
}

/// Delete an asset from local storage
///
/// **Context**: Asset removed from page, cleanup local storage
#[instrument(skip(asset_store), fields(hash = %hash))]
pub fn delete_asset(asset_store: &AssetStore, hash: &str) -> Result<bool> {
    asset_store.delete(hash)
}

/// List all locally stored asset hashes
///
/// **Context**: For debugging or garbage collection
#[instrument(skip_all)]
pub fn list_local_assets(asset_store: &AssetStore) -> Result<Vec<String>> {
    asset_store.list()
}

// These functions work with the `{page_id}/assets` Loro layer that stores
// asset metadata (hash → AssetMetadata). The actual encrypted files are
// stored separately in AssetStore.

use crate::models::Layer;
use std::collections::HashMap;

/// Get all asset metadata from a page's assets layer
///
/// **Context**: Read the Loro layer to get metadata for all assets in a page
/// **Layer format**: LoroMap keyed by hash → `{ "hash1": AssetMetadata, "hash2": AssetMetadata }`
///
/// # Arguments
/// * `layer` - The assets layer (already loaded from Scribe)
///
/// # Returns
/// HashMap of hash → AssetMetadata
#[instrument(skip_all)]
pub fn get_assets_from_layer(layer: &Layer) -> HashMap<String, AssetMetadata> {
    // Use get_content("root") to get the unwrapped asset map directly
    let json = layer.get_content("root");

    // The unwrapped content should be the assets map with hash as key
    let Some(assets_map) = json.as_object() else {
        return HashMap::new();
    };

    // Check for nested "root" structure (from MapInsert with path="root")
    // If assets_map["root"] exists and is an object, use that instead
    let final_map = assets_map
        .get("root")
        .and_then(|v| v.as_object())
        .unwrap_or(assets_map);

    let mut result = HashMap::new();
    for (hash, value) in final_map {
        if let Ok(metadata) = serde_json::from_value::<AssetMetadata>(value.clone()) {
            result.insert(hash.clone(), metadata);
        }
    }

    result
}

/// Add asset metadata to an assets layer
///
/// **Context**: After uploading an asset, add its metadata to the layer for sync
/// **Layer format**: LoroMap keyed by hash → AssetMetadata
///
/// # Arguments
/// * `layer` - The assets layer to update
/// * `metadata` - The asset metadata to add
///
/// # Returns
/// () on success
#[instrument(skip_all)]
pub fn add_to_assets_layer(layer: &Layer, metadata: &AssetMetadata) -> Result<()> {
    let map = layer.loro().get_map("root");

    // Serialize metadata to JSON, then to LoroValue
    let json_value = serde_json::to_value(metadata).map_err(|e| {
        ButlerError::Internal(format!("Layer: Failed to serialize metadata: {}", e))
    })?;

    // Convert JSON to LoroValue and insert
    let loro_value = domains::json_to_loro_value(&json_value);
    map.insert(&metadata.hash, loro_value)
        .map_err(|e| ButlerError::Internal(format!("Layer: Failed to insert metadata: {}", e)))?;

    layer.commit();

    info!(
        hash = %metadata.hash,
        filename = %metadata.filename,
        "Added asset metadata to layer"
    );

    Ok(())
}

/// Remove asset metadata from an assets layer
///
/// **Context**: Asset was deleted, remove from metadata layer
///
/// # Arguments
/// * `layer` - The assets layer to update
/// * `hash` - The asset hash to remove
#[instrument(skip(layer), fields(hash = %hash))]
pub fn remove_from_assets_layer(layer: &Layer, hash: &str) -> Result<()> {
    let map = layer.loro().get_map("root");
    map.delete(hash).ok(); // Ignore if doesn't exist
    layer.commit();

    info!(hash = %hash, "Removed asset metadata from layer");
    Ok(())
}

/// Find assets in layer metadata that are missing from local AssetStore
///
/// **Context**: After syncing metadata via Loro, find which assets need to be fetched
/// **Flow**:
///   1. Get all hashes from the assets layer
///   2. Check each against local AssetStore
///   3. Return metadata for assets that need to be fetched
///
/// # Arguments
/// * `asset_store` - Filesystem storage for encrypted assets
/// * `layer` - The assets layer containing metadata
///
/// # Returns
/// List of AssetMetadata for assets that are missing locally
#[instrument(skip_all)]
pub fn find_missing_assets_from_layer(
    asset_store: &AssetStore,
    layer: &Layer,
) -> Vec<AssetMetadata> {
    let all_assets = get_assets_from_layer(layer);

    all_assets
        .into_values()
        .filter(|metadata| !asset_store.exists(&metadata.hash))
        .collect()
}

/// Get list of missing asset hashes from layer metadata
///
/// **Context**: Simpler version that just returns hashes (not full metadata)
///
/// # Arguments
/// * `asset_store` - Filesystem storage for encrypted assets
/// * `layer` - The assets layer containing metadata
///
/// # Returns
/// List of hashes that are missing locally
#[instrument(skip_all)]
pub fn find_missing_asset_hashes(asset_store: &AssetStore, layer: &Layer) -> Vec<String> {
    let all_assets = get_assets_from_layer(layer);

    all_assets
        .keys()
        .filter(|hash| !asset_store.exists(hash))
        .cloned()
        .collect()
}
