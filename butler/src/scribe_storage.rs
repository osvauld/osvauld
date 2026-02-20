//! Storage trait implementations for Scribe actor
//!
//! Butler's implementations of the scribe storage traits using RedbStore.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use gurkha::errors::ServiceResult;
use scribe::{LayerStorage, PeerResolver, PeerVectorStorage, PermitIssuer, Result, ScribeError};

use crate::storage::RedbStore;
use tracing::{info, instrument, warn};

// Permit Issuer Implementation

/// Butler's implementation of PermitIssuer (node mode only)
///
/// Holds signing key + node's page permit. Scoped to a single page.
/// Scribe never sees the signing key — uses this trait to request permits.
pub struct ButlerPermitIssuer {
    signing_key: [u8; 32],
    node_permit_token: String,
    page_id: String,
    store: Arc<RedbStore>,
}

impl ButlerPermitIssuer {
    pub fn new(
        signing_key: [u8; 32],
        node_permit_token: String,
        page_id: String,
        store: Arc<RedbStore>,
    ) -> Self {
        Self {
            signing_key,
            node_permit_token,
            page_id,
            store,
        }
    }
}

fn block_on_gurkha<T>(future: impl Future<Output = ServiceResult<T>>) -> ServiceResult<T> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            if matches!(
                handle.runtime_flavor(),
                tokio::runtime::RuntimeFlavor::MultiThread
            ) {
                tokio::task::block_in_place(|| handle.block_on(future))
            } else {
                futures::executor::block_on(future)
            }
        }
        Err(_) => futures::executor::block_on(future),
    }
}

impl PermitIssuer for ButlerPermitIssuer {
    #[instrument(skip_all, fields(audience = %audience, layer_name = %layer_name))]
    fn issue_layer_permit(
        &self,
        audience: &str,
        layer_name: &str,
        config: gurkha::LayerConfig,
        intent_cid: Option<&str>,
    ) -> Result<(String, String)> {
        info!(page_id = %self.page_id, "ButlerPermitIssuer::issue_layer_permit");
        block_on_gurkha(gurkha::issue_layer_permit(
            &self.signing_key,
            &self.node_permit_token,
            audience,
            &self.page_id,
            layer_name,
            config,
            intent_cid,
        ))
        .map_err(|e| {
            warn!(error = %e, "Permit issuance failed");
            ScribeError::Other(format!("Permit issuance failed: {}", e))
        })
    }

    #[instrument(skip_all, fields(audience = %audience))]
    fn list_layer_authority_permits_for_audience(
        &self,
        audience: &str,
    ) -> Result<Vec<(String, u64, String)>> {
        self.store
            .list_layer_authority_permits_for_audience(&self.page_id, audience)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }

    #[instrument(skip_all, fields(audience = %audience, layer_name = %layer_name))]
    fn get_layer_authority_permit(
        &self,
        audience: &str,
        layer_name: &str,
    ) -> Result<Option<(u64, String)>> {
        self.store
            .get_layer_authority_permit(&self.page_id, layer_name, audience)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }

    #[instrument(skip_all, fields(audience = %audience, layer_name = %layer_name))]
    fn has_layer_access_permit(&self, audience: &str, layer_name: &str) -> Result<bool> {
        self.store
            .has_layer_permit(&self.page_id, audience, layer_name)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }

    #[instrument(skip_all, fields(audience = %audience, layer_name = %layer_name, version = version))]
    fn issue_layer_authority_permit(
        &self,
        audience: &str,
        layer_name: &str,
        config: gurkha::LayerConfig,
        authorized_peers: Option<Vec<String>>,
        version: u64,
    ) -> Result<(String, String)> {
        let (token, cid) = block_on_gurkha(gurkha::issue_layer_authority_permit(
            &self.signing_key,
            &self.node_permit_token,
            audience,
            layer_name,
            config,
            authorized_peers,
            version,
        ))
        .map_err(|e| {
            warn!(error = %e, "Layer authority permit issuance failed");
            ScribeError::Other(format!("Layer authority permit issuance failed: {}", e))
        })?;

        self.store
            .store_layer_authority_permit(&self.page_id, layer_name, audience, version, &token)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))?;

        Ok((token, cid))
    }

    /// Store authority permit received from a creator (node-side).
    ///
    /// **Context**: Node received LayerSubscribeAck from creator with authority permit.
    /// Reuses the LAYER_AUTHORITY_PERMITS table keyed by creator_did as audience.
    #[instrument(skip_all, fields(creator_did = %creator_did, layer_name = %layer_name, version = version))]
    fn store_authority_permit(
        &self,
        creator_did: &str,
        layer_name: &str,
        authority_token: &str,
        version: u64,
    ) -> Result<()> {
        self.store
            .store_layer_authority_permit(
                &self.page_id,
                layer_name,
                creator_did,
                version,
                authority_token,
            )
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }

    /// Get stored authority for a layer (any audience — for node checking authorization).
    ///
    /// **Context**: Node needs to check if any creator has stored authority for this layer.
    /// Scans all authority permits for the page to find a match by layer_name suffix.
    #[instrument(skip_all, fields(layer_name = %layer_name))]
    fn get_authority_for_layer(&self, layer_name: &str) -> Result<Option<(String, u64, String)>> {
        self.store
            .get_authority_for_layer(&self.page_id, layer_name)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }
}

