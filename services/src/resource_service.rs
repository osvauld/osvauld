use crate::errors::{ResourceServiceError, ServiceResult};
use crypto_utils::{CryptoUtils, encrypt_data_for_user, errors::UcanError};
use log::{error, info};
use osvauld_core::models::{
    DecryptedResource, PermissionLevel, Resource, ResourceKey, ResourceVectorClock,
    ResourceWithKey, ShareOperation, ShareRecord, User,
};
use persistance::database::RepositoryContext;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
#[derive(Debug)]
pub struct ResourceSharingData {
    pub resource_key: ResourceKey,
    pub share_record: ShareRecord,
    pub vector_clocks: Vec<ResourceVectorClock>,
}

/// Create a new resource with all its dependencies
pub async fn create_resource(
    resource_payload: String,
    resource_type: String,
    folder_id: String,
    user: &User,
    current_device_id: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<DecryptedResource> {
    let (encrypted_data, encrypted_key) =
        encrypt_data_for_user(&resource_payload, &user.public_key)?;

    // Create the resource
    let mut resource = Resource::new(
        resource_type,
        encrypted_data,
        folder_id.clone(),
        "signature".to_string(),
        user.id.clone(),
    );

    let signature = {
        let crypto = crypto_utils.read().await;
        crypto.sign_and_hash_message(&resource.id)?
    };
    resource.signature = signature.to_owned();

    // Create the resource key for the user
    let resource_key = ResourceKey::new(resource.id.clone(), user.id.clone(), encrypted_key, true);

    let encrypted_ucan_pvt_key = repo_ctx.store_repo.get_ucan_key().await?;

    let (ucan_token, ucan_cid) = {
        let crypto = crypto_utils.read().await;
        crypto
            .generate_resource_owner_ucan(&encrypted_ucan_pvt_key, &resource.id, domain)
            .await?
    };

    // Create the share record (user sharing with themselves as owner)
    let share_record = ShareRecord::prepare_share_record(
        resource.id.clone(),
        user.id.clone(),
        user.id.clone(),
        PermissionLevel::Admin,
        ucan_token,
        ucan_cid,
    );

    // Get user's devices to create vector clocks
    let user_devices = repo_ctx
        .device_repo
        .get_devices_by_user_id(&user.id)
        .await?;

    let device_ids: Vec<String> = user_devices.iter().map(|d| d.id.clone()).collect();

    // Create initial vector clocks for all user's devices
    let vector_clocks =
        ResourceVectorClock::create_initial_entries(&resource.id, &device_ids, current_device_id);

    // Save everything in a single transaction
    repo_ctx
        .resource_repo
        .save_resource_with_dependencies(&resource, &resource_key, &share_record, &vector_clocks)
        .await?;

    let decrypted_resource =
        get_resource_by_id_direct(&resource.id, &user.id, repo_ctx.clone(), crypto_utils).await?;

    Ok(decrypted_resource)
}

/// Get a resource by ID and decrypt it for the user
pub async fn get_resource_by_id_direct(
    resource_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<DecryptedResource> {
    // Get the resource with its key from the repository
    let resource_with_key = repo_ctx
        .resource_repo
        .find_by_id(resource_id, user_id)
        .await?;

    // Decrypt the resource
    let decrypted_resource = decrypt_single_resource(resource_with_key, crypto_utils).await?;

    Ok(decrypted_resource)
}

/// Helper function to decrypt a single resource
async fn decrypt_single_resource(
    resource_with_key: ResourceWithKey,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<DecryptedResource> {
    // Lock crypto_utils and decrypt the resource data
    let decrypted_data = {
        let crypto = crypto_utils.read().await;
        crypto.decrypt_resource(
            &resource_with_key.resource.data,
            &resource_with_key.encrypted_key,
        )?
    };

    // Parse the JSON data
    let parsed_data: Value = serde_json::from_str(&decrypted_data)
        .unwrap_or_else(|_| serde_json::json!({"error": "Failed to parse resource data"}));
    let last_modified = parsed_data
        .get("last_modified")
        .and_then(|v| v.as_i64())
        .unwrap_or(resource_with_key.resource.last_accessed);
    // Create the DecryptedResource
    let decrypted_resource = DecryptedResource {
        id: resource_with_key.resource.id,
        resource_type: resource_with_key.resource.resource_type,
        data: parsed_data,
        last_accessed: last_modified,
        favourite: resource_with_key.resource.favourite,
        folder_id: resource_with_key.resource.folder_id,
    };

    Ok(decrypted_resource)
}

pub async fn delete_resource(
    resource_id: String,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    Ok(repo_ctx
        .resource_repo
        .soft_delete_resource(&resource_id)
        .await?)
}

pub async fn toggle_fav(
    resource_id: String,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    Ok(repo_ctx.resource_repo.toggle_fav(&resource_id).await?)
}

pub async fn update_last_accessed(
    resource_id: String,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    Ok(repo_ctx
        .resource_repo
        .update_last_accessed(&resource_id)
        .await?)
}

pub async fn update_resource(
    resource_id: &str,
    data: String,
    user_id: &str,
    current_device_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<DecryptedResource> {
    let old_resource = repo_ctx
        .resource_repo
        .find_by_id(resource_id, user_id)
        .await?;

    let encrypted_data = {
        let crypto = crypto_utils.read().await;
        crypto.update_resource(&data, &old_resource.encrypted_key)?
    };

    repo_ctx
        .resource_repo
        .update_resource(&encrypted_data, resource_id)
        .await?;

    repo_ctx
        .vector_clock_repo
        .increment_vector_clock(resource_id, current_device_id)
        .await?;

    let decrypted_resource =
        get_resource_by_id_direct(resource_id, user_id, repo_ctx, crypto_utils).await?;

    Ok(decrypted_resource)
}

pub async fn get_resource(
    resource_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    user_id: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<(DecryptedResource, String)> {
    // Get encrypted resource from repository
    let resource_with_key = repo_ctx
        .resource_repo
        .find_by_id(resource_id, user_id)
        .await?;

    let encrypted_key = resource_with_key.encrypted_key.clone();
    // Use the helper function
    let decrypted_resources = decrypt_single_resource(resource_with_key, crypto_utils).await?;

    Ok((decrypted_resources, encrypted_key))
}

pub async fn get_resources_for_folder(
    folder_id: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<DecryptedResource>> {
    let resources_with_keys = repo_ctx
        .resource_repo
        .find_by_folder(folder_id, &user_id)
        .await?;

    // Decrypt and return the resources
    decrypt_resources(resources_with_keys, crypto_utils).await
}

pub async fn get_all_resources(
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: Arc<RepositoryContext>,
    user_id: &str,
) -> ServiceResult<Vec<DecryptedResource>> {
    // Get resources with their keys
    let resources_with_keys = repo_ctx.resource_repo.get_all_resources(&user_id).await?;

    // Decrypt and return the resources
    decrypt_resources(resources_with_keys, crypto_utils).await
}

async fn decrypt_resources(
    resources_with_keys: Vec<ResourceWithKey>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Vec<DecryptedResource>> {
    // Initialize vector to store decrypted resources
    let mut decrypted_resources = Vec::with_capacity(resources_with_keys.len());

    // Lock crypto_utils once before the loop
    let crypto = crypto_utils.read().await;

    // Process each resource
    for rk in resources_with_keys {
        // Decrypt the resource data
        let decrypted_data = crypto.decrypt_resource(&rk.resource.data, &rk.encrypted_key)?;

        // Parse the JSON data
        let parsed_data: Value = serde_json::from_str(&decrypted_data)
            .unwrap_or_else(|_| serde_json::json!({"error": "Failed to parse resource data"}));

        // Create the DecryptedResource
        let decrypted_resource = DecryptedResource {
            id: rk.resource.id,
            resource_type: rk.resource.resource_type,
            data: parsed_data,
            last_accessed: rk.resource.last_accessed,
            favourite: rk.resource.favourite,
            folder_id: rk.resource.folder_id,
        };

        decrypted_resources.push(decrypted_resource);
    }

    Ok(decrypted_resources)
}
pub async fn prepare_share_resource(
    recipient_user_id: &str,
    resource_id: &str,
    permissions: Vec<(String, String)>,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Option<ResourceSharingData>> {
    // 1. Check if resource is already shared with recipient
    if repo_ctx
        .resource_key_repo
        .find_by_resource_and_user(resource_id, recipient_user_id)
        .await
        .is_ok()
    {
        return Ok(None); // Already shared
    }

    // 2. Get the resource key for the current user
    let resource_key = repo_ctx
        .resource_key_repo
        .find_by_resource_and_user(resource_id, &current_user.id)
        .await?;

    let delegator_share_record = repo_ctx
        .share_repo
        .find_by_resource_and_operation_and_user(
            resource_id,
            &ShareOperation::Share.to_string(),
            &current_user.id,
        )
        .await?;

    // 3. Get the recipient user to access their public key
    let recipient_user = repo_ctx.user_repo.get_user_by_id(recipient_user_id).await?;

    // 4. Encrypt the resource key for the recipient using their public key
    let new_encryption_key = {
        let crypto = crypto_utils.read().await;
        crypto
            .encrypt_key_with_new_pub_key(&resource_key.encrypted_key, &recipient_user.public_key)?
    };

    // 5. Create the new resource key for the recipient
    let new_resource_key = ResourceKey::new(
        resource_id.to_string(),
        recipient_user_id.to_string(),
        new_encryption_key,
        false,
    );

    let encrypted_ucan_pvt_key = repo_ctx.store_repo.get_ucan_key().await?;

    // 6. Get the resource owner (root authority for validation)
    let resource_owner = repo_ctx
        .resource_repo
        .find_owner_by_resource_id(resource_id)
        .await?;

    // 7. Create the signature for the share record
    let repo_ctx_clone = repo_ctx.clone();
    let proof_resolver = move |cid: &str| resolve_proof(repo_ctx_clone.clone(), cid.to_string());

    let (ucan_token, ucan_cid) = {
        let crypto = crypto_utils.read().await;
        crypto
            .issue_delegated_resource_ucan(
                &encrypted_ucan_pvt_key,
                &delegator_share_record.ucan_token,
                &resource_owner.ucan_pub_key,
                resource_id,
                &recipient_user.ucan_pub_key,
                permissions,
                &proof_resolver,
            )
            .await?
    };

    // 8. Create the share record
    let share_record = ShareRecord::prepare_share_record(
        resource_id.to_string(),
        current_user.id.to_string(),
        recipient_user_id.to_string(),
        PermissionLevel::Write, // Default permission level
        ucan_token,
        ucan_cid,
    );

    // 9. Get recipient's devices to create vector clocks
    let recipient_devices = repo_ctx
        .device_repo
        .get_devices_by_user_id(recipient_user_id)
        .await?;

    let recipient_device_ids: Vec<String> =
        recipient_devices.iter().map(|d| d.id.clone()).collect();

    // 10. Create vector clocks for recipient's devices
    let recipient_vector_clocks =
        ResourceVectorClock::create_entries_for_sharing(resource_id, &recipient_device_ids);

    Ok(Some(ResourceSharingData {
        resource_key: new_resource_key,
        share_record,
        vector_clocks: recipient_vector_clocks,
    }))
}

pub async fn share_resource(
    recipient_user_id: &str,
    resource_id: &str,
    permissions: Vec<(String, String)>,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()> {
    if let Some(sharing_data) = prepare_share_resource(
        recipient_user_id,
        resource_id,
        permissions,
        current_user,
        repo_ctx.clone(),
        crypto_utils,
    )
    .await?
    {
        // Save to repository if sharing is needed
        repo_ctx
            .resource_repo
            .share_resource_transaction(
                &sharing_data.resource_key,
                &sharing_data.share_record,
                &sharing_data.vector_clocks,
            )
            .await?;
    }
    // If None, resource was already shared - do nothing
    Ok(())
}

pub async fn get_resource_state_vector(
    resource_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<String> {
    // 1. Get the decrypted resource
    let (decrypted_resource, _) =
        get_resource(resource_id, repo_ctx, user_id, crypto_utils).await?;

    let state_vectors = decrypted_resource
        .get_state_vectors()
        .await
        .map_err(|e| ResourceServiceError::ParseError(e))?;

    Ok(state_vectors)
}

pub async fn get_resource_sync_info(
    resource_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<osvauld_core::models::ResourceSyncInfo> {
    // 1. Get decrypted resource
    let (decrypted_resource, _) =
        get_resource(resource_id, repo_ctx.clone(), user_id, crypto_utils).await?;
    info!("decrypted_resource {:?}", decrypted_resource.data);
    // 2. Get state vectors JSON
    let state_vectors_json = decrypted_resource
        .get_state_vectors()
        .await
        .map_err(|e| ResourceServiceError::ParseError(e))?;

    // 3. For Website resources being synced by viewer, populate form data
    let sync_data =
        if decrypted_resource.resource_type == osvauld_core::models::ResourceType::Website {
            let mut parsed: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&state_vectors_json)
                    .map_err(|e| ResourceServiceError::ParseError(e.to_string()))?;

            // Get form data and populate it
            if let Some(form_doc) = parsed.get_mut("form_submissions_doc") {
                if let Some(doc_obj) = form_doc.as_object_mut() {
                    let form_data = decrypted_resource
                        .get_document_state("form_submissions_doc")
                        .unwrap_or_default();

                    let form_data_array: Vec<serde_json::Value> = form_data
                        .iter()
                        .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                        .collect();

                    doc_obj.insert(
                        "updates".to_string(),
                        serde_json::Value::Array(form_data_array),
                    );
                    doc_obj.insert("state_vector".to_string(), serde_json::Value::Array(vec![]));
                }
            }

            serde_json::to_string(&parsed)
                .map_err(|e| ResourceServiceError::ParseError(e.to_string()))?
        } else {
            state_vectors_json
        };

    // 4. Get resource UCAN token
    let resource_ucan = get_resource_ucan_key(resource_id, user_id, repo_ctx).await?;

    Ok(osvauld_core::models::ResourceSyncInfo {
        resource_id: resource_id.to_string(),
        resource_ucan,
        sync_data,
    })
}

/// Process incremental sync from viewer and return updates for viewer
/// Replaces form_submissions_doc with empty updates (node doesn't send form data back)
pub async fn process_incremental_resource_sync(
    resource_id: &str,
    user_id: &str,
    viewer_sync_data: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<String> {
    // 1. Parse viewer's sync data
    let mut viewer_data: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(viewer_sync_data)
            .map_err(|e| ResourceServiceError::ParseError(e.to_string()))?;

    // 2. Empty updates for blocksuite_doc (viewer can't edit main content)
    // Keep state_vector so sync protocol works correctly
    if let Some(blocksuite_doc) = viewer_data.get_mut("blocksuite_doc") {
        if let Some(obj) = blocksuite_doc.as_object_mut() {
            obj.insert("updates".to_string(), serde_json::Value::Array(vec![]));
        }
    }

    // 3. Empty updates for thread_comments_doc (comments use separate ViewerCommentsUpdate flow)
    // Keep state_vector so sync protocol works correctly
    if let Some(thread_doc) = viewer_data.get_mut("thread_comments_doc") {
        if let Some(obj) = thread_doc.as_object_mut() {
            obj.insert("updates".to_string(), serde_json::Value::Array(vec![]));
        }
    }

    // 4. Keep form_submissions_doc with updates intact - these will be applied to DB
    let modified_sync = serde_json::to_string(&viewer_data)
        .map_err(|e| ResourceServiceError::ParseError(e.to_string()))?;

    // 5. Apply form submissions to DB and generate response for viewer
    // This will:
    // - Merge form_submissions_doc into database
    // - Emit form_submissions_doc updates to owner
    // - Generate updates viewer needs (blocksuite_doc, thread_comments_doc)
    let response = apply_updates_and_get_peer_updates(
        resource_id,
        user_id,
        &modified_sync,
        repo_ctx,
        crypto_utils,
    )
    .await?;

    info!("response back from node {:?}", response);

    // 6. Parse response and empty form_submissions_doc (viewer doesn't need it back)
    let mut parsed: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&response)
        .map_err(|e| ResourceServiceError::ParseError(e.to_string()))?;

    // Empty form_submissions_doc updates in response (viewer has append-only, doesn't read back)
    // Keep state_vector for sync protocol
    if let Some(form_doc) = parsed.get_mut("form_submissions_doc") {
        if let Some(obj) = form_doc.as_object_mut() {
            obj.insert("updates".to_string(), serde_json::Value::Array(vec![]));
            // state_vector is preserved
        }
    }

    Ok(serde_json::to_string(&parsed)
        .map_err(|e| ResourceServiceError::ParseError(e.to_string()))?)
}

/// Apply updates from node and generate viewer updates (only comments)
/// Used by viewer to process node's sync response and generate updates to send back
/// Viewer only sends back thread_comments_doc updates (bidirectional sync)
pub async fn apply_and_generate_viewer_updates(
    resource_id: &str,
    user_id: &str,
    node_sync_data: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<String> {
    // Apply updates from node and get response
    let response = apply_updates_and_get_peer_updates(
        resource_id,
        user_id,
        node_sync_data,
        repo_ctx,
        crypto_utils,
    )
    .await?;

    // Parse and keep only thread_comments_doc (remove website and forms)
    let mut parsed: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&response)
        .map_err(|e| ResourceServiceError::ParseError(e.to_string()))?;

    // Remove blocksuite_doc (viewer is read-only for website content)
    parsed.remove("blocksuite_doc");
    // Remove form_submissions_doc (viewer → node only, not bidirectional)
    parsed.remove("form_submissions_doc");

    // Only thread_comments_doc remains for bidirectional sync
    Ok(serde_json::to_string(&parsed)
        .map_err(|e| ResourceServiceError::ParseError(e.to_string()))?)
}

pub async fn get_resource_ucan_key(
    resource_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<String> {
    let delegator_share_record = repo_ctx
        .share_repo
        .find_by_resource_and_operation_and_user(
            resource_id,
            &ShareOperation::Share.to_string(),
            user_id,
        )
        .await?;

    Ok(delegator_share_record.ucan_token)
}

// Also update the generate_updates_for_peer function similarly
pub async fn generate_updates_for_peer(
    resource_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    peer_state_vectors: &String,
) -> ServiceResult<String> {
    // 1. Get the decrypted resource
    let (mut decrypted_resource, _) =
        get_resource(resource_id, repo_ctx, user_id, crypto_utils).await?;
    info!("decrypted_resource {:?}", decrypted_resource);

    let updates = decrypted_resource
        .sync_updates(peer_state_vectors)
        .await
        .map_err(|e| ResourceServiceError::ParseError(e))?;

    Ok(updates)
}

/// Apply updates from a peer and generate any updates they might need in return
pub async fn apply_updates_and_get_peer_updates(
    resource_id: &str,
    user_id: &str,
    updates: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<String> {
    // 1. Get the current resource with its YJS state
    let (mut decrypted_resource, encrypted_key) =
        get_resource(resource_id, repo_ctx.clone(), user_id, crypto_utils).await?;

    info!("decrypted resource {:?}", &decrypted_resource);
    let remote_updates = decrypted_resource
        .sync_updates(updates)
        .await
        .map_err(|e| ResourceServiceError::ParseError(e))?;

    let encrypted_data = {
        let crypto = crypto_utils.read().await;
        crypto.update_resource(&decrypted_resource.data.to_string(), &encrypted_key)?
    };

    repo_ctx
        .resource_repo
        .update_resource(&encrypted_data, resource_id)
        .await?;

    Ok(remote_updates)
}

pub async fn apply_buffer_updates_and_get_remote_updates(
    resource_id: &str,
    user_id: &str,
    updates: &str,
    peer_state_vectors: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<String> {
    // 1. Get the current resource with its YJS state
    let (mut decrypted_resource, _encrypted_key) =
        get_resource(resource_id, repo_ctx, user_id, crypto_utils).await?;

    let _ = decrypted_resource
        .sync_updates(updates)
        .await
        .map_err(|e| ResourceServiceError::ParseError(e))?;

    let remote_updates = decrypted_resource
        .sync_updates(peer_state_vectors)
        .await
        .map_err(|e| ResourceServiceError::ParseError(e))?;

    Ok(remote_updates)
}

pub async fn apply_buffer_and_peer_updates_and_get_remote_updates(
    resource_id: &str,
    user_id: &str,
    remote_updates: &str,
    local_updates: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<String> {
    // 1. Get the current resource with its YJS state
    let (mut decrypted_resource, _encrypted_key) =
        get_resource(resource_id, repo_ctx, user_id, crypto_utils).await?;

    let _remote_updates = decrypted_resource
        .sync_updates(local_updates)
        .await
        .map_err(|e| ResourceServiceError::ParseError(e))?;

    let remote_updates = decrypted_resource
        .sync_updates(remote_updates)
        .await
        .map_err(|e| ResourceServiceError::ParseError(e))?;

    Ok(remote_updates)
}

pub async fn apply_updates(
    resource_id: &str,
    updates: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()> {
    let (mut decrypted_resource, encrypted_key) =
        get_resource(resource_id, repo_ctx.clone(), user_id, crypto_utils).await?;

    decrypted_resource
        .sync_updates(updates)
        .await
        .map_err(|e| ResourceServiceError::ParseError(e))?;

    let encrypted_data = {
        let crypto = crypto_utils.read().await;
        crypto.update_resource(&decrypted_resource.data.to_string(), &encrypted_key)?
    };

    repo_ctx
        .resource_repo
        .update_resource(&encrypted_data, resource_id)
        .await?;

    Ok(())
}

pub async fn get_share_records_for_resource(
    resource_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<ShareRecord>> {
    Ok(repo_ctx.share_repo.find_by_resource(resource_id).await?)
}

pub async fn get_vector_clocks_for_resource(
    resource_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<ResourceVectorClock>> {
    Ok(repo_ctx
        .vector_clock_repo
        .get_vector_clocks_for_resource(resource_id)
        .await?)
}

pub fn find_missing_resource_keys(
    local: &[ResourceKey],
    remote: &[ResourceKey],
) -> (Vec<ResourceKey>, Vec<ResourceKey>) {
    let missing_in_remote: Vec<ResourceKey> = local
        .iter()
        .filter(|item1| !remote.iter().any(|item2| item2.id == item1.id))
        .cloned()
        .collect();

    let missing_in_local: Vec<ResourceKey> = remote
        .iter()
        .filter(|item2| !local.iter().any(|item1| item1.id == item2.id))
        .cloned()
        .collect();

    (missing_in_local, missing_in_remote)
}

pub async fn get_resource_keys_for_resource(
    resource_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<ResourceKey>> {
    Ok(repo_ctx
        .resource_key_repo
        .find_by_resource_id(resource_id)
        .await?)
}

pub async fn merge_vector_clocks(
    resource_id: &str,
    vector_clocks: &Vec<ResourceVectorClock>,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<(Vec<ResourceVectorClock>, Vec<ResourceVectorClock>)> {
    info!("Merging vector clocks for resource {}", resource_id);

    // Get our local vector clocks for this resource
    let local_vector_clocks = repo_ctx
        .vector_clock_repo
        .get_vector_clocks_for_resource(resource_id)
        .await?;

    let merge_result = ResourceVectorClock::merge(&local_vector_clocks, vector_clocks);

    // If local clocks need updating, update our database
    if merge_result.local_needs_update {
        repo_ctx
            .vector_clock_repo
            .update_vector_clocks(&merge_result.update_local, &merge_result.add_local)
            .await?;
    }

    Ok((merge_result.add_remote, merge_result.update_remote))
}

pub async fn merge_share_records(
    resource_id: &str,
    remote_share_records: &[ShareRecord],
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<ShareRecord>> {
    let local_share_records = repo_ctx.share_repo.find_by_resource(resource_id).await?;

    let local_set: HashSet<String> = local_share_records
        .iter()
        .map(|record| record.id.clone())
        .collect();

    let remote_set: HashSet<String> = remote_share_records
        .iter()
        .map(|record| record.id.clone())
        .collect();

    // Find share records that exist in remote but not in local
    let remote_only_records: Vec<ShareRecord> = remote_share_records
        .iter()
        .filter(|record| !local_set.contains(&record.id))
        .cloned()
        .collect();

    repo_ctx.share_repo.save_many(&remote_only_records).await?;

    // Find share records that exist in local but not in remote
    let local_only_records: Vec<ShareRecord> = local_share_records
        .iter()
        .filter(|record| !remote_set.contains(&record.id))
        .cloned()
        .collect();

    Ok(local_only_records)
}

pub async fn update_vector_clocks(
    add_clock: &[ResourceVectorClock],
    update_clock: &[ResourceVectorClock],
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    Ok(repo_ctx
        .vector_clock_repo
        .update_vector_clocks(&update_clock, &add_clock)
        .await?)
}

pub async fn add_share_records(
    share_records: &[ShareRecord],
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    Ok(repo_ctx.share_repo.save_many(share_records).await?)
}

pub async fn validate_authority_for_update(
    resource_id: &str,
    token: &str,
    peer_user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    domain: &str,
) -> ServiceResult<bool> {
    let peer_user = repo_ctx.user_repo.get_user_by_id(peer_user_id).await?;

    let resource_owner = repo_ctx
        .resource_repo
        .find_owner_by_resource_id(resource_id)
        .await?;

    let repo_ctx_clone = repo_ctx.clone();
    let proof_resolver = move |cid: &str| resolve_proof(repo_ctx_clone.clone(), cid.to_string());

    let token = crypto_utils::validate_authority_for_update(
        token,
        &peer_user.ucan_pub_key,
        &resource_owner.ucan_pub_key,
        resource_id,
        domain,
        &proof_resolver,
    )
    .await?;

    Ok(token)
}

async fn resolve_proof(repo_ctx: Arc<RepositoryContext>, cid: String) -> Result<String, UcanError> {
    repo_ctx
        .share_repo
        .get_ucan_by_cid(&cid)
        .await
        .map_err(|e| UcanError::ProofChainInvalid(e.to_string()))
}

/// Auto-share a resource with all users who have access to the folder
/// This runs in background to avoid blocking the frontend response
pub async fn auto_share_resource_with_folder_users(
    resource_id: &str,
    folder_id: &str,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    domain: &str,
) -> ServiceResult<()> {
    info!(
        "Starting auto-share for resource {} in folder {}",
        resource_id, folder_id
    );

    // 1. Get users who have access to this folder
    let shared_users = match repo_ctx.folder_share_repo.get_shared_users(folder_id).await {
        Ok(users) => users,
        Err(e) => {
            error!("Failed to get shared users for folder {}: {}", folder_id, e);
            return Err(e.into());
        }
    };

    // 2. Filter out current user (already has access)
    let recipient_users: Vec<_> = shared_users
        .iter()
        .filter(|user| user.id != current_user.id)
        .collect();

    if recipient_users.is_empty() {
        info!(
            "No other users to share resource {} with in folder {}",
            resource_id, folder_id
        );
        return Ok(());
    }

    info!(
        "Found {} users to auto-share resource {} with",
        recipient_users.len(),
        resource_id
    );

    // 3. Prepare sharing data for each recipient
    let mut all_sharing_data = Vec::new();

    for recipient_user in recipient_users {
        // Create resource permissions (same as folder sharing)
        let resource_permissions = vec![
            (
                format!("{}:resource:{}", domain, resource_id),
                "crud/read".to_string(),
            ),
            (
                format!("{}:resource:{}", domain, resource_id),
                "crud/update".to_string(),
            ),
            (
                format!("{}:resource:{}", domain, resource_id),
                "ucan/share".to_string(),
            ),
        ];

        // Prepare sharing data
        match prepare_share_resource(
            &recipient_user.id,
            resource_id,
            resource_permissions,
            current_user,
            repo_ctx.clone(),
            crypto_utils,
        )
        .await
        {
            Ok(Some(sharing_data)) => {
                info!(
                    "Prepared sharing data for user {} on resource {}",
                    recipient_user.id, resource_id
                );
                all_sharing_data.push(sharing_data);
            }
            Ok(None) => {
                info!(
                    "Resource {} already shared with user {}, skipping",
                    resource_id, recipient_user.id
                );
            }
            Err(e) => {
                error!(
                    "Failed to prepare sharing data for user {} on resource {}: {}",
                    recipient_user.id, resource_id, e
                );
                // Continue with other users instead of failing completely
                continue;
            }
        }
    }

    // 4. Batch save all sharing data if we have any
    if !all_sharing_data.is_empty() {
        let resource_keys: Vec<_> = all_sharing_data
            .iter()
            .map(|d| d.resource_key.clone())
            .collect();
        let share_records: Vec<_> = all_sharing_data
            .iter()
            .map(|d| d.share_record.clone())
            .collect();
        let vector_clocks: Vec<_> = all_sharing_data
            .iter()
            .flat_map(|d| d.vector_clocks.clone())
            .collect();

        // Save all in a transaction
        match repo_ctx
            .resource_repo
            .save_bulk_sharing_data(&resource_keys, &share_records, &vector_clocks)
            .await
        {
            Ok(()) => {
                info!(
                    "Successfully auto-shared resource {} with {} users in folder {}",
                    resource_id,
                    all_sharing_data.len(),
                    folder_id
                );
            }
            Err(e) => {
                error!(
                    "Failed to save bulk sharing data for resource {} in folder {}: {}",
                    resource_id, folder_id, e
                );
                return Err(e.into());
            }
        }
    } else {
        info!(
            "No new sharing data to save for resource {} in folder {}",
            resource_id, folder_id
        );
    }

    Ok(())
}
