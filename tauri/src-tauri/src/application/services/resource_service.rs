use crate::domains::models::resource::{DecryptedResource, Resource};
use crate::domains::models::resource_key::ResourceKey;
use crate::domains::repositories::{RepositoryError, ResourceRepository};
use crate::types::UpdateResources;
use crypto_utils::{encrypt_data_for_users, get_key_id, types::UserPublicKey, CryptoUtils};
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
}

impl ResourceService {
    pub fn new(
        resource_repository: Arc<dyn ResourceRepository>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
    ) -> Self {
        Self {
            resource_repository,
            crypto_utils,
        }
    }

    pub async fn add_resource(
        &self,
        resource_payload: String,
        resource_type: String,
        folder_id: String,
    ) -> Result<Resource, ResourceServiceError> {
        // Encrypt the resource
        let public_key = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .get_public_key()
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
        };
        let user_id = get_key_id(&public_key)
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?;
        let user_pub_key = UserPublicKey {
            user_id: user_id.clone(),
            public_key,
            access: "owner".to_string(),
        };
        let encrypted = encrypt_data_for_users(&resource_payload, &[user_pub_key])
            .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?;

        // Create and save the resource
        let resource = Resource::new(
            resource_type,
            encrypted.encrypted_data,
            folder_id,
            "signature".to_string(), // TODO: Implement proper signing
        );
        let resource_key = ResourceKey::new(
            resource.id.clone(),
            user_id,
            encrypted.access_list[0].encrypted_key.clone(),
            true, // Owner
        );
        self.resource_repository
            .save(&resource)
            .await
            .map_err(ResourceServiceError::RepositoryError)?;

        Ok(resource)
    }

    pub async fn get_resources_for_folder(
        &self,
        folder_id: String,
    ) -> Result<Vec<DecryptedResource>, ResourceServiceError> {
        // Get encrypted resources
        let encrypted_resources = self
            .resource_repository
            .find_by_folder(&folder_id)
            .await
            .map_err(ResourceServiceError::RepositoryError)?;

        // Convert to crypto utils format
        let crypto_resources = encrypted_resources
            .into_iter()
            .map(|cred| crypto_utils::types::ResourceWithEncryptedKey {
                id: cred.id,
                resource_type: cred.resource_type,
                data: cred.data,
                signature: cred.signature,
                encrypted_key: cred.encrypted_key,
                last_accessed: cred.last_accessed,
                favourite: cred.favourite,
                folder_id: cred.folder_id,
            })
            .collect();

        // Decrypt resources
        let decrypted_resources = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .decrypt_resources(crypto_resources)
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
        };

        // Parse and convert to domain model
        let resources = decrypted_resources
            .into_iter()
            .map(|cred| {
                let parsed_data: Value = serde_json::from_str(&cred.data).unwrap_or_else(
                    |_| serde_json::json!({"error": "Failed to parse resource data"}),
                );

                DecryptedResource {
                    id: cred.id,
                    resource_type: cred.resource_type,
                    data: parsed_data,
                    last_accessed: cred.last_accessed,
                    favourite: cred.favourite,
                    folder_id: cred.folder_id,
                }
            })
            .collect();

        Ok(resources)
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

    pub async fn get_all_resources(
        &self,
        favourite: bool,
    ) -> Result<Vec<DecryptedResource>, ResourceServiceError> {
        let encrypted_resources = if favourite {
            self.resource_repository
                .get_favourites()
                .await
                .map_err(ResourceServiceError::RepositoryError)?
        } else {
            self.resource_repository
                .get_all_resources()
                .await
                .map_err(ResourceServiceError::RepositoryError)?
        };
        // Convert to crypto utils format
        let crypto_resources = encrypted_resources
            .into_iter()
            .map(|cred| crypto_utils::types::ResourceWithEncryptedKey {
                id: cred.id,
                resource_type: cred.resource_type,
                data: cred.data,
                signature: cred.signature,
                last_accessed: cred.last_accessed,
                favourite: cred.favourite,
                folder_id: cred.folder_id,
            })
            .collect();

        // Decrypt resources
        let decrypted_resources = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .decrypt_resources(crypto_resources)
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
        };

        // Parse and convert to domain model
        let resources = decrypted_resources
            .into_iter()
            .map(|cred| {
                let parsed_data: Value = serde_json::from_str(&cred.data).unwrap_or_else(
                    |_| serde_json::json!({"error": "Failed to parse resource data"}),
                );

                DecryptedResource {
                    id: cred.id,
                    resource_type: cred.resource_type,
                    data: parsed_data,
                    last_accessed: cred.last_accessed,
                    favourite: cred.favourite,
                    folder_id: cred.folder_id,
                }
            })
            .collect();

        Ok(resources)
    }

    pub async fn update_resources(
        &self,
        input: UpdateResources,
    ) -> Result<(), ResourceServiceError> {
        let old_resource = self
            .resource_repository
            .find_by_id(&input.id)
            .await
            .map_err(ResourceServiceError::RepositoryError)?;

        let encrypted = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .update_resource(input.data, old_resource.encrypted_key)
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?
        };
        self.resource_repository
            .update_resource(encrypted, old_resource.id)
            .await?;
        Ok(())
    }

    pub async fn get_resource(
        &self,
        resource_id: String,
    ) -> Result<DecryptedResource, ResourceServiceError> {
        // Get encrypted resource from repository
        let encrypted_resource = self
            .resource_repository
            .find_by_id(&resource_id)
            .await
            .map_err(ResourceServiceError::RepositoryError)?;

        // Convert to crypto utils format
        let crypto_resource = crypto_utils::types::ResourceWithEncryptedKey {
            id: encrypted_resource.id,
            resource_type: encrypted_resource.resource_type,
            data: encrypted_resource.data,
            signature: encrypted_resource.signature,
            last_accessed: encrypted_resource.last_accessed,
            favourite: encrypted_resource.favourite,
            folder_id: encrypted_resource.folder_id,
        };

        // Decrypt resource
        let decrypted_resource = {
            let crypto = self.crypto_utils.lock().await;
            let decrypted_resources = crypto
                .decrypt_resources(vec![crypto_resource])
                .map_err(|e| ResourceServiceError::CryptoError(e.to_string()))?;
            decrypted_resources.into_iter().next().unwrap()
        };

        // Parse and convert to domain model
        let parsed_data: Value = serde_json::from_str(&decrypted_resource.data)
            .unwrap_or_else(|_| serde_json::json!({"error": "Failed to parse resource data"}));

        Ok(DecryptedResource {
            id: decrypted_resource.id,
            resource_type: decrypted_resource.resource_type,
            data: parsed_data,
            last_accessed: decrypted_resource.last_accessed,
            favourite: decrypted_resource.favourite,
            folder_id: decrypted_resource.folder_id,
        })
    }
}