// Layer Storage Implementation

/// Butler's implementation of LayerStorage
///
/// Saves layer snapshots to RedbStore with AES encryption.
/// Scoped to a single page (page_id passed at construction).
pub struct ButlerLayerStorage {
    store: Arc<RedbStore>,
    page_id: String,
    aes_key: [u8; 32],
}

impl ButlerLayerStorage {
    pub fn new(store: Arc<RedbStore>, page_id: String, aes_key: [u8; 32]) -> Self {
        Self {
            store,
            page_id,
            aes_key,
        }
    }
}

impl LayerStorage for ButlerLayerStorage {
    #[instrument(skip_all)]
    fn save_layer(&self, layer_name: &str, data: &[u8]) -> Result<()> {
        info!(page_id = %self.page_id, layer_name = %layer_name, data_len = data.len(), "ButlerLayerStorage::save_layer");

        // Encrypt layer data
        let encrypted = herald::encrypt_symmetric(&self.aes_key, data)
            .map_err(|e| ScribeError::Other(format!("Encryption failed: {}", e)))?;

        // Save to store
        self.store
            .put_layer(&self.page_id, layer_name, &encrypted)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }
}

// Peer Vector Storage Implementation

/// Butler's implementation of PeerVectorStorage
///
/// Saves/loads peer state vectors to RedbStore.
/// Scoped to a single page (page_id passed at construction).
pub struct ButlerPeerVectorStorage {
    store: Arc<RedbStore>,
    page_id: String,
}

impl ButlerPeerVectorStorage {
    pub fn new(store: Arc<RedbStore>, page_id: String) -> Self {
        Self { store, page_id }
    }
}

impl PeerVectorStorage for ButlerPeerVectorStorage {
    #[instrument(skip_all)]
    fn load_vectors(
        &self,
        user_did: &str,
        device_id: &str,
    ) -> Result<Option<HashMap<String, Vec<u8>>>> {
        self.store
            .get_peer_vectors(&self.page_id, user_did, device_id)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }

    #[instrument(skip_all)]
    fn save_vectors(
        &self,
        user_did: &str,
        device_id: &str,
        vectors: &HashMap<String, Vec<u8>>,
    ) -> Result<()> {
        self.store
            .put_peer_vectors(&self.page_id, user_did, device_id, vectors)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }
}

// Peer Resolver Implementation

/// Butler's implementation of PeerResolver (node mode only)
///
/// Lists authorized users and loads their permits from RedbStore.
/// Scoped to a single page (page_id passed at construction).
pub struct ButlerPeerResolver {
    store: Arc<RedbStore>,
    page_id: String,
}

impl ButlerPeerResolver {
    pub fn new(store: Arc<RedbStore>, page_id: String) -> Self {
        Self { store, page_id }
    }
}

impl PeerResolver for ButlerPeerResolver {
    #[instrument(skip_all)]
    fn list_authorized_users(&self) -> Vec<String> {
        self.store
            .list_authorized_users_for_page(&self.page_id)
            .unwrap_or_default()
    }

    #[instrument(skip_all)]
    fn load_user_permit(&self, user_did: &str) -> Option<String> {
        self.store
            .get_user_page_permit(&self.page_id, user_did)
            .ok()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: [u8; 32] = [1u8; 32];

    fn load_shop_template() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("sample_apps/my-shop/permit_template.json");
        std::fs::read_to_string(path).unwrap()
    }

    /// C5: ButlerPermitIssuer issues valid layer permits via gurkha
    #[test]
    fn test_butler_permit_issuer() {
        let template = load_shop_template();

        // Create owner → node delegation chain
        let (owner_token, _) = futures::executor::block_on(gurkha::issue_page_owner_token(
            &TEST_KEY, "page1", &template,
        ))
        .unwrap();
        let (node_token, _) = futures::executor::block_on(gurkha::delegate_page(
            &TEST_KEY,
            &owner_token,
            "node",
            "did:key:node",
        ))
        .unwrap();

        // Create ButlerPermitIssuer
        let temp_path = std::env::temp_dir().join(format!(
            "butler_permit_issuer_test_{}.redb",
            uuid::Uuid::new_v4()
        ));
        let store = Arc::new(crate::storage::RedbStore::open(&temp_path).unwrap());
        let issuer = ButlerPermitIssuer::new(TEST_KEY, node_token, "page1".to_string(), store);

        // Issue a layer permit
        let config = gurkha::LayerConfig {
            sync: true,
            write: true,
            layer_type: None,
        };
        let (token, cid) = issuer
            .issue_layer_permit(
                "did:key:viewer",
                "page1/channels/did:key:alice/general/messages",
                config,
                None,
            )
            .unwrap();

        assert!(!token.is_empty());
        assert!(!cid.is_empty());

        // Verify the token is a valid UCAN with the expected layer
        let permit = gurkha::Permit::from_token(&token).unwrap();
        assert_eq!(permit.layers().len(), 1);
        assert!(permit
            .layers()
            .contains_key("page1/channels/did:key:alice/general/messages"));
    }
}
