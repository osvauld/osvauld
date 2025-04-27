use crypto_utils::{CryptoUtils, encrypt_data_for_user, get_key_id};
use osvauld_core::models::device::Device;
use osvauld_core::models::document::{
    apply_updates_and_generate_peer_updates, generate_updates_for_peer, get_state_vector,
};
use osvauld_core::models::resource::{DecryptedResource, Resource, ResourceWithKey};
use osvauld_core::models::resource_key::ResourceKey;
use osvauld_core::models::share_record::{PermissionLevel, ShareOperation, ShareRecord};
use osvauld_core::models::sync_record::{DeviceRecordSet, SyncRecord, SyncRecordSet};
use osvauld_core::models::sync_types::{OperationType, ResourceType};
use osvauld_core::models::user::User;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::{
    DeviceRepository, RepositoryError, ResourceKeyRepository, ResourceRepository, ShareRepository,
    SyncRepository, UserRepository, VectorClockRepository,
};
use serde_json::Value;
use std::result::Result::Ok;
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

pub struct ResourceService {
    resource_repository: Arc<dyn ResourceRepository>,
    crypto_utils: Arc<Mutex<CryptoUtils>>,
    vector_clock_repo: Arc<dyn VectorClockRepository>,
    resource_key_repo: Arc<dyn ResourceKeyRepository>,
    device_repository: Arc<dyn DeviceRepository>,
    user_repository: Arc<dyn UserRepository>,
    share_repository: Arc<dyn ShareRepository>,
    sync_repository: Arc<dyn SyncRepository>,
}

impl ResourceService {
    pub fn new(
        resource_repository: Arc<dyn ResourceRepository>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
        vector_clock_repo: Arc<dyn VectorClockRepository>,
        resource_key_repo: Arc<dyn ResourceKeyRepository>,
        device_repository: Arc<dyn DeviceRepository>,
        user_repository: Arc<dyn UserRepository>,
        share_repository: Arc<dyn ShareRepository>,
        sync_repository: Arc<dyn SyncRepository>,
    ) -> Self {
        Self {
            resource_repository,
            crypto_utils,
            vector_clock_repo,
            resource_key_repo,
            device_repository,
            user_repository,
            share_repository,
            sync_repository,
        }
    }

    pub async fn add_resource(
        &self,
        resource_payload: String,
        resource_type: String,
        folder_id: String,
        user: &User,
        current_device_id: &str,
    ) -> Result<
        (
            Resource,
            ResourceKey,
            SyncRecordSet,
            SyncRecordSet,
            Vec<ResourceVectorClock>,
            ShareRecord,
        ),
        ResourceServiceError,
    > {
        // Encrypt the resource using the new function
        let (encrypted_data, encrypted_key) =
            encrypt_data_for_user(&resource_payload, &user.public_key)
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?;

        let resource = Resource::new(
            resource_type,
            encrypted_data,
            folder_id,
            "signature".to_string(), // TODO: Implement proper signing
        );

        let resource_key =
            ResourceKey::new(resource.id.clone(), user.id.clone(), encrypted_key, true);

        let share_record = ShareRecord::prepare_share_record(
            resource.id.clone(),
            user.id.clone(),
            user.id.clone(),
            PermissionLevel::Admin,
            "signature".to_string(),
        );

        let (sync_record_set, share_record_set, vector_clocks) = self
            .prepare_resource_to_sync(&resource, &share_record, &user.id, current_device_id)
            .await?;

        Ok((
            resource,
            resource_key,
            sync_record_set,
            share_record_set,
            vector_clocks,
            share_record,
        ))
    }

    pub async fn delete_resource(&self, resource_id: String) -> Result<(), RepositoryError> {
        self.resource_repository
            .soft_delete_resource(&resource_id)
            .await
    }

    pub async fn toggle_fav(&self, resource_id: String) -> Result<(), RepositoryError> {
        self.resource_repository.toggle_fav(&resource_id).await
    }

    pub async fn update_last_accessed(&self, resource_id: String) -> Result<(), RepositoryError> {
        self.resource_repository
            .update_last_accessed(&resource_id)
            .await
    }

