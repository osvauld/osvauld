use crypto_utils::{CryptoUtils, encrypt_data_for_user};
use log::info;
use osvauld_core::models::{
    DecryptedResource, PermissionLevel, Resource, ResourceKey, ResourceVectorClock,
    ResourceWithKey, ShareRecord, User,
};
use osvauld_core::repositories::RepositoryError;
use persistance::database::RepositoryContext;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Error, Debug)]
pub enum ResourceServiceError {
    #[error("Repository error: {0}")]
    RepositoryError(#[from] RepositoryError),
    #[error("Crypto error: {0}")]
    CryptoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Create a new resource with all its dependencies
pub async fn create_resource(
    resource_payload: String,
    resource_type: String,
    folder_id: String,
    user: &User,
    current_device_id: &str,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<DecryptedResource, ResourceServiceError> {
    // Encrypt the resource using the provided function
    let (encrypted_data, encrypted_key) =
        encrypt_data_for_user(&resource_payload, &user.public_key)
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?;

    //TODO: add created by to verify.
    // Create the resource
    let mut resource = Resource::new(
        resource_type,
        encrypted_data,
        folder_id,
        "signature".to_string(),
    );
    let signature = {
        let crypto = crypto_utils.lock().await;
        crypto
            .sign_and_hash_message(&resource.id)
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
    };
    resource.signature = signature.to_owned();

    // Create the resource key for the user
    let resource_key = ResourceKey::new(resource.id.clone(), user.id.clone(), encrypted_key, true);
    //signature string combinattion of resource_id, shared by and shared to users ids
    let signature_string = format!(
        "{}{}{}{}",
        resource.id,
        user.id.clone(),
        user.id.clone(),
        PermissionLevel::Admin.to_string()
    );
    let signature = {
        let crypto = crypto_utils.lock().await;
        crypto
            .sign_and_hash_message(&signature_string)
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
    };

    // Create the share record (user sharing with themselves as owner)
    let share_record = ShareRecord::prepare_share_record(
        resource.id.clone(),
        user.id.clone(),
        user.id.clone(),
        PermissionLevel::Admin,
        signature,
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
    info!(" vecto clocks{:?}", vector_clocks);

    // Save everything in a single transaction
    repo_ctx
        .resource_repo
        .save_resource_with_dependencies(&resource, &resource_key, &share_record, &vector_clocks)
        .await?;
    let decrypted_resource =
        get_resource_by_id_direct(&resource.id, &user.id, repo_ctx, crypto_utils).await?;

    Ok(decrypted_resource)
}

/// Get a resource by ID and decrypt it for the user
pub async fn get_resource_by_id_direct(
    resource_id: &str,
    user_id: &str,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<DecryptedResource, ResourceServiceError> {
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
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<DecryptedResource, ResourceServiceError> {
    // Lock crypto_utils and decrypt the resource data
    let decrypted_data = {
        let crypto = crypto_utils.lock().await;
        crypto
            .decrypt_resource(
                &resource_with_key.resource.data,
                &resource_with_key.encrypted_key,
            )
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
    };

    // Parse the JSON data
    let parsed_data: Value = serde_json::from_str(&decrypted_data)
        .unwrap_or_else(|_| serde_json::json!({"error": "Failed to parse resource data"}));

    // Create the DecryptedResource
    let decrypted_resource = DecryptedResource {
        id: resource_with_key.resource.id,
        resource_type: resource_with_key.resource.resource_type,
        data: parsed_data,
        last_accessed: resource_with_key.resource.last_accessed,
        favourite: resource_with_key.resource.favourite,
        folder_id: resource_with_key.resource.folder_id,
    };

    Ok(decrypted_resource)
}

pub async fn delete_resource(
    resource_id: String,
    repo_ctx: &RepositoryContext,
) -> Result<(), RepositoryError> {
    repo_ctx
        .resource_repo
        .soft_delete_resource(&resource_id)
        .await
}

pub async fn toggle_fav(
    resource_id: String,
    repo_ctx: &RepositoryContext,
) -> Result<(), RepositoryError> {
    repo_ctx.resource_repo.toggle_fav(&resource_id).await
}

pub async fn update_last_accessed(
    resource_id: String,
    repo_ctx: &RepositoryContext,
) -> Result<(), RepositoryError> {
    repo_ctx
        .resource_repo
        .update_last_accessed(&resource_id)
        .await
}

pub async fn update_resource(
    resource_id: &str,
    data: String,
    user_id: &str,
    current_device_id: &str,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<DecryptedResource, ResourceServiceError> {
    let old_resource = repo_ctx
        .resource_repo
        .find_by_id(resource_id, user_id)
        .await
        .map_err(ResourceServiceError::RepositoryError)?;

    let encrypted_data = {
        let crypto = crypto_utils.lock().await;
        crypto
            .update_resource(&data, &old_resource.encrypted_key)
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
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
    repo_ctx: &RepositoryContext,
    user_id: &str,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<(DecryptedResource, String), ResourceServiceError> {
    // Get encrypted resource from repository
    let resource_with_key = repo_ctx
        .resource_repo
        .find_by_id(resource_id, user_id)
        .await
        .map_err(ResourceServiceError::RepositoryError)?;
    let encrypted_key = resource_with_key.encrypted_key.clone();
    // Use the helper function with a single-element vector
    let decrypted_resources = decrypt_single_resource(resource_with_key, crypto_utils).await?;

    Ok((decrypted_resources, encrypted_key))
}

pub async fn get_resources_for_folder(
    folder_id: &str,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
    user_id: &str,
    repo_ctx: &RepositoryContext,
) -> Result<Vec<DecryptedResource>, ResourceServiceError> {
    let resources_with_keys = repo_ctx
        .resource_repo
        .find_by_folder(folder_id, &user_id)
        .await
        .map_err(ResourceServiceError::RepositoryError)?;

    // Decrypt and return the resources
    decrypt_resources(resources_with_keys, crypto_utils).await
}

pub async fn get_all_resources(
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
    repo_ctx: &RepositoryContext,
    user_id: &str,
) -> Result<Vec<DecryptedResource>, ResourceServiceError> {
    // Get resources with their keys
    let resources_with_keys = repo_ctx
        .resource_repo
        .get_all_resources(&user_id)
        .await
        .map_err(ResourceServiceError::RepositoryError)?;

    // Decrypt and return the resources
    decrypt_resources(resources_with_keys, crypto_utils).await
}

async fn decrypt_resources(
    resources_with_keys: Vec<ResourceWithKey>,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<Vec<DecryptedResource>, ResourceServiceError> {
    // Initialize vector to store decrypted resources
    let mut decrypted_resources = Vec::with_capacity(resources_with_keys.len());

    // Lock crypto_utils once before the loop
    let crypto = crypto_utils.lock().await;

    // Process each resource
    for rk in resources_with_keys {
        // Decrypt the resource data using the single-resource function
        let decrypted_data = crypto
            .decrypt_resource(&rk.resource.data, &rk.encrypted_key)
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?;

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
pub async fn share_resource(
    recipient_user_id: &str,
    resource_id: &str,
    current_user_id: &str,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<(), ResourceServiceError> {
    // 1. Get the resource key for the current user (resource owner)
    let resource_key = repo_ctx
        .resource_key_repo
        .find_by_resource_and_user(resource_id, current_user_id)
        .await?;

    // 2. Get the recipient user to access their public key
    let recipient_user = repo_ctx
        .user_repo
        .get_user_by_id(&recipient_user_id)
        .await?;

    // 3. Encrypt the resource key for the recipient using their public key
    let new_encryption_key = {
        let crypto = crypto_utils.lock().await;
        crypto
            .encrypt_key_with_new_pub_key(&resource_key.encrypted_key, &recipient_user.public_key)
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
    };

    // 4. Create the new resource key for the recipient
    let new_resource_key = ResourceKey::new(
        resource_id.to_string(),
        recipient_user_id.to_string(),
        new_encryption_key,
        false,
    );

    // 5. Create the signature for the share record
    let signature_string = format!(
        "{}{}{}{}",
        resource_id,
        current_user_id,
        recipient_user_id,
        PermissionLevel::Write.to_string()
    );
    let signature = {
        let crypto = crypto_utils.lock().await;
        crypto
            .sign_and_hash_message(&signature_string)
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
    };

    // 6. Create the share record
    let share_record = ShareRecord::prepare_share_record(
        resource_id.to_string(),
        current_user_id.to_string(),
        recipient_user_id.to_string(),
        PermissionLevel::Write, // Default permission level
        signature,
    );

    // 7. Get recipient's devices to create vector clocks
    let recipient_devices = repo_ctx
        .device_repo
        .get_devices_by_user_id(&recipient_user_id)
        .await?;

    let recipient_device_ids: Vec<String> =
        recipient_devices.iter().map(|d| d.id.clone()).collect();

    // 8. Create vector clocks for recipient's devices
    let recipient_vector_clocks =
        ResourceVectorClock::create_entries_for_sharing(&resource_id, &recipient_device_ids);

    // 9. Save everything in a single transaction
    repo_ctx
        .resource_repo
        .share_resource_transaction(&new_resource_key, &share_record, &recipient_vector_clocks)
        .await?;

    Ok(())
}

pub async fn get_resource_state_vector(
    resource_id: &str,
    user_id: &str,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<String, RepositoryError> {
    // 1. Get the decrypted resource
    let (decrypted_resource, _) =
        match get_resource(resource_id, repo_ctx, user_id, crypto_utils).await {
            Ok(resource) => resource,
            Err(e) => return Err(RepositoryError::CustomError(e.to_string())),
        };
    let state_vectors = decrypted_resource
        .get_state_vectors()
        .await
        .map_err(|e| RepositoryError::CustomError(e))?;

    Ok(state_vectors)
}

// Also update the generate_updates_for_peer function similarly
pub async fn generate_updates_for_peer(
    resource_id: &str,
    user_id: &str,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
    peer_state_vectors: &String,
) -> Result<String, RepositoryError> {
    // 1. Get the decrypted resource
    let decrypted_resource = match get_resource(resource_id, repo_ctx, user_id, crypto_utils).await
    {
        Ok((resource, _)) => resource,
        Err(e) => return Err(RepositoryError::CustomError(e.to_string())),
    };
    let mut decrypted_resource = decrypted_resource;
    let updates = decrypted_resource
        .sync_updates(peer_state_vectors)
        .await
        .map_err(|e| RepositoryError::CustomError(e))?;

    Ok(updates)
}
/// Apply updates from a peer and generate any updates they might need in return
///
/// # Arguments
/// * `resource_id` - The ID of the resource being updated
/// * `updates` - The updates  and state_vectors received from the peer
///
/// # Returns
/// * `Result<String, RepositoryError>` - (Updates for peer, Current state vector)
pub async fn apply_updates_and_get_peer_updates(
    resource_id: &str,
    user_id: &str,
    updates: &str,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<String, RepositoryError> {
    // 1. Get the current resource with its YJS state
    let (decrypted_resource, encrypted_key) =
        match get_resource(resource_id, repo_ctx, user_id, crypto_utils).await {
            Ok((resource, encrypted_key)) => (resource, encrypted_key),
            Err(e) => return Err(RepositoryError::CustomError(e.to_string())),
        };
    let mut decrypted_resource = decrypted_resource;
    let remote_updates = decrypted_resource
        .sync_updates(updates)
        .await
        .map_err(|e| RepositoryError::CustomError(e))?;

    let encrypted_data = {
        let crypto = crypto_utils.lock().await;
        crypto
            .update_resource(&decrypted_resource.data.to_string(), &encrypted_key)
            .map_err(|e| RepositoryError::CustomError(e.to_string()))?
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
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<String, RepositoryError> {
    // 1. Get the current resource with its YJS state
    let (decrypted_resource, _encrypted_key) =
        match get_resource(resource_id, repo_ctx, user_id, crypto_utils).await {
            Ok((resource, encrypted_key)) => (resource, encrypted_key),
            Err(e) => return Err(RepositoryError::CustomError(e.to_string())),
        };
    let mut decrypted_resource = decrypted_resource;
    let _ = decrypted_resource
        .sync_updates(updates)
        .await
        .map_err(|e| RepositoryError::CustomError(e))?;
    let remote_updates = decrypted_resource
        .sync_updates(peer_state_vectors)
        .await
        .map_err(|e| RepositoryError::CustomError(e))?;

    Ok(remote_updates)
}

pub async fn apply_buffer_and_peer_updates_and_get_remote_updates(
    resource_id: &str,
    user_id: &str,
    remote_updates: &str,
    local_updates: &str,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<String, RepositoryError> {
    // 1. Get the current resource with its YJS state
    let (decrypted_resource, _encrypted_key) =
        match get_resource(resource_id, repo_ctx, user_id, crypto_utils).await {
            Ok((resource, encrypted_key)) => (resource, encrypted_key),
            Err(e) => return Err(RepositoryError::CustomError(e.to_string())),
        };
    let mut decrypted_resource = decrypted_resource;
    let _remote_updates = decrypted_resource
        .sync_updates(local_updates)
        .await
        .map_err(|e| RepositoryError::CustomError(e))?;

    let remote_updates = decrypted_resource
        .sync_updates(remote_updates)
        .await
        .map_err(|e| RepositoryError::CustomError(e))?;

    Ok(remote_updates)
}

pub async fn apply_updates(
    resource_id: &str,
    updates: &str,
    user_id: &str,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<(), String> {
    let (decrypted_resource, encrypted_key) =
        get_resource(resource_id, repo_ctx, user_id, crypto_utils)
            .await
            .map_err(|e| e.to_string())?;
    let mut decrypted_resource = decrypted_resource;
    decrypted_resource.sync_updates(updates).await?;
    let encrypted_data = {
        let crypto = crypto_utils.lock().await;
        crypto
            .update_resource(&decrypted_resource.data.to_string(), &encrypted_key)
            .map_err(|e| e.to_string())?
    };

    repo_ctx
        .resource_repo
        .update_resource(&encrypted_data, resource_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_share_records_for_resource(
    resource_id: &str,
    repo_ctx: &RepositoryContext,
) -> Result<Vec<ShareRecord>, RepositoryError> {
    repo_ctx.share_repo.find_by_resource(resource_id).await
}
pub async fn get_vector_clocks_for_resource(
    resource_id: &str,
    repo_ctx: &RepositoryContext,
) -> Result<Vec<ResourceVectorClock>, RepositoryError> {
    repo_ctx
        .vector_clock_repo
        .get_vector_clocks_for_resource(resource_id)
        .await
}

pub async fn merge_vector_clocks(
    resource_id: &str,
    vector_clocks: &Vec<ResourceVectorClock>,
    repo_ctx: &RepositoryContext,
) -> Result<(Vec<ResourceVectorClock>, Vec<ResourceVectorClock>), RepositoryError> {
    info!("Merging vector clocks for resource {}", resource_id);

    // Get our local vector clocks for this resource
    let local_vector_clocks = repo_ctx
        .vector_clock_repo
        .get_vector_clocks_for_resource(resource_id)
        .await?;

    let merge_result = ResourceVectorClock::merge(&local_vector_clocks, vector_clocks);

    // If local clocks need updating, update our database
    if merge_result.local_needs_update {
        // Update existing vector clocks
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
    repo_ctx: &RepositoryContext,
) -> Result<Vec<ShareRecord>, RepositoryError> {
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
    repo_ctx: &RepositoryContext,
) -> Result<(), RepositoryError> {
    repo_ctx
        .vector_clock_repo
        .update_vector_clocks(&update_clock, &add_clock)
        .await
}
pub async fn add_share_records(
    share_records: &[ShareRecord],
    repo_ctx: &RepositoryContext,
) -> Result<(), RepositoryError> {
    repo_ctx.share_repo.save_many(share_records).await
}
