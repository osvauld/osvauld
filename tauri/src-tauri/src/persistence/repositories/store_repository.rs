use crate::domains::models::auth::Certificate;
use crate::domains::repositories::{RepositoryError, StoreRepository};
use async_trait::async_trait;
use std::sync::Arc;
use tauri::{AppHandle, Wry};
use tauri_plugin_store::Store;
use tauri_plugin_store::StoreExt;

pub struct TauriStoreRepository {
    app_handle: AppHandle,
}

impl TauriStoreRepository {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    async fn get_store(&self) -> Arc<Store<Wry>> {
        self.app_handle.store("mobile.bin").unwrap()
    }
}

#[async_trait]
impl StoreRepository for TauriStoreRepository {
    async fn store_certificate(
        &self,
        certificate: &Certificate,
        certificate_key: String,
        salt_key: String,
    ) -> Result<(), RepositoryError> {
        let store = self.get_store().await;
        store.set(certificate_key, certificate.private_key.clone());
        store.set(salt_key, certificate.salt.clone());
        store
            .save()
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn get_certificate(
        &self,
        certificate_key: String,
        salt_key: String,
    ) -> Result<Certificate, RepositoryError> {
        let store = self.get_store().await;
        let private_key = store
            .get(&certificate_key)
            .and_then(|v| v.as_str().map(String::from))
            .ok_or(RepositoryError::NotFound)?;
        let salt = store
            .get(&salt_key)
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
        Ok(store.get("primary_key").is_some())
    }

    async fn store_device_key(&self, device_key: &str) -> Result<(), RepositoryError> {
        let store = self.get_store().await;
        store.set("device_id", device_key.to_string());
        store
            .save()
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn get_device_key(&self) -> Result<String, RepositoryError> {
        let store = self.get_store().await;
        store
            .get("device_id")
            .and_then(|v| v.as_str().map(String::from))
            .ok_or(RepositoryError::NotFound)
    }
}
