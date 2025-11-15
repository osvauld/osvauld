//! Core reusable building blocks for resource operations
//!
//! This module contains common patterns extracted from resource_service, merge_service,
//! and website_service. These functions eliminate ~400 lines of duplication.
//!
//! ## Patterns Implemented:
//! 1. Load-Decrypt-Parse: Generic resource loading by token
//! 2. Update-Encrypt-Save: Resource encryption and database updates
//! 3. State-Vector-Generation: CRDT state vector JSON building
//! 4. Filter-and-Re-encrypt: Permission-based filtering with re-encryption

use crate::errors::{ResourceServiceError, ServiceError, ServiceResult};
use crypto_utils::{encrypt_data_for_user, CryptoUtils};
use log::{error, info};
use osvauld_core::models::{
    document::{create_doc, state_frontiers},
    resource::{EncryptedResource, Resource}, ResourceShareToken,
    SyncContext, SyncDecision,
};
use persistance::database::RepositoryContext;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

// =============================================================================
// PATTERN 1: Load-Decrypt-Parse
// =============================================================================

/// Load resource by ResourceOwnerToken
/// Load resource by ResourceShareToken
pub async fn load_and_decrypt_by_share_token(
    token: &ResourceShareToken,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Resource> {
    let resource_id = token.resource_id();
    load_and_decrypt_resource(&resource_id, repo_ctx, crypto_utils).await
}

/// Load resource directly by ID (no token)
///
/// Used for internal operations where resource_id is already known.
///
/// # Arguments
/// * `resource_id` - Resource ID
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// * `Resource` - Decrypted resource
pub async fn load_and_decrypt_resource(
    resource_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Resource> {
    info!("Loading resource by ID: {}", resource_id);

    let encrypted_resource = repo_ctx.resource_repo.find_by_id(resource_id).await?;
    decrypt_encrypted_resource(&encrypted_resource, crypto_utils).await
}

/// Decrypt single encrypted resource
///
/// Internal helper for decryption logic.
async fn decrypt_encrypted_resource(
    encrypted_resource: &EncryptedResource,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Resource> {
    let crypto = crypto_utils.read().await;

    // Decrypt the encrypted_data field
    let decrypted_json = crypto
        .decrypt_resource(
            &encrypted_resource.encrypted_data,
            &encrypted_resource.encrypted_key,
        )
        .map_err(|e| {
            error!(
                "Failed to decrypt resource {}: {}",
                encrypted_resource.id, e
            );
            ResourceServiceError::DecryptionFailed(encrypted_resource.id.clone())
        })?;

    // Parse JSON into Resource struct
    Resource::from_decrypted_data(
        encrypted_resource.id.clone(),
        encrypted_resource.folder_id.clone(),
        encrypted_resource.ucan_token.clone(),
        encrypted_resource.metadata.clone(),
        &decrypted_json,
    )
    .map_err(|e| {
        error!(
            "Failed to parse resource {} JSON: {}",
            encrypted_resource.id, e
        );
        ServiceError::Resource(ResourceServiceError::InvalidResourceData(encrypted_resource.id.clone()))
    })
}

/// Batch decrypt resources
///
/// Used when fetching multiple resources (e.g., folder listing).
///
/// # Arguments
/// * `encrypted_resources` - Vector of encrypted resources
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// * `Vec<Resource>` - Decrypted resources
// =============================================================================
// PATTERN 2: Update-Encrypt-Save
// =============================================================================

/// Encrypt resource data and save to database
///
/// Eliminates the Update-Encrypt-Save pattern that appears 3+ times.
/// Uses key rotation for forward secrecy.
///
/// # Arguments
/// * `resource` - Resource to save
/// * `user_public_key` - User's public key for encryption
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `Ok(())` on success
pub async fn encrypt_and_save_resource(
    resource: &Resource,
    user_public_key: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    // Serialize resource to JSON
    let data = resource.to_json().map_err(|e| {
        error!("Failed to serialize resource {}: {}", resource.id, e);
        ResourceServiceError::SerializationError(format!(
            "Failed to serialize resource: {}",
            e
        ))
    })?;

    // Generate NEW AES key and encrypt data (key rotation for forward secrecy)
    let (encrypted_data, encrypted_key) = encrypt_data_for_user(&data, user_public_key)
        .map_err(|e| ResourceServiceError::EncryptionFailed(e.to_string()))?;

    // Update resource in database with new encrypted data and key
    repo_ctx
        .resource_repo
        .update_resource(&encrypted_data, &encrypted_key, &resource.id)
        .await
        .map_err(|e| {
            error!(
                "Failed to update resource {} in database: {}",
                resource.id, e
            );
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    info!("Successfully saved resource {}", resource.id);
    Ok(())
}

// =============================================================================
// PATTERN 3: State Vector Generation
// =============================================================================

/// Build state vectors JSON for documents matching filter
///
/// Eliminates the Generate-State-Vectors pattern that appears 4+ times.
///
/// # Arguments
/// * `resource` - Resource containing documents
/// * `doc_filter` - Closure that returns true for docs to include
///
/// # Returns
/// * `String` - JSON string with state vectors
///
/// # Example
/// ```ignore
/// let json = build_state_vectors_json(resource, |doc_name| {
///     sync_context.should_send(doc_name) != SyncDecision::DontSend
/// })?;
/// ```
pub fn build_state_vectors_json(
    resource: &Resource,
    doc_filter: impl Fn(&str) -> bool,
) -> ServiceResult<String> {
    let mut result = serde_json::Map::new();

    for doc_name in resource.doc_names() {
        // Apply filter
        if !doc_filter(doc_name.as_str()) {
            continue;
        }

        // Get state vector for this doc
        if let Some(doc) = resource.get_doc(doc_name.as_str()) {
            let state_vector = state_frontiers(doc);

            let vector_array: Vec<Value> = state_vector
                .iter()
                .map(|&b| Value::Number(serde_json::Number::from(b)))
                .collect();

            let mut doc_data = serde_json::Map::new();
            doc_data.insert("state_vector".to_string(), Value::Array(vector_array));

            result.insert(doc_name.to_string(), Value::Object(doc_data));
        }
    }

    serde_json::to_string(&result).map_err(|e| {
        error!("Failed to serialize state vectors: {}", e);
        ServiceError::Resource(ResourceServiceError::SerializationError(format!(
            "Failed to serialize state vectors: {}",
            e
        )))
    })
}

// =============================================================================
// PATTERN 4: Filter and Re-encrypt
// =============================================================================

/// Filter resource documents and encrypt for peer
///
/// Eliminates the Filter-and-Re-encrypt pattern that appears 3+ times.
/// Uses SyncContext to determine which documents to send.
///
/// # Arguments
/// * `resource` - Resource containing documents
/// * `sync_context` - Dual-UCAN context for permission filtering
/// * `peer_public_key` - Peer's public key for encryption
///
/// # Returns
/// * `(encrypted_data, encrypted_key)` - Base64-encoded encrypted filtered resource
pub async fn filter_and_encrypt_for_peer(
    resource: &Resource,
    sync_context: &SyncContext,
    peer_public_key: &str,
) -> ServiceResult<(String, String)> {
    // Create filtered resource with only documents peer can access
    let mut filtered_resource = Resource::new(resource.id.clone());

    for doc_name in resource.doc_names() {
        let decision = osvauld_core::models::should_send_updates(sync_context, doc_name.as_str());

        // Include document if we should send it
        if decision != SyncDecision::DontSend {
            if resource.get_doc(doc_name.as_str()).is_some() {
                // Clone the document for the filtered resource
                filtered_resource.add_doc(doc_name.to_string(), create_doc());
                // TODO: Copy document data properly (needs LoroDoc clone)
            }
        }
    }

    // Serialize filtered resource
    let filtered_json = filtered_resource.to_json().map_err(|e| {
        error!("Failed to serialize filtered resource: {}", e);
        ResourceServiceError::SerializationError(format!(
            "Failed to serialize filtered resource: {}",
            e
        ))
    })?;

    // Encrypt for peer (returns base64 strings directly - no need to decode/re-encode)
    let (encrypted_data, encrypted_key) = encrypt_data_for_user(&filtered_json, peer_public_key)
        .map_err(|e| ResourceServiceError::EncryptionFailed(e.to_string()))?;

    Ok((encrypted_data, encrypted_key))
}

// =============================================================================
// HELPER: Create SyncContext
// =============================================================================

/// Create SyncContext from two raw UCAN tokens
///
/// Convenience helper for creating sync contexts.
///
/// # Arguments
/// * `our_token` - Our UCAN token string
/// * `peer_token` - Peer's UCAN token string
///
/// # Returns
/// * `SyncContext` - Context for sync permission decisions
pub async fn create_sync_context(
    our_token: &str,
    peer_token: &str,
) -> ServiceResult<SyncContext> {
    SyncContext::new(our_token, peer_token).map_err(|e| {
        error!("Failed to create SyncContext: {}", e);
        ServiceError::InvalidUcan(format!("Failed to create SyncContext: {}", e))
    })
}
