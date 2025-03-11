use osvauld_core::modelc::auth::Certificate;
useosvauld_core::repositories::{AuthRepository, RepositoryError};
use async_trait::async_trait;
use std::sync::Arc;
use tauri::{AppHandle, Wry};
use tauri_plugin_store::Store;
use tauri_plugin_store::StoreExt;
pub struct TauriStoreAuthRepository {
    app_handle: AppHandle,
}

impl TauriStoreAuthRepository {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    async fn get_store(&self) -> Arc<Store<Wry>> {
        self.app_handle.store("sqlite1_store18.bin").unwrap()
    }
}

#[async_trait]
impl AuthRepository for TauriStoreAuthRepository {
    async fn store_certificate(&self, certificate: &Certificate) -> Result<(), RepositoryError> {
        let store = self.get_store().await;
        store.set("certificate", certificate.private_key.clone());
        store.set("salt", certificate.salt.clone());
        store
            .save()
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn get_certificate(&self) -> Result<Certificate, RepositoryError> {
        let store = self.get_store().await;
        let private_key = store
            .get("certificate")
            .and_then(|v| v.as_str().map(String::from))
            .ok_or(RepositoryError::NotFound)?;

        let salt = store
            .get("salt")
            .and_then(|v| v.as_str().map(String::from))
            .ok_or(RepositoryError::NotFound)?;

        Ok(Certificate {
            private_key,
            public_key: String::new(), // This will be generated when needed
            salt,
        })
    }

    async fn is_signed_up(&self) -> Result<bool, RepositoryError> {
        let store = self.get_store().await;
        Ok(store.get("certificate").is_some())
    }
}
