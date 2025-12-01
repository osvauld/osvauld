//! Butler: Storage and service layer for Osvauld
//!
//! Butler provides:
//! - redb-based persistent storage
//! - Layer cache for in-memory CRDT operations
//! - Filesystem storage for encrypted static assets
//! - Identity state (set after login, used for crypto operations)
//!
//! ## Architecture
//! Butler is the single entry point (facade) for all storage and identity operations.
//! Services are stateless functions - Butler injects store/identity into them.
//!
//! ## Terminology
//! - Space: Container for Pages (like a project or workspace)
//! - Page: A sub-application instance with its own template
//! - Layer: CRDT data containers (the actual collaborative state)

pub mod error;
pub mod models;
pub mod storage;
pub mod services;

pub use error::{ButlerError, Result};
pub use models::*;
pub use storage::{RedbStore, LayerCache, CachedLayer, LayerCacheStats, AssetStore};
// Auth functions are standalone (don't need Butler instance)
pub use services::{signup, login, is_signed_up, recover, change_passphrase, SignupResult, get_identity_data};

// Stateless service modules
use services::{space_service, node_service, contact_service};

use herald::Identity;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Butler - Main entry point for storage and services
///
/// Owns identity state and storage. All operations go through Butler,
/// which injects dependencies into stateless service functions.
pub struct Butler {
    store: Arc<RedbStore>,
    layer_cache: Arc<RwLock<LayerCache>>,
    identity: Arc<RwLock<Option<Identity>>>,
}

impl Butler {
    /// Create a new Butler instance
    pub fn new(store: Arc<RedbStore>, layer_cache: Arc<RwLock<LayerCache>>) -> Self {
        Self {
            store,
            layer_cache,
            identity: Arc::new(RwLock::new(None)),
        }
    }

    // ==================== Identity Management ====================

    /// Set identity after login
    pub async fn set_identity(&self, identity: Identity) {
        let mut guard = self.identity.write().await;
        *guard = Some(identity);
    }

    /// Get identity (errors if not logged in)
    pub async fn get_identity(&self) -> Result<Identity> {
        let guard = self.identity.read().await;
        guard.clone().ok_or(ButlerError::NotLoggedIn)
    }

    /// Clear identity on logout
    pub async fn clear_identity(&self) {
        let mut guard = self.identity.write().await;
        *guard = None;
    }

    /// Check if logged in
    pub async fn is_logged_in(&self) -> bool {
        self.identity.read().await.is_some()
    }

    // ==================== Key Accessors ====================

    /// Get device key for Transport initialization
    pub async fn device_key(&self) -> Result<[u8; 32]> {
        let identity = self.get_identity().await?;
        Ok(identity.secret_device_key())
    }

    /// Get signing key for permit operations
    pub async fn signing_key(&self) -> Result<[u8; 32]> {
        let identity = self.get_identity().await?;
        Ok(identity.secret_signing_key())
    }

    /// Get encryption key for crypto operations
    pub async fn encryption_key(&self) -> Result<[u8; 32]> {
        let identity = self.get_identity().await?;
        Ok(identity.secret_encryption_key())
    }

    // ==================== User Info ====================

    /// Get user info for UI display
    pub async fn user_info(&self) -> Result<UserInfo> {
        let identity = self.get_identity().await?;
        let identity_data = get_identity_data(&self.store)?
            .ok_or(ButlerError::NotLoggedIn)?;

        Ok(UserInfo {
            did: identity.did().to_string(),
            username: identity_data.username,
            public_key: identity.public_signing_key().to_vec(),
        })
    }

    // ==================== Permit Operations (wraps gurkha) ====================

    /// Issue one-time permit for node connection
    pub async fn issue_one_time_permit(&self, role: &str) -> Result<(String, String)> {
        let signing_key = self.signing_key().await?;
        let (permit, _cid) = gurkha::issue_one_time(&signing_key, role)
            .await
            .map_err(|e| ButlerError::PermitError(format!("{:?}", e)))?;
        let pub_key = gurkha::get_public_key(&signing_key);
        Ok((permit, pub_key))
    }

    /// Issue folder viewer permit
    pub async fn issue_folder_viewer_permit(&self, folder_id: &str) -> Result<(String, String)> {
        let signing_key = self.signing_key().await?;
        let (permit, _cid) = gurkha::issue_folder_viewer_auth(&signing_key, folder_id)
            .await
            .map_err(|e| ButlerError::PermitError(format!("{:?}", e)))?;
        let pub_key = gurkha::get_public_key(&signing_key);
        Ok((permit, pub_key))
    }

    /// Issue peer connection permit (for long-lived owner/node connections)
    pub async fn issue_peer_connection_permit(&self, peer_pubkey: &str, relationship: &str) -> Result<(String, String)> {
        let signing_key = self.signing_key().await?;
        let permit = gurkha::issue_peer_connection(&signing_key, peer_pubkey, relationship)
            .await
            .map_err(|e| ButlerError::PermitError(format!("{:?}", e)))?;
        let pub_key = gurkha::get_public_key(&signing_key);
        Ok((permit, pub_key))
    }

