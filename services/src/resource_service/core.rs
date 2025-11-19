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
use gurkha::decision::{SyncContext, should_send_updates};
use log::{error, info};
use osvauld_core::models::{
    document::{create_doc, state_frontiers},
    resource::{EncryptedResource, Resource},
    Permit, SyncDecision,
};
use persistance::database::RepositoryContext;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

// =============================================================================
// PATTERN 1: Load-Decrypt-Parse
// =============================================================================

/// Load resource by share permit
pub async fn load_and_decrypt_by_share_token(
    permit: &Permit,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Resource> {
    let resource_id = permit.resource_id()
        .ok_or_else(|| ResourceServiceError::ParseError("No resource_id in permit facts".to_string()))?;
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
    let resource = Resource::from_decrypted_data(
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
    })?;

    let doc_names: Vec<String> = resource.doc_names();
    info!("📂 [decrypt_encrypted_resource] Decrypted resource {} has {} documents: {:?}",
        resource.id, doc_names.len(), doc_names);

    Ok(resource)
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
    let doc_names: Vec<String> = resource.doc_names();
    info!("🔍 [filter_and_encrypt_for_peer] Source resource {} has {} documents: {:?}",
        resource.id, doc_names.len(), doc_names);

    // Create filtered resource with only documents peer can access
    let mut filtered_resource = Resource::new(resource.id.clone());

    for doc_name in doc_names {
        let decision = should_send_updates(sync_context, doc_name.as_str());
        info!("📊 [filter_and_encrypt_for_peer] Document '{}' → decision: {:?}", doc_name, decision);

        // Include document if we should send it
        if decision != SyncDecision::DontSend {
            if let Some(doc) = resource.get_doc(doc_name.as_str()) {
                // Export document as full snapshot (with operation history)
                // This ensures all parties have compatible CRDT operation logs for proper sync
                let snapshot = gurkha::MergeService::export_snapshot(doc);

                info!("📦 [filter_and_encrypt_for_peer] Cloning document '{}' with snapshot size: {} bytes",
                    doc_name, snapshot.len());

                // Import snapshot into new document for filtered resource
                let cloned_doc = gurkha::MergeService::import_snapshot(&snapshot)
                    .map_err(|e| {
                        error!("Failed to clone document {}: {}", doc_name, e);
                        ResourceServiceError::InvalidState(format!("Failed to clone document: {}", e))
                    })?;

                filtered_resource.add_doc(doc_name.to_string(), cloned_doc);
                info!("✅ [filter_and_encrypt_for_peer] Added document '{}' to filtered resource", doc_name);
            } else {
                error!("⚠️ [filter_and_encrypt_for_peer] Document '{}' not found in resource.get_doc()!", doc_name);
            }
        }
    }

    let filtered_doc_names: Vec<String> = filtered_resource.doc_names();
    info!("🎯 [filter_and_encrypt_for_peer] Filtered resource has {} documents: {:?}",
        filtered_doc_names.len(), filtered_doc_names);

    // Serialize filtered resource
    let filtered_json = filtered_resource.to_json().map_err(|e| {
        error!("Failed to serialize filtered resource: {}", e);
        ResourceServiceError::SerializationError(format!(
            "Failed to serialize filtered resource: {}",
            e
        ))
    })?;

    info!("📄 [filter_and_encrypt_for_peer] Serialized to {} bytes before encryption", filtered_json.len());

    // Encrypt for peer (returns base64 strings directly - no need to decode/re-encode)
    let (encrypted_data, encrypted_key) = encrypt_data_for_user(&filtered_json, peer_public_key)
        .map_err(|e| ResourceServiceError::EncryptionFailed(e.to_string()))?;

    info!("🔐 [filter_and_encrypt_for_peer] Encrypted data size: {} bytes (base64)", encrypted_data.len());

    Ok((encrypted_data, encrypted_key))
}

// =============================================================================
// PATTERN 5: Delegate and Create Share
// =============================================================================