    pub async fn update_resources(
        &self,
        resource_id: String,
        data: String,
    ) -> Result<(String, String), ResourceServiceError> {
        let user_id = self.get_current_user_id().await?;
        let old_resource = self
            .resource_repository
            .find_by_id(&resource_id, &user_id)
            .await
            .map_err(ResourceServiceError::RepositoryError)?;

        let encrypted = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .update_resource(&data, &old_resource.encrypted_key)
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
        };
        Ok((encrypted, user_id))
    }

    pub async fn get_resource(
        &self,
        resource_id: String,
    ) -> Result<DecryptedResource, ResourceServiceError> {
        // Get encrypted resource from repository
        let user_id = self.get_current_user_id().await?;
        let resource_with_key = self
            .resource_repository
            .find_by_id(&resource_id, &user_id)
            .await
            .map_err(ResourceServiceError::RepositoryError)?;

        // Use the helper function with a single-element vector
        let decrypted_resources = self.decrypt_resources(vec![resource_with_key]).await?;

        // Extract the single result (with proper error handling)
        decrypted_resources
            .into_iter()
            .next()
            .ok_or(ResourceServiceError::CryptoError(
                "Failed to decrypt resource".to_string(),
            ))
    }

    pub async fn get_resources_for_folder(
        &self,
        folder_id: String,
    ) -> Result<Vec<DecryptedResource>, ResourceServiceError> {
        // Get current user's ID
        let user_id = self.get_current_user_id().await?;

        // Get resources with their keys in a single repository call
        let resources_with_keys = self
            .resource_repository
            .find_by_folder(&folder_id, &user_id)
            .await
            .map_err(ResourceServiceError::RepositoryError)?;

        // Decrypt and return the resources
        self.decrypt_resources(resources_with_keys).await
    }

    pub async fn get_all_resources(&self) -> Result<Vec<DecryptedResource>, ResourceServiceError> {
        // Get current user's ID
        let user_id = self.get_current_user_id().await?;

        // Get resources with their keys
        let resources_with_keys = self
            .resource_repository
            .get_all_resources(&user_id)
            .await
            .map_err(ResourceServiceError::RepositoryError)?;

        // Decrypt and return the resources
        self.decrypt_resources(resources_with_keys).await
    }

    async fn decrypt_resources(
        &self,
        resources_with_keys: Vec<ResourceWithKey>,
    ) -> Result<Vec<DecryptedResource>, ResourceServiceError> {
        // Initialize vector to store decrypted resources
        let mut decrypted_resources = Vec::with_capacity(resources_with_keys.len());

        // Lock crypto_utils once before the loop
        let crypto = self.crypto_utils.lock().await;

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

    // Helper to get current user ID
    async fn get_current_user_id(&self) -> Result<String, ResourceServiceError> {
        let public_key = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .get_public_key()
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
        };

        get_key_id(&public_key).map_err(|e| ResourceServiceError::CryptoError(e.to_string()))
    }

    pub async fn get_update_payload(
        &self,
        remote_resource: Resource,
    ) -> Result<(DecryptedResource, DecryptedResource), ResourceServiceError> {
        // Get the user ID
        let user_id = self.get_current_user_id().await?;

        // Fetch the local resource with its key from the repository
        let local_resource_with_key = self
            .resource_repository
            .find_by_id(&remote_resource.id, &user_id)
            .await
            .map_err(ResourceServiceError::RepositoryError)?;

        // We can use the same encrypted key for both resources since the key is symmetric
        // and has already been encrypted with the user's public key
        let remote_resource_with_key = ResourceWithKey {
            resource: remote_resource,
            encrypted_key: local_resource_with_key.encrypted_key.clone(),
        };

        // Now decrypt both resources
        let mut decrypted_resources = self
            .decrypt_resources(vec![local_resource_with_key, remote_resource_with_key])
            .await?;

        // Extract the decrypted resources (there should be exactly 2)
        if decrypted_resources.len() != 2 {
            return Err(ResourceServiceError::CryptoError(
                "Failed to decrypt resources for comparison".to_string(),
            ));
        }

        // The second item should be the remote resource
        let remote_decrypted = decrypted_resources.pop().unwrap();
        // The first item should be the local resource
        let local_decrypted = decrypted_resources.pop().unwrap();

        // Return both decrypted resources
        Ok((local_decrypted, remote_decrypted))
    }

    pub async fn update_merged_payload(
        &self,
        merged_payload: &str,
        remote_vector_clock: &[ResourceVectorClock],
        resource_id: &str,
    ) -> Result<(String, (Vec<ResourceVectorClock>, Vec<ResourceVectorClock>)), ResourceServiceError>
    {
        let current_user_id = self.get_current_user_id().await?;
        let resource_key = self
            .resource_key_repo
            .find_by_resource_and_user(resource_id, &current_user_id)
            .await?;

        let encrypted_data = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .update_resource(&merged_payload, &resource_key.encrypted_key)
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
        };
        let local_vector_clock = self
            .vector_clock_repo
            .get_vector_clocks_for_resource(resource_id)
            .await?;

        let merged_vector_clock =
            ResourceVectorClock::merge(&local_vector_clock, &remote_vector_clock);

        if merged_vector_clock.local_needs_update {
            self.vector_clock_repo
                .update_vector_clocks(
                    &merged_vector_clock.update_local,
                    &merged_vector_clock.add_local,
                )
                .await
                .map_err(|e| ResourceServiceError::RepositoryError(e))?;
        }

        self.resource_repository
            .update_resource(&encrypted_data, resource_id)
            .await?;
        Ok((
            encrypted_data,
            (
                merged_vector_clock.add_remote,
                merged_vector_clock.update_remote,
            ),
        ))
    }

    pub async fn prepare_resource_to_sync(
        &self,
        resource: &Resource,
        share_record: &ShareRecord,
        user_id: &str,
        current_device_id: &str,
    ) -> Result<(SyncRecordSet, SyncRecordSet, Vec<ResourceVectorClock>), RepositoryError> {
        let devices = self
            .device_repository
            .get_devices_by_user_id(&user_id)
            .await?;
        let sync_record_set = SyncRecord::create_resource_sync_record(
            resource.id.clone(),
            current_device_id.to_string(),
            &devices,
        );
        let share_record_set = SyncRecord::create_share_sync_record(
            share_record.id.clone(),
            current_device_id.to_string(),
            &devices,
        );
        let device_ids: Vec<String> = devices
            .iter()
            .map(|d| d.id.clone())
            .chain(std::iter::once(current_device_id.to_string()))
            .collect();
        let vector_clocks = ResourceVectorClock::create_initial_entries(
            &resource.id,
            &device_ids,
            current_device_id,
        );
        Ok((sync_record_set, share_record_set, vector_clocks))
    }

    // Helper function for collecting device information
    async fn collect_devices_for_sharing(
        &self,
        resource_id: &str,
        recipient_user_id: &str,
    ) -> Result<(Vec<Device>, Vec<Device>, Vec<String>), ResourceServiceError> {
        // Get recipient's devices
        let recipient_user_devices = self
            .device_repository
            .get_devices_by_user_id(recipient_user_id)
            .await?;

        // Get existing share records
        let shared_records = self
            .share_repository
            .find_by_resource_and_operation(resource_id, &ShareOperation::Share.to_string())
            .await?;
        let shared_record_ids: Vec<String> =
            shared_records.iter().map(|sr| sr.id.clone()).collect();

        // Get existing shared user devices
        let shared_user_ids: Vec<String> = shared_records
            .iter()
            .map(|sr| sr.recipient_user_id.clone())
            .collect();

        let shared_user_devices = if !shared_user_ids.is_empty() {
            self.device_repository
                .get_devices_by_user_ids(&shared_user_ids)
                .await?
        } else {
            Vec::new()
        };

        // Combine all devices
        let mut all_devices = recipient_user_devices.clone();
        all_devices.extend(shared_user_devices.clone().into_iter());

        Ok((recipient_user_devices, all_devices, shared_record_ids))
    }

    // Helper function for resource key and vector clock preparation
    async fn prepare_resource_key_and_vectors(
        &self,
        resource_id: &str,
        recipient_user_id: &str,
        current_user_id: &str,
        recipient_device_ids: &[String],
    ) -> Result<(ResourceKey, Vec<ResourceVectorClock>), ResourceServiceError> {
        // Get the resource key for the current user
        let resource_key = self
            .resource_key_repo
            .find_by_resource_and_user(resource_id, current_user_id)
            .await?;

        // Get recipient user to access their public key
        let recipient_user = self
            .user_repository
            .get_user_by_id(recipient_user_id)
            .await?;

        // Encrypt the key for the recipient
        let new_encryption_key = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .encrypt_key_with_new_pub_key(
                    &resource_key.encrypted_key,
                    &recipient_user.public_key,
                )
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
        };

        // Create the new resource key
        let new_resource_key = ResourceKey::new(
            resource_id.to_string(),
            recipient_user_id.to_string(),
            new_encryption_key,
            false, // Not owner
        );

        // Create vector clocks for recipient's devices
        let recipient_vector_clocks =
            ResourceVectorClock::create_entries_for_sharing(resource_id, recipient_device_ids);

        Ok((new_resource_key, recipient_vector_clocks))
    }

    // Main share_resource function
    pub async fn share_resource(
        &self,
        recipient_user_id: String,
        resource_id: String,
        current_user_id: &str,
        current_device_id: &str,
    ) -> Result<
        (
            ResourceKey,
            ShareRecord,
            Vec<ResourceVectorClock>,
            SyncRecordSet,
            DeviceRecordSet,
            DeviceRecordSet,
        ),
        ResourceServiceError,
    > {
        // Collect all necessary device IDs
        let (recipient_devices, all_devices, shared_record_ids) = self
            .collect_devices_for_sharing(&resource_id, &recipient_user_id)
            .await?;
        let recipient_device_ids: Vec<String> =
            recipient_devices.iter().map(|rd| rd.id.clone()).collect();
        // Prepare resource key and vector clocks
        let (new_resource_key, recipient_vector_clocks) = self
            .prepare_resource_key_and_vectors(
                &resource_id,
                &recipient_user_id,
                current_user_id,
                &recipient_device_ids,
            )
            .await?;

        // Create the share record
        let share_record = ShareRecord::prepare_share_record(
            resource_id.clone(),
            current_user_id.to_string(),
            recipient_user_id,
            PermissionLevel::Write,
            "signature".to_string(),
        );
        let resource_sync_record = self
            .sync_repository
            .get_sync_record_by_resource_and_operation(
                &resource_id,
                &OperationType::Create.to_string(),
                &ResourceType::Resource.to_string(),
            )
            .await?;

        // Extract all device IDs for the resource device record set
        let all_device_ids: Vec<String> = all_devices.iter().map(|d| d.id.clone()).collect();

        // Create device records for resource sync
        let resource_device_record_set = SyncRecord::create_resource_share_records(
            resource_sync_record.id,
            current_device_id.to_string(),
            &recipient_devices,
            &all_device_ids,
        );

        let (sync_records, existing_device_records) = self
            .sync_repository
            .get_sync_and_device_records_by_resource_ids(
                &shared_record_ids,
                &OperationType::Create.to_string(),
            )
            .await?;
        //prepare the records
        let (share_sync_record_set, device_record_set) =
            SyncRecord::prepare_sync_records_for_sharing(
                &share_record.id,
                current_device_id,
                &recipient_devices,
                &all_devices,
                &sync_records,
                &existing_device_records,
            );
        Ok((
            new_resource_key,
            share_record,
            recipient_vector_clocks,
            share_sync_record_set,
            device_record_set,
            resource_device_record_set,
        ))
    }
    pub async fn get_resource_by_id_direct(
        &self,
        resource_id: &str,
        user_id: &str,
    ) -> Result<DecryptedResource, ResourceServiceError> {
        // Get current user's ID

        // Get the specific resource with its key
        let resource_with_key = self
            .resource_repository
            .find_by_id(resource_id, user_id)
            .await
            .map_err(ResourceServiceError::RepositoryError)?;

        // Use the existing decrypt_resources method with a single item
        let decrypted_resources = self.decrypt_resources(vec![resource_with_key]).await?;

        // Return the first (and only) result
        decrypted_resources
            .into_iter()
            .next()
            .ok_or(ResourceServiceError::CryptoError(
                "Failed to decrypt resource".to_string(),
            ))
    }

    pub async fn get_resource_state_vector(
        &self,
        resource_id: &str,
    ) -> Result<Vec<u8>, RepositoryError> {
        // 1. Get the decrypted resource
        let decrypted_resource = match self.get_resource(resource_id.to_string()).await {
            Ok(resource) => resource,
            Err(e) => return Err(RepositoryError::CustomError(e.to_string())),
        };

        // 2. Extract yjs_state from the data field
        // The data field is already parsed as a Value, so we can access it directly
        let yjs_state = match decrypted_resource.data.get("yjs_state") {
            Some(state) => {
                // Convert the JSON array to a Vec<u8>
                match state.as_array() {
                    Some(array) => {
                        array
                            .iter()
                            .try_fold(Vec::new(), |mut acc, v| match v.as_u64() {
                                Some(n) if n <= 255 => {
                                    acc.push(n as u8);
                                    Ok(acc)
                                }
                                Some(n) => Err(RepositoryError::CustomError(format!(
                                    "yjs_state contains value {} which exceeds u8 range",
                                    n
                                ))),
                                None => Err(RepositoryError::CustomError(
                                    "yjs_state contains non-numeric value".to_string(),
                                )),
                            })?
                    }
                    None => {
                        return Err(RepositoryError::CustomError(
                            "yjs_state is not an array".to_string(),
                        ));
                    }
                }
            }
            None => {
                return Err(RepositoryError::CustomError(
                    "yjs_state not found in resource data".to_string(),
                ));
            }
        };

        // 3. Use document.rs to get the state vector
        let state_vector = match get_state_vector(&yjs_state).await {
            Ok(vector) => vector,
            Err(e) => return Err(RepositoryError::CustomError(e)),
        };

        Ok(state_vector)
    }

    // Also update the generate_updates_for_peer function similarly
    pub async fn generate_updates_for_peer(
        &self,
        resource_id: &str,
        peer_state_vector: &[u8],
    ) -> Result<Vec<u8>, RepositoryError> {
        // 1. Get the decrypted resource
        let decrypted_resource = match self.get_resource(resource_id.to_string()).await {
            Ok(resource) => resource,
            Err(e) => return Err(RepositoryError::CustomError(e.to_string())),
        };

        // 2. Extract yjs_state from the data field
        let yjs_state = match decrypted_resource.data.get("yjs_state") {
            Some(state) => match state.as_array() {
                Some(array) => array
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect::<Vec<u8>>(),
                None => {
                    return Err(RepositoryError::CustomError(
                        "yjs_state is not an array".to_string(),
                    ));
                }
            },
            None => {
                return Err(RepositoryError::CustomError(
                    "yjs_state not found in resource data".to_string(),
                ));
            }
        };

        // 3. Use document.rs to generate updates for the peer
        let updates = match generate_updates_for_peer(&yjs_state, peer_state_vector).await {
            Ok(updates) => updates,
            Err(e) => return Err(RepositoryError::CustomError(e)),
        };

        Ok(updates)
    }
    /// Apply updates from a peer and generate any updates they might need in return
    ///
    /// # Arguments
    /// * `resource_id` - The ID of the resource being updated
    /// * `updates` - The updates received from the peer
    /// * `peer_state_vector` - The state vector from the peer
    ///
    /// # Returns
    /// * `Result<(Vec<u8>, Vec<u8>), RepositoryError>` - (Updates for peer, Current state vector)
    pub async fn apply_updates_and_get_peer_updates(
        &self,
        resource_id: &str,
        updates: &[u8],
        peer_state_vector: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), RepositoryError> {
        // 1. Get the current resource with its YJS state
        let decrypted_resource = match self.get_resource(resource_id.to_string()).await {
            Ok(resource) => resource,
            Err(e) => return Err(RepositoryError::CustomError(e.to_string())),
        };

        // 2. Extract the current yjs_state from the data field
        let current_yjs_state = match decrypted_resource.data.get("yjs_state") {
            Some(state) => match state.as_array() {
                Some(array) => array
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect::<Vec<u8>>(),
                None => {
                    return Err(RepositoryError::CustomError(
                        "yjs_state is not an array".to_string(),
                    ));
                }
            },
            None => {
                return Err(RepositoryError::CustomError(
                    "yjs_state not found in resource data".to_string(),
                ));
            }
        };

        // 3. Use document.rs to apply the updates and generate any updates for the peer
        let (peer_updates, current_state_vector) = match apply_updates_and_generate_peer_updates(
            &current_yjs_state,
            updates,
            peer_state_vector,
        )
        .await
        {
            Ok((updates, state_vector)) => (updates, state_vector),
            Err(e) => return Err(RepositoryError::CustomError(e)),
        };

        // 4. Return both the updates needed by the peer and the current state vector
        Ok((peer_updates, current_state_vector))
    }
}