    // ==================== Space Operations ====================

    pub async fn create_space(&self, name: String, owner_did: String, permit_template_json: &str) -> Result<Space> {
        let signing_key = self.signing_key().await?;
        space_service::create_space(&self.store, name, owner_did, &signing_key, permit_template_json).await
    }

    pub fn get_space(&self, space_id: &str) -> Result<Option<Space>> {
        space_service::get_space(&self.store, space_id)
    }

    pub fn list_spaces(&self) -> Result<Vec<Space>> {
        space_service::list_all_spaces(&self.store)
    }

    pub fn list_root_spaces(&self) -> Result<Vec<Space>> {
        space_service::list_root_spaces(&self.store)
    }

    pub fn delete_space(&self, space_id: &str) -> Result<bool> {
        space_service::delete_space(&self.store, space_id, true)
    }

    /// Track that space was shared with a user (stores pubkey only)
    pub fn share_space(&self, space_id: &str, user_pubkey: String) -> Result<()> {
        space_service::share_space(&self.store, space_id, user_pubkey)
    }

    /// Set permit for a space
    pub fn set_space_permit(&self, space_id: &str, permit: String) -> Result<()> {
        space_service::set_space_permit(&self.store, space_id, permit)
    }

    // ==================== Page Operations ====================

    /// Extract layer names from a permit template JSON
    ///
    /// Looks for `layers` field and returns names of CRDT layers.
    pub fn extract_layer_names_from_template(permit_template_json: &str) -> Vec<String> {
        let Ok(template) = serde_json::from_str::<serde_json::Value>(permit_template_json) else {
            return Vec::new();
        };

        let Some(layers) = template.get("layers").and_then(|d| d.as_object()) else {
            return Vec::new();
        };

        layers
            .iter()
            .filter_map(|(name, config)| {
                let layer_type = config.get("type").and_then(|t| t.as_str()).unwrap_or("crdt");
                if layer_type == "crdt" {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub async fn create_page(
        &self,
        space_id: &str,
        name: &str,
        page_type: PageType,
        layer_names: Vec<String>,
    ) -> Result<Page> {
        let identity = self.get_identity().await?;
        let owner_did = identity.did().to_string();
        let owner_public_key = identity.public_encryption_key();
        space_service::create_page(
            &self.store,
            space_id.to_string(),
            name.to_string(),
            owner_did,
            &owner_public_key,
            page_type,
            layer_names,
        ).await
    }

    pub fn get_page(&self, page_id: &str) -> Result<Option<PageData>> {
        space_service::find_page_by_id(&self.store, page_id)
    }

    pub fn list_pages(&self, space_id: &str) -> Result<Vec<Page>> {
        space_service::list_pages(&self.store, space_id)
    }

    pub fn delete_page(&self, space_id: &str, page_id: &str) -> Result<bool> {
        space_service::delete_page(&self.store, space_id, page_id)
    }

    /// Get all pages accessible by the logged-in user
    pub async fn get_accessible_pages(&self) -> Result<Vec<Page>> {
        let identity = self.get_identity().await?;
        let user_did = identity.did().to_string();
        space_service::get_accessible_pages(&self.store, &user_did)
    }

    // ==================== Encrypted Page Operations (need identity) ====================

    pub async fn get_decrypted_page(&self, page_id: &str) -> Result<DecryptedPage> {
        let identity = self.get_identity().await?;
        let user_did = identity.did().to_string();
        let secret_key = identity.secret_encryption_key();
        space_service::get_decrypted_page(&self.store, page_id, &user_did, &secret_key).await
    }

    pub fn update_page_layers(
        &self,
        page_id: &str,
        secret_key: &[u8; 32],
        layer_updates: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Page> {
        space_service::update_page_layers(&self.store, page_id, secret_key, layer_updates)
    }

    pub async fn update_page_layers_auto(
        &self,
        page_id: &str,
        layer_updates: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Page> {
        let identity = self.get_identity().await?;
        let secret_key = identity.secret_encryption_key();
        space_service::update_page_layers(&self.store, page_id, &secret_key, layer_updates)
    }

    pub async fn prepare_space_for_publish(
        &self,
        space_id: &str,
        node_public_key: &str,
    ) -> Result<(SpaceMeta, String)> {
        let identity = self.get_identity().await?;
        let owner_did = identity.did().to_string();
        space_service::prepare_space_for_publish(
            &self.store,
            space_id,
            &owner_did,
            node_public_key,
            &identity.secret_signing_key(),
        ).await
    }

    pub async fn prepare_page_for_publish(
        &self,
        page_id: &str,
        node_public_key: &str,
    ) -> Result<(PageMeta, String, std::collections::HashMap<String, Vec<u8>>)> {
        let identity = self.get_identity().await?;
        let owner_did = identity.did().to_string();
        space_service::prepare_page_for_publish(
            &self.store,
            page_id,
            &owner_did,
            node_public_key,
            &identity.secret_signing_key(),
            &identity.secret_encryption_key(),
        ).await
    }

    // ==================== Node Operations ====================

    pub fn register_my_node(
        &self,
        node_did: String,
        device_id: String,
        iroh_node_id: String,
        node_addr: Option<String>,
    ) -> Result<NodeInfo> {
        let identity_data = get_identity_data(&self.store)?
            .ok_or(ButlerError::NotLoggedIn)?;
        node_service::register_my_node(
            &self.store,
            identity_data.did,
            node_did,
            device_id,
            iroh_node_id,
            node_addr,
        )
    }

    pub fn get_my_node(&self) -> Result<Option<NodeInfo>> {
        let identity_data = get_identity_data(&self.store)?
            .ok_or(ButlerError::NotLoggedIn)?;
        node_service::get_my_node(&self.store, &identity_data.did)
    }

    pub fn add_sovereign_node(&self, connection_string: &str) -> std::result::Result<SovereignNode, String> {
        node_service::add_sovereign_node(&self.store, connection_string)
    }

    pub fn get_sovereign_node(&self, node_id: &str) -> Result<Option<SovereignNode>> {
        node_service::get_sovereign_node(&self.store, node_id)
    }

    pub fn list_sovereign_nodes(&self) -> Result<Vec<SovereignNode>> {
        node_service::list_sovereign_nodes(&self.store)
    }

    pub fn list_connected_sovereign_nodes(&self) -> Result<Vec<SovereignNode>> {
        node_service::list_connected_sovereign_nodes(&self.store)
    }

    pub fn set_sovereign_node_connected(&self, node_id: &str, connected: bool) -> Result<bool> {
        node_service::set_sovereign_node_connected(&self.store, node_id, connected)
    }

    pub fn set_sovereign_node_permit(&self, node_id: &str, permit: String) -> Result<bool> {
        node_service::set_sovereign_node_permit(&self.store, node_id, permit)
    }

    pub fn delete_sovereign_node(&self, node_id: &str) -> Result<bool> {
        node_service::delete_sovereign_node(&self.store, node_id)
    }

    /// Store the permit we issued to a sovereign node
    pub fn set_sovereign_node_permit_for_them(&self, node_id: &str, permit: String) -> Result<bool> {
        node_service::set_sovereign_node_permit_for_them(&self.store, node_id, permit)
    }

    // ==================== Owner Operations (for Node mode) ====================

    pub fn get_owner(&self) -> Result<Option<OwnerInfo>> {
        node_service::get_owner(&self.store)
    }

    pub fn set_owner(&self, owner: &OwnerInfo) -> Result<()> {
        node_service::set_owner(&self.store, owner)
    }

    pub fn has_owner(&self) -> Result<bool> {
        node_service::has_owner(&self.store)
    }

    /// Store the permit we issued to owner (for Node mode handshake)
    pub fn set_owner_permit_for_owner(&self, permit: String) -> Result<bool> {
        node_service::set_owner_permit_for_owner(&self.store, permit)
    }

    /// Store the permit owner issued to us (for Node mode handshake)
    pub fn set_owner_permit_from_owner(&self, permit: String) -> Result<bool> {
        node_service::set_owner_permit_from_owner(&self.store, permit)
    }

    /// Update owner's last connected timestamp
    pub fn update_owner_last_connected(&self) -> Result<bool> {
        node_service::update_owner_last_connected(&self.store)
    }

    // ==================== Contact Operations (Node-side for viewer shares) ====================

    /// Get a contact by DID
    pub fn get_contact(&self, user_did: &str) -> Result<Option<ContactData>> {
        contact_service::get_contact(&self.store, user_did)
    }

    /// Create or update a contact
    pub fn upsert_contact(&self, contact: &ContactData) -> Result<()> {
        contact_service::upsert_contact(&self.store, contact)
    }

    /// List all contacts
    pub fn list_contacts(&self) -> Result<Vec<ContactData>> {
        contact_service::list_contacts(&self.store)
    }

    /// Add a share to a contact (also updates shares_by_page index)
    pub fn add_share_to_contact(
        &self,
        user_did: &str,
        page_id: &str,
        share: ShareInfo,
    ) -> Result<()> {
        contact_service::add_share_to_contact(&self.store, user_did, page_id, share)
    }

    /// Get all user DIDs who have a share for a given page
    pub fn get_users_for_page(&self, page_id: &str) -> Result<Vec<String>> {
        contact_service::get_users_for_page(&self.store, page_id)
    }

    /// Get contacts with their shares for a page
    pub fn get_contacts_for_page(&self, page_id: &str) -> Result<Vec<ContactData>> {
        contact_service::get_contacts_for_page(&self.store, page_id)
    }

    // ==================== Internal Accessors (rarely needed) ====================

    pub fn store(&self) -> &Arc<RedbStore> {
        &self.store
    }
}