/// Delegate resource UCAN and create ShareRecord
///
/// Handles the complete flow of delegating a resource UCAN to a recipient
/// and creating the corresponding ShareRecord (persisted or ephemeral based on CEL rules).
///
/// This abstraction eliminates ~90 lines of duplication between share_resource(),
/// prepare_resource_transfer(), and on-the-fly network share record generation.
///
/// # Arguments
/// * `resource_id` - Resource ID to share
/// * `recipient_user_id` - User ID to share with
/// * `recipient_role` - Role/template key for recipient ("node", "viewer", "user")
/// * `current_user` - Current user (delegator)
/// * `persist` - Whether to save to database (overrides CEL if needed)
/// * `repo_ctx` - Database repository context
/// * `ucan_service` - UCAN service for delegation
///
/// # Returns
/// * `Ok((ShareRecord, delegated_ucan_token, ucan_cid))` - Created share record and token
/// * `Err` - If delegation or database save fails
pub async fn delegate_and_create_share_record(
    resource_id: &str,
    recipient_user_id: &str,
    recipient_role: &str,
    current_user: &osvauld_core::models::User,
    persist: bool,
    repo_ctx: Arc<RepositoryContext>,
    ucan_service: &Arc<RwLock<gurkha::UcanService>>,
) -> ServiceResult<(osvauld_core::models::ShareRecord, String, String)> {
    // 1. Get recipient user
    let recipient_user = repo_ctx
        .user_repo
        .get_user_by_id(recipient_user_id)
        .await
        .map_err(|e| {
            error!("Failed to find recipient user {}: {}", recipient_user_id, e);
            ResourceServiceError::UserNotFound(recipient_user_id.to_string())
        })?;

    // 2. Get resource to read current UCAN
    let resource = repo_ctx
        .resource_repo
        .find_by_id(resource_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch resource {}: {}", resource_id, e);
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    // 3. Delegate UCAN using unified API
    let (delegated_ucan_token, ucan_cid) = {
        let ucan_guard = ucan_service.read().await;
        ucan_guard
            .delegate_resource(
                &resource.ucan_token,
                recipient_role,
                &recipient_user.ucan_pub_key,
            )
            .await
            .map_err(|e| {
                error!("Failed to generate delegated UCAN: {}", e);
                ResourceServiceError::UcanError(e.to_string())
            })?
    };

    // 4. Parse delegated token to check CEL rules
    let permit = Permit::from_token(&delegated_ucan_token).map_err(|e| {
        error!("Failed to parse delegated UCAN: {}", e);
        ResourceServiceError::ParseError(e.to_string())
    })?;

    // 5. Create ShareRecord
    let now = chrono::Utc::now().timestamp();
    let share_record = osvauld_core::models::ShareRecord {
        id: uuid::Uuid::new_v4().to_string(),
        resource_id: resource_id.to_string(),
        shared_by_user_id: current_user.id.clone(),
        recipient_user_id: recipient_user_id.to_string(),
        ucan_token: delegated_ucan_token.clone(),
        ucan_cid: ucan_cid.clone(),
        permission_level: osvauld_core::models::PermissionLevel::Read,
        operation_type: osvauld_core::models::ShareOperation::Share,
        created_at: now,
        updated_at: now,
    };

    // 6. Save to database if persist is true AND CEL allows
    if persist && permit.should_persist_share() {
        repo_ctx
            .share_repo
            .save(&share_record)
            .await
            .map_err(|e| {
                error!("Failed to save share record: {}", e);
                ResourceServiceError::DatabaseError(e.to_string())
            })?;
        info!("✅ Share record persisted to database");
    } else if persist && !permit.should_persist_share() {
        info!("⚠️ CEL rule prevented share record persistence (ephemeral token)");
    } else {
        info!("✅ Share record created (ephemeral, not persisted)");
    }

    Ok((share_record, delegated_ucan_token, ucan_cid))
}

/// Create EncryptedResource struct from components
///
/// Helper to construct EncryptedResource with proper timestamp handling.
/// Reduces boilerplate and ensures consistent field ordering.
///
/// This abstraction eliminates ~20 lines of duplication between create_resource()
/// and prepare_resource_transfer().
///
/// # Arguments
/// * `resource_id` - Resource ID
/// * `folder_id` - Folder ID
/// * `encrypted_data` - Base64 encrypted data
/// * `encrypted_key` - Base64 encrypted AES key
/// * `ucan_token` - UCAN token for this resource
/// * `metadata` - Resource metadata JSON
/// * `timestamps` - Optional (created_at, updated_at) tuple. If None, uses current time.
///
/// # Returns
/// * `EncryptedResource` - Constructed struct ready to save or send
pub fn create_encrypted_resource_struct(
    resource_id: String,
    folder_id: String,
    encrypted_data: String,
    encrypted_key: String,
    ucan_token: String,
    metadata: serde_json::Value,
    timestamps: Option<(i64, i64)>,
) -> EncryptedResource {
    let (created_at, updated_at) = timestamps.unwrap_or_else(|| {
        let now = chrono::Utc::now().timestamp();
        (now, now)
    });

    EncryptedResource {
        id: resource_id,
        folder_id,
        created_at,
        updated_at,
        encrypted_data,
        encrypted_key,
        ucan_token,
        metadata,
    }
}
