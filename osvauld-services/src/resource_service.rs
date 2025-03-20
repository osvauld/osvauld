use crypto_utils::{CryptoUtils, encrypt_data_for_users, get_key_id, types::UserPublicKey};
use osvauld_core::models::resource::{DecryptedResource, Resource, ResourceWithKey};
use osvauld_core::models::resource_key::ResourceKey;
use osvauld_core::models::user::User;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::{
    RepositoryError, ResourceKeyRepository, ResourceRepository, ShareRepository, UserRepository,
};
use serde_json::Value;
use std::error::Error;
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
    resource_key_repository: Arc<dyn ResourceKeyRepository>,
}

impl ResourceService {
    pub fn new(
        resource_repository: Arc<dyn ResourceRepository>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
        resource_key_repository: Arc<dyn ResourceKeyRepository>,
    ) -> Self {
        Self {
            resource_repository,
            crypto_utils,
            resource_key_repository,
        }
    }

    pub async fn add_resource(
        &self,
        resource_payload: String,
        resource_type: String,
        folder_id: String,
        user: &User,
    ) -> Result<(Resource, ResourceKey), ResourceServiceError> {
        // Encrypt the resource
        let user_pub_key = UserPublicKey {
            user_id: user.id.clone(),
            public_key: user.public_key.clone(),
            access: "owner".to_string(),
        };
        let encrypted = encrypt_data_for_users(&resource_payload, &[user_pub_key])
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?;

        let resource = Resource::new(
            resource_type,
            encrypted.encrypted_data,
            folder_id,
            "signature".to_string(), // TODO: Implement proper signing
        );
        let resource_key = ResourceKey::new(
            resource.id.clone(),
            user.id.clone(),
            encrypted.access_list[0].encrypted_key.clone(),
            true, // Owner
        );
        log::info!("resource_key{:?}", resource_key);
        Ok((resource, resource_key))
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

    pub async fn get_all_resources(
        &self,
        favourites_only: bool,
    ) -> Result<Vec<DecryptedResource>, ResourceServiceError> {
        // Get current user's ID
        let user_id = self.get_current_user_id().await?;

        // Get resources with their keys
        let resources_with_keys = if favourites_only {
            self.resource_repository.get_favourites(&user_id).await
        } else {
            self.resource_repository.get_all_resources(&user_id).await
        }
        .map_err(ResourceServiceError::RepositoryError)?;

        // Decrypt and return the resources
        self.decrypt_resources(resources_with_keys).await
    }

    // Helper method to handle decryption (reduces duplication)
    async fn decrypt_resources(
        &self,
        resources_with_keys: Vec<ResourceWithKey>,
    ) -> Result<Vec<DecryptedResource>, ResourceServiceError> {
        // Convert to format needed for decryption
        let crypto_resources: Vec<crypto_utils::types::ResourceWithEncryptedKey> =
            resources_with_keys
                .into_iter()
                .map(|rk| crypto_utils::types::ResourceWithEncryptedKey {
                    id: rk.resource.id,
                    resource_type: rk.resource.resource_type,
                    data: rk.resource.data,
                    signature: rk.resource.signature,
                    encrypted_key: rk.encrypted_key,
                    last_accessed: rk.resource.last_accessed,
                    favourite: rk.resource.favourite,
                    folder_id: rk.resource.folder_id,
                })
                .collect();
        // Decrypt and convert to domain model
        let decrypted_resources = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .decrypt_resources(&crypto_resources)
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
        };

        // Convert to domain model
        let resources = decrypted_resources
            .into_iter()
            .map(|res| {
                let parsed_data: Value = serde_json::from_str(&res.data).unwrap_or_else(
                    |_| serde_json::json!({"error": "Failed to parse resource data"}),
                );

                DecryptedResource {
                    id: res.id,
                    resource_type: res.resource_type,
                    data: parsed_data,
                    last_accessed: res.last_accessed,
                    favourite: res.favourite,
                    folder_id: res.folder_id,
                }
            })
            .collect();

        Ok(resources)
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

    // pub async fn share_resource(
    //     &self,
    //     resource_id: String,
    //     recipient_public_key: String,
    // ) -> Result<(ResourceKey, VectorClock), ResourceServiceError> {
    //     // Get current user ID
    //     let current_user_id = self.get_current_user_id().await?;
    //
    //     // Find the resource key for the current user
    //     let resource_key = self
    //         .resource_key_repository
    //         .find_by_resource_and_user(&resource_id, &current_user_id)
    //         .await?;
    //
    //     // Encrypt the key for the recipient
    //     let new_encryption_key = {
    //         let crypto = self.crypto_utils.lock().await;
    //         crypto
    //             .encrypt_key_with_new_pub_key(&resource_key.encrypted_key, &recipient_public_key)
    //             .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
    //     };
    //
    //     // Get recipient's user ID from their public key
    //     let recipient_user_id = get_key_id(&recipient_public_key)
    //         .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?;
    //
    //     // Create a new resource key for the recipient
    //     let new_resource_key = ResourceKey::new(
    //         resource_id.clone(),
    //         recipient_user_id.clone(),
    //         new_encryption_key,
    //         false,
    //     );
    //
    //     // Update the resource's vector clock
    //     // 1. Get the current resource with its vector clock
    //     let resource = self
    //         .resource_repository
    //         .find_by_id_raw(&resource_id)
    //         .await
    //         .map_err(ResourceServiceError::RepositoryError)?;
    //
    //     // 2. Increment the current user's counter in the vector clock
    //     let mut updated_vector_clock = resource.vector_clock.clone();
    //     updated_vector_clock.increment(&current_user_id);
    //
    //     // 3. Ensure the recipient has an entry in the vector clock (initialized to 0)
    //     // This is important so they're included in future causality tracking
    //     updated_vector_clock
    //         .clock
    //         .entry(recipient_user_id)
    //         .or_insert(0);
    //
    //     // Return the new resource key and vector clock
    //     Ok((new_resource_key, updated_vector_clock))
    // }
    //
    pub async fn get_update_payload(
        &self,
        remote_resource: Resource,
        remote_vector_clock: Vec<ResourceVectorClock>,
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

        // Note: This implementation doesn't handle the vector clock yet.
        // As mentioned, that will be handled in a future update.
    }
}
