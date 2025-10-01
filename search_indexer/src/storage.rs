// search_index/storage.rs

use super::search_types::{IndexError, IndexResult, IndexSnapshot};
use crypto_utils::CryptoUtils;
use log::info;
use persistance::database::RepositoryContext;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct SearchIndexStorage {
    encrypted_index_path: PathBuf,
}

impl SearchIndexStorage {
    pub fn new(app_data_dir: &std::path::Path) -> Self {
        Self {
            encrypted_index_path: app_data_dir.join("search_index.enc"),
        }
    }

    /// Save and encrypt the index snapshot
    pub async fn save_encrypted(
        &self,
        snapshot: IndexSnapshot,
        repo_ctx: &Arc<RepositoryContext>,
        user_pub_key: String,
    ) -> IndexResult<()> {
        info!(
            "Saving encrypted search index with {} documents",
            snapshot.documents.len()
        );

        // Serialize the snapshot
        let serialized =
            serde_json::to_vec(&snapshot).map_err(|e| IndexError::SerializationError(e))?;

        let serialized_str = String::from_utf8(serialized)
            .map_err(|e| IndexError::EncryptionError(e.to_string()))?;
        let (encrypted_data, encrypted_key) =
            crypto_utils::encrypt_data_for_user(&serialized_str, &user_pub_key)
                .map_err(|e| IndexError::EncryptionError(e.to_string()))?;

        // Store the encrypted key in the repository
        repo_ctx
            .store_repo
            .add_index_key(&encrypted_key)
            .await
            .map_err(|e| {
                IndexError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

        // Write encrypted data to disk
        fs::write(&self.encrypted_index_path, encrypted_data)
            .await
            .map_err(|e| IndexError::IoError(e))?;

        info!("Successfully saved encrypted index");
        Ok(())
    }

    /// Load and decrypt the index snapshot
    pub async fn load_encrypted(
        &self,
        crypto_utils: &Arc<Mutex<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> IndexResult<Option<IndexSnapshot>> {
        // Check if encrypted index exists
        if !self.encrypted_index_path.exists() {
            info!("No existing encrypted index found");
            return Ok(None);
        }

        info!("Loading encrypted index from disk");

        // Read encrypted data
        let encrypted_data = fs::read_to_string(&self.encrypted_index_path)
            .await
            .map_err(|e| IndexError::IoError(e))?;

        // Get the encrypted key from repository
        let encrypted_key =
            repo_ctx.store_repo.get_index_key().await.map_err(|e| {
                IndexError::DecryptionError(format!("Failed to get index key: {}", e))
            })?;

        // Decrypt the data
        let decrypted_data = {
            let crypto = crypto_utils.lock().await;
            crypto
                .decrypt_resource(&encrypted_data, &encrypted_key)
                .map_err(|e| IndexError::DecryptionError(e.to_string()))?
        };

        // Deserialize the snapshot
        let snapshot: IndexSnapshot =
            serde_json::from_str(&decrypted_data).map_err(|e| IndexError::SerializationError(e))?;

        info!(
            "Successfully loaded {} documents from encrypted index",
            snapshot.documents.len()
        );

        Ok(Some(snapshot))
    }

    /// Delete the encrypted index file
    pub async fn delete(&self) -> IndexResult<()> {
        if self.encrypted_index_path.exists() {
            fs::remove_file(&self.encrypted_index_path)
                .await
                .map_err(|e| IndexError::IoError(e))?;
            info!("Deleted encrypted index file");
        }
        Ok(())
    }
}

