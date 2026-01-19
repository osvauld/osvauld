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
pub mod scribe;
pub mod merge;
pub mod sync;
pub mod runtime;

pub use error::{ButlerError, Result};
pub use models::*;
pub use storage::{RedbStore, LayerCache, CachedLayer, LayerCacheStats, AssetStore};
// Auth functions are standalone (don't need Butler instance)
pub use services::{signup, login, is_signed_up, recover, change_passphrase, SignupResult, get_identity_data};
// Scribe actor exports
pub use scribe::{
    Scribe, ScribeMessage, ScribeArgs, ScribeState, SyncEvent,
    LoroDelta, ListOp, PageEvent, PageEventType,
    BroadcastPayload, LayerWritePermission, SyncPolicy, SyncConfig,
    SaveLayerFn, LoadPeerVectorFn, SavePeerVectorFn, ListAuthorizedUsersFn, LoadUserPermitFn,
    EphemeralEvent, EphemeralBroadcast, EphemeralOutbound,
};
pub use runtime::{
    HeadlessRuntime,
    LoroBindings, PermitBindings, LuaLoroList, LuaLoroMap,
    // Loro <-> Lua
    loro_value_to_lua, lua_to_loro_value,
    // JSON <-> Lua
    json_to_lua, lua_to_json,
    // JSON <-> Loro
    json_to_loro_value, loro_value_to_json,
    // Pattern matching
    matches_layer_pattern,
};

// Stateless service modules
use services::{space_service, page_service, publish_service, node_service, contact_service};

use herald::Identity;
use ractor::ActorRef;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Scribe entry with last access time for LRU eviction
struct ScribeEntry {
    actor: ActorRef<ScribeMessage>,
    last_accessed: Instant,
}

/// Butler - Main entry point for storage and services
///
/// Owns identity state and storage. All operations go through Butler,
/// which injects dependencies into stateless service functions.
pub struct Butler {
    store: Arc<RedbStore>,
    layer_cache: Arc<RwLock<LayerCache>>,
    identity: Arc<RwLock<Option<Identity>>>,
    /// Active Scribe actors (page_id → ScribeEntry)
    scribes: RwLock<HashMap<String, ScribeEntry>>,
    /// Per-page locks for serializing open_page calls
    page_locks: RwLock<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Maximum number of open pages
    max_open_pages: usize,
    /// Channel to emit sync events (EnsureSync) from Scribes to Coordinator
    /// Uses RwLock for interior mutability (allows setting after Arc wrapping)
    sync_event_tx: RwLock<Option<tokio::sync::mpsc::Sender<SyncEvent>>>,
}

/// Default maximum number of open pages
const DEFAULT_MAX_OPEN_PAGES: usize = 50;

impl Butler {
    /// Create a new Butler instance
    pub fn new(store: Arc<RedbStore>, layer_cache: Arc<RwLock<LayerCache>>) -> Self {
        Self {
            store,
            layer_cache,
            identity: Arc::new(RwLock::new(None)),
            scribes: RwLock::new(HashMap::new()),
            page_locks: RwLock::new(HashMap::new()),
            max_open_pages: DEFAULT_MAX_OPEN_PAGES,
            sync_event_tx: RwLock::new(None),
        }
    }

    /// Create a new Butler instance with custom max open pages
    pub fn with_max_pages(store: Arc<RedbStore>, layer_cache: Arc<RwLock<LayerCache>>, max_open_pages: usize) -> Self {
        Self {
            store,
            layer_cache,
            identity: Arc::new(RwLock::new(None)),
            scribes: RwLock::new(HashMap::new()),
            page_locks: RwLock::new(HashMap::new()),
            max_open_pages,
            sync_event_tx: RwLock::new(None),
        }
    }

    /// Set the sync event channel for Scribe-to-Coordinator communication.
    ///
    /// **Context**: Called by the application after creating both Butler and Coordinator.
    /// Uses interior mutability so this works on Arc<Butler>.
    pub async fn set_sync_event_tx(&self, tx: tokio::sync::mpsc::Sender<SyncEvent>) {
        let mut guard = self.sync_event_tx.write().await;
        *guard = Some(tx);
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

    // ==================== Auth Operations (Sync) ====================

    /// Check if user has signed up
    pub fn is_signed_up(&self) -> Result<bool> {
        is_signed_up(&self.store)
    }

    /// Sign up a new user (blocking)
    pub fn signup_sync(&self, username: &str, passphrase: &str) -> Result<SignupResult> {
        signup(&self.store, username, passphrase)
    }

    /// Login and return identity (blocking)
    pub fn login_sync(&self, passphrase: &str) -> Result<Identity> {
        login(&self.store, passphrase)
    }

    /// Get stored identity data
    pub fn identity_data(&self) -> Result<Option<IdentityData>> {
        get_identity_data(&self.store)
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
            encryption_key: identity.public_encryption_key().to_vec(),
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

    /// Issue space viewer permit
    pub async fn issue_space_viewer_permit(&self, space_id: &str) -> Result<(String, String)> {
        let signing_key = self.signing_key().await?;
        let (permit, _cid) = gurkha::issue_space_viewer_auth(&signing_key, space_id)
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

    /// Get space data with permit and shares
    pub fn get_space_data(&self, space_id: &str) -> Result<Option<SpaceData>> {
        space_service::get_space_with_shares(&self.store, space_id)
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

    /// Delegate space permit to node for publishing
    ///
    /// Creates a delegated permit with relationship="node" for the given node pubkey.
    pub async fn delegate_space_to_node(&self, space_id: &str, node_pubkey: &str) -> Result<String> {
        let signing_key = self.signing_key().await?;

        // Get the space with permit data
        let space_data = self.store.get_space(space_id)?
            .ok_or_else(|| ButlerError::NotFound(format!("Space {} not found", space_id)))?;

        let space_permit = space_data.permit
            .ok_or_else(|| ButlerError::PermitError("Space has no permit".to_string()))?;

        // Delegate to node using the "node" template from SPACE_TEMPLATE.delegation.node
        let (delegated, _cid) = gurkha::delegate_space(&signing_key, &space_permit, "node", node_pubkey)
            .await
            .map_err(|e| ButlerError::PermitError(format!("Failed to delegate: {:?}", e)))?;

        Ok(delegated)
    }

    /// Store a space received via publish (Node mode)
    ///
    /// Called by Node when it receives PublishSpace from owner.
    /// The space and permit come from the owner - we just store them.
    pub fn store_published_space(&self, space: &Space, permit: &str) -> Result<()> {
        space_service::store_published_space(&self.store, space, permit)
    }

    /// Store a space received from another peer with source tracking
    ///
    /// Called by Viewer when receiving SpaceData from node.
    /// The source_node_id is stored so viewer knows which node to sync back to.
    pub fn store_published_space_with_source(
        &self,
        space: &Space,
        permit: &str,
        source_node_id: Option<&str>,
    ) -> Result<()> {
        space_service::store_published_space_with_source(&self.store, space, permit, source_node_id)
    }

    /// Get the source node for a page (for lazy sync)
    ///
    /// Returns the node_id that gave us this page's space, if any.
    /// Used by Scribe to know which node to sync edits back to.
    pub fn get_source_node_for_page(&self, page_id: &str) -> Result<Option<String>> {
        page_service::get_source_node_for_page(&self.store, page_id)
    }

    /// Get sovereign node for owner sync
    ///
    /// Returns the first connected sovereign node, if any.
    /// Used for owner role lazy sync - owner syncs edits to their sovereign node.
    pub fn get_sovereign_node_for_sync(&self) -> Result<Option<String>> {
        let nodes = self.list_connected_sovereign_nodes()?;
        // Return DID (identity layer) not node_id (transport layer)
        // EnsureSync uses DID to resolve device info
        Ok(nodes.into_iter().next().map(|n| n.did))
    }

    /// List page IDs for a space (for sync)
    pub fn list_page_ids_for_space(&self, space_id: &str) -> Result<Vec<String>> {
        let pages = self.list_pages(space_id)?;
        Ok(pages.into_iter().map(|p| p.id).collect())
    }

    /// Mark space as published on a specific node
    ///
    /// DEPRECATED: Use store_user_space_permit instead for permit-based tracking
    pub fn mark_space_published(&self, space_id: &str, node_id: &str) -> Result<()> {
        space_service::mark_space_published(&self.store, space_id, node_id)
    }

    /// Store a node's permit for a space (Owner mode - proves space is published)
    ///
    /// **Context**: Owner receives permit from node after PublishSpaceAck.
    /// This permit proves the space is published to that node.
    pub fn store_user_space_permit(&self, space_id: &str, node_did: &str, permit: &str) -> Result<()> {
        self.store.put_user_space_permit(space_id, node_did, permit)
    }

    /// Get a node's permit for a space (Owner mode - check if published)
    pub fn get_user_space_permit(&self, space_id: &str, node_did: &str) -> Result<Option<String>> {
        self.store.get_user_space_permit(space_id, node_did)
    }

    /// List all nodes that have issued permits for a space (= published nodes)
    ///
    /// **Context**: UI needs to show which nodes a space is published to
    /// **We return**: DIDs of nodes with permits for this space
    pub fn get_nodes_with_space_permits(&self, space_id: &str) -> Result<Vec<String>> {
        self.store.list_nodes_with_space_permits(space_id)
    }

    /// Issue a space permit from node to owner
    ///
    /// **Context**: Node receives PublishSpace, issues permit back to owner
    /// **We do**: Create node→owner permit proving space is published
    pub async fn issue_space_permit_to_owner(
        &self,
        space_id: &str,
        owner_pubkey: &str,
    ) -> Result<String> {
        let signing_key = self.signing_key().await?;

        let (permit, _cid) = gurkha::issue_space_node_to_owner(
            &signing_key,
            space_id,
            owner_pubkey,
        ).await.map_err(|e| ButlerError::PermitError(format!("Failed to issue permit: {:?}", e)))?;

        Ok(permit)
    }

    // ==================== Page Operations ====================

    /// Extract layer names from a permit template JSON
    ///
    /// Looks for `owner_template.layers` field (matching gurkha's expected structure).
    /// Returns all layer names (both CRDT and asset types need storage).
    pub fn extract_layer_names_from_template(permit_template_json: &str) -> Vec<String> {
        let Ok(template) = serde_json::from_str::<serde_json::Value>(permit_template_json) else {
            return Vec::new();
        };

        // Extract layers from owner_template (matches gurkha structure)
        let Some(layers) = template
            .get("owner_template")
            .and_then(|ot| ot.get("layers"))
            .and_then(|l| l.as_object())
        else {
            return Vec::new();
        };

        // Return all layer names (both crdt and asset types need storage)
        layers.keys().cloned().collect()
    }

    pub async fn create_page(
        &self,
        space_id: &str,
        name: &str,
        layer_names: Vec<String>,
        permit_template_json: &str,
    ) -> Result<Page> {
        let identity = self.get_identity().await?;
        let owner_did = identity.did().to_string();
        let owner_public_key = identity.public_encryption_key();
        let signing_key = identity.secret_signing_key();
        page_service::create_page(
            &self.store,
            space_id.to_string(),
            name.to_string(),
            owner_did,
            &owner_public_key,
            &signing_key,
            layer_names,
            permit_template_json,
        ).await
    }

    pub fn get_page(&self, page_id: &str) -> Result<Option<PageData>> {
        page_service::find_page_by_id(&self.store, page_id)
    }

    pub fn list_pages(&self, space_id: &str) -> Result<Vec<Page>> {
        page_service::list_pages(&self.store, space_id)
    }

    pub fn delete_page(&self, space_id: &str, page_id: &str) -> Result<bool> {
        page_service::delete_page(&self.store, space_id, page_id)
    }

    /// Get all pages (no filtering - if stored, user has access)
    pub fn get_pages(&self) -> Result<Vec<Page>> {
        page_service::get_pages(&self.store)
    }

    // ==================== Encrypted Page Operations (need identity) ====================

    pub async fn get_decrypted_page(&self, page_id: &str) -> Result<(DecryptedPage, [u8; 32])> {
        let identity = self.get_identity().await?;
        let user_did = identity.did().to_string();
        let secret_key = identity.secret_encryption_key();
        page_service::get_decrypted_page(&self.store, page_id, &user_did, &secret_key).await
    }

    pub fn update_page_layers(
        &self,
        page_id: &str,
        secret_key: &[u8; 32],
        layer_updates: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Page> {
        page_service::update_page_layers(&self.store, page_id, secret_key, layer_updates)
    }

    pub async fn update_page_layers_auto(
        &self,
        page_id: &str,
        layer_updates: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Page> {
        let identity = self.get_identity().await?;
        let secret_key = identity.secret_encryption_key();
        page_service::update_page_layers(&self.store, page_id, &secret_key, layer_updates)
    }

    // ==================== App File Storage ====================

    /// Save an app file to page storage
    ///
    /// **Context**: User uploads a multi-file app (e.g., .slint, .lua files)
    /// **We do**: Encrypt with page AES key, store with "file:{path}" layer name
    pub async fn save_page_file(&self, page_id: &str, file_path: &str, content: &str) -> Result<()> {
        // Get page's AES key
        let (_decrypted, aes_key) = self.get_decrypted_page(page_id).await?;

        // Encrypt content with page's AES key
        let encrypted = herald::encrypt_symmetric(&aes_key, content.as_bytes())
            .map_err(|e| ButlerError::Encryption(e.to_string()))?;

        // Store with prefixed layer name to avoid collisions
        let layer_name = format!("file:{}", file_path);
        self.store.put_layer(page_id, &layer_name, &encrypted)?;

        tracing::info!(page_id = %page_id, file_path = %file_path, size = content.len(), "Saved app file to page");
        Ok(())
    }

    /// Get an app file from page storage
    ///
    /// **Context**: Loading a multi-file app
    /// **We return**: Decrypted file content or None if file doesn't exist
    pub async fn get_page_file(&self, page_id: &str, file_path: &str) -> Result<Option<String>> {
        let layer_name = format!("file:{}", file_path);

        // Check if layer exists
        let encrypted = match self.store.get_layer(page_id, &layer_name)? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        // Get page's AES key to decrypt
        let (_decrypted, aes_key) = self.get_decrypted_page(page_id).await?;

        // Decrypt file
        let file_bytes = herald::decrypt_symmetric(&aes_key, &encrypted)
            .map_err(|e| ButlerError::Encryption(format!("Decryption failed: {}", e)))?;

        let content = String::from_utf8(file_bytes)
            .map_err(|e| ButlerError::Encryption(format!("Invalid UTF-8 in file: {}", e)))?;

        tracing::info!(page_id = %page_id, file_path = %file_path, size = content.len(), "Loaded app file from page");
        Ok(Some(content))
    }

    /// List all app files in a page
    ///
    /// **Context**: Need to enumerate files in a multi-file app
    /// **We return**: List of file paths (without "file:" prefix)
    pub fn list_page_files(&self, page_id: &str) -> Result<Vec<String>> {
        let layers = self.store.list_layer_names(page_id)?;
        let files: Vec<String> = layers
            .into_iter()
            .filter_map(|name| name.strip_prefix("file:").map(|s| s.to_string()))
            .collect();
        Ok(files)
    }

    // ==================== Static File Storage (Binary Blobs) ====================

    /// Save a static binary file to page storage
    ///
    /// **Context**: Store binary assets (images, fonts) that don't need CRDT merging
    /// **We do**: Encrypt with page AES key, store with "static:{path}" layer name
    pub async fn save_static_file(&self, page_id: &str, file_path: &str, content: &[u8]) -> Result<()> {
        // Get page's AES key
        let (_decrypted, aes_key) = self.get_decrypted_page(page_id).await?;

        // Encrypt content with page's AES key
        let encrypted = herald::encrypt_symmetric(&aes_key, content)
            .map_err(|e| ButlerError::Encryption(e.to_string()))?;

        // Store with static: prefix
        let layer_name = format!("static:{}", file_path);
        self.store.put_layer(page_id, &layer_name, &encrypted)?;

        tracing::info!(page_id = %page_id, file_path = %file_path, size = content.len(), "Saved static file");
        Ok(())
    }

    /// Get a static binary file from page storage
    ///
    /// **Context**: Load binary assets (images, fonts)
    /// **We return**: Decrypted bytes or None if file doesn't exist
    pub async fn get_static_file(&self, page_id: &str, file_path: &str) -> Result<Option<Vec<u8>>> {
        let layer_name = format!("static:{}", file_path);

        // Check if layer exists
        let encrypted = match self.store.get_layer(page_id, &layer_name)? {
            Some(data) => data,
            None => return Ok(None),
        };

        // Get page's AES key
        let (_decrypted, aes_key) = self.get_decrypted_page(page_id).await?;

        // Decrypt
        let content = herald::decrypt_symmetric(&aes_key, &encrypted)
            .map_err(|e| ButlerError::Encryption(format!("Decryption failed: {}", e)))?;

        tracing::info!(page_id = %page_id, file_path = %file_path, size = content.len(), "Loaded static file");
        Ok(Some(content))
    }

    /// List all static files in a page
    ///
    /// **Context**: Enumerate binary assets in a page
    /// **We return**: List of file paths (without "static:" prefix)
    pub fn list_static_files(&self, page_id: &str) -> Result<Vec<String>> {
        let layers = self.store.list_layer_names(page_id)?;
        let files: Vec<String> = layers
            .into_iter()
            .filter_map(|name| name.strip_prefix("static:").map(|s| s.to_string()))
            .collect();
        Ok(files)
    }

    /// Delete a static file from page storage
    pub fn delete_static_file(&self, page_id: &str, file_path: &str) -> Result<()> {
        let layer_name = format!("static:{}", file_path);
        self.store.delete_layer(page_id, &layer_name)?;
        tracing::info!(page_id = %page_id, file_path = %file_path, "Deleted static file");
        Ok(())
    }

    // ==================== App Layer Operations ====================

    /// List all apps in a page
    ///
    /// **Context**: Find all `app:*` layers in a page
    /// **We return**: List of app names (without "app:" prefix)
    pub fn list_apps(&self, page_id: &str) -> Result<Vec<String>> {
        let layers = self.store.list_layer_names(page_id)?;
        let apps: Vec<String> = layers
            .into_iter()
            .filter_map(|name| name.strip_prefix("app:").map(|s| s.to_string()))
            .collect();
        Ok(apps)
    }

    /// List data layers in a page
    ///
    /// **Context**: Find layers that store app data (not app code, not files, not static assets)
    /// **We return**: Layer names that don't have app:, file:, or static: prefix
    pub fn list_data_layers(&self, page_id: &str) -> Result<Vec<String>> {
        let layers = self.store.list_layer_names(page_id)?;
        let data_layers: Vec<String> = layers
            .into_iter()
            .filter(|name| {
                !name.starts_with("app:") &&
                !name.starts_with("file:") &&
                !name.starts_with("static:")
            })
            .collect();
        Ok(data_layers)
    }

    /// Get all files from an app layer
    ///
    /// **Context**: Load app files from `app:{app_name}` LoroMap layer
    /// **We return**: HashMap of file_path -> content
    pub async fn get_app_files(&self, page_id: &str, app_name: &str) -> Result<HashMap<String, String>> {
        let layer_name = format!("app:{}", app_name);

        // Get decrypted page data
        let (decrypted, _aes_key) = self.get_decrypted_page(page_id).await?;

        // Check if app layer exists
        let layer_bytes = decrypted.docs.get(&layer_name)
            .ok_or_else(|| ButlerError::NotFound(format!("App '{}' not found in page", app_name)))?;

        if layer_bytes.is_empty() {
            return Ok(HashMap::new());
        }

        // Reconstruct Layer from snapshot and extract files
        let layer = Layer::from_snapshot(layer_bytes)
            .map_err(|e| ButlerError::Layer(e.to_string()))?;

        let files = layer.get_all_files();

        tracing::debug!(
            page_id = %page_id,
            app_name = %app_name,
            file_count = files.len(),
            "Loaded app files from layer"
        );

        Ok(files)
    }

    /// Get a single file from an app layer
    ///
    /// **Context**: Load specific file from `app:{app_name}` LoroMap layer
    /// **We return**: File content or None if not found
    pub async fn get_app_file(&self, page_id: &str, app_name: &str, file_path: &str) -> Result<Option<String>> {
        let files = self.get_app_files(page_id, app_name).await?;
        Ok(files.get(file_path).cloned())
    }

    // ==================== App Import Operations ====================

    /// Import an app from a directory
    ///
    /// **Context**: Development workflow - import sample app to Butler
    /// **We do**: Delegate to app_service::import_app_from_directory
    pub async fn import_app(&self, space_id: &str, app_dir: &Path) -> Result<Page> {
        services::app_service::import_app_from_directory(self, space_id, app_dir).await
    }

    /// Update an existing app from a directory
    ///
    /// **Context**: Development workflow - update app with new version
    /// **We do**: Delegate to app_service::update_app_from_directory
    pub async fn update_app(&self, page_id: &str, app_dir: &Path) -> Result<()> {
        services::app_service::update_app_from_directory(self, page_id, app_dir).await
    }

    /// Import a page directory containing multiple apps
    ///
    /// **Context**: Development workflow - import page with multiple apps
    /// **Structure**:
    /// ```text
    /// page_dir/                     ← directory name = page name
    ///   ├── permit_template.json    ← page-level permit (data layers)
    ///   ├── shop-owner/             ← app 1
    ///   └── shop-customer/          ← app 2
    /// ```
    /// **We do**: Delegate to app_service::import_page_from_directory
    pub async fn import_page(&self, space_id: &str, page_dir: &Path) -> Result<Page> {
        services::app_service::import_page_from_directory(self, space_id, page_dir).await
    }

    /// Reload a page from directory (sync from filesystem)
    ///
    /// **Context**: Development workflow - reload page after editing files
    /// **Behavior**:
    /// - Updates existing apps (files changed on disk)
    /// - Adds new apps (new subdirectories with manifest.json)
    /// **We do**: Delegate to app_service::reload_page_from_directory
    pub async fn reload_page(&self, page_id: &str, page_dir: &Path) -> Result<services::app_service::PageReloadResult> {
        services::app_service::reload_page_from_directory(self, page_id, page_dir).await
    }

    pub async fn prepare_space_for_publish(
        &self,
        space_id: &str,
        node_public_key: &str,
    ) -> Result<(SpaceMeta, String)> {
        let identity = self.get_identity().await?;
        let owner_did = identity.did().to_string();
        publish_service::prepare_space_for_publish(
            &self.store,
            space_id,
            &owner_did,
            node_public_key,
            &identity.secret_signing_key(),
        ).await
    }

    /// Prepare page for publishing to node
    ///
    /// **Context**: Owner wants to publish page to their node
    /// **We do**:
    ///   1. Get page and owner's permit
    ///   2. Delegate permit to node
    ///   3. Filter out local_only layers
    ///   4. Encrypt layers for transit using ephemeral ECDH
    /// **We return**: PreparedPage with transit-encrypted layers
    pub async fn prepare_page_for_publish(
        &self,
        page_id: &str,
        node_public_key: &str,
        node_encryption_key: &[u8; 32],
    ) -> Result<PreparedPage> {
        let identity = self.get_identity().await?;
        let owner_did = identity.did().to_string();
        publish_service::prepare_page_for_publish(
            &self.store,
            page_id,
            &owner_did,
            node_public_key,
            node_encryption_key,
            &identity.secret_signing_key(),
            &identity.secret_encryption_key(),
        ).await
    }

    /// Store a page received via publish (Node/Viewer mode)
    ///
    /// **Context**: Node received PublishPage from owner, or Viewer received PageData from node
    /// **We do**: Decrypt transit layers, re-encrypt with new AES key, store
    /// **source_node_did**: For viewer side, tracks which node sent this page (for reconnection)
    /// **sender_did**: DID of the peer sending this page (for state vector storage)
    /// **sender_device_id**: Device ID of the sender (for state vector storage)
    pub async fn store_published_page(
        &self,
        page_meta: PageMeta,
        permit: &str,
        ephemeral_public: &[u8; 32],
        transit_layers: Vec<(String, Vec<u8>)>,
        source_node_did: Option<&str>,
        sender_did: &str,
        sender_device_id: &str,
    ) -> Result<Page> {
        let identity = self.get_identity().await?;
        publish_service::store_published_page(
            &self.store,
            page_meta,
            permit,
            ephemeral_public,
            transit_layers,
            &identity.secret_encryption_key(),
            &identity.public_encryption_key(),
            source_node_did,
            sender_did,
            sender_device_id,
        )
    }

    /// Mark a page as published to a specific node
    pub fn mark_page_published(&self, page_id: &str, node_id: &str) -> Result<()> {
        publish_service::mark_page_published(&self.store, page_id, node_id)
    }

    /// Prepare page for viewer (Node mode)
    ///
    /// **Context**: Node is responding to viewer's SpaceRequest
    /// **We do**:
    ///   1. Get page and node's permit
    ///   2. Delegate permit to viewer using "viewer" template
    ///   3. Filter out local_only layers
    ///   4. Encrypt layers for transit using ephemeral ECDH
    /// **We return**: PreparedPage with transit-encrypted layers for viewer
    pub async fn prepare_page_for_viewer(
        &self,
        page_id: &str,
        viewer_public_key: &str,
        viewer_encryption_key: &[u8; 32],
    ) -> Result<PreparedPage> {
        let identity = self.get_identity().await?;
        publish_service::prepare_page_for_viewer(
            &self.store,
            page_id,
            viewer_public_key,
            viewer_encryption_key,
            &identity.secret_signing_key(),
            &identity.secret_encryption_key(),
        ).await
    }

    /// Delegate space permit to viewer (Node mode)
    ///
    /// **Context**: Node delegates its space permit to viewer
    /// **We do**: Use "viewer" template from space permit delegation
    /// **We return**: Delegated viewer permit
    pub async fn delegate_space_to_viewer(&self, space_id: &str, viewer_pubkey: &str) -> Result<String> {
        let signing_key = self.signing_key().await?;

        // Get the space with permit data
        let space_data = self.store.get_space(space_id)?
            .ok_or_else(|| ButlerError::NotFound(format!("Space {} not found", space_id)))?;

        let space_permit = space_data.permit
            .ok_or_else(|| ButlerError::PermitError("Space has no permit".to_string()))?;

        // Delegate to viewer using the "viewer" template from SPACE_TEMPLATE.delegation.viewer
        let (delegated, _cid) = gurkha::delegate_space(&signing_key, &space_permit, "viewer", viewer_pubkey)
            .await
            .map_err(|e| ButlerError::PermitError(format!("Failed to delegate to viewer: {:?}", e)))?;

        Ok(delegated)
    }

    // ==================== Node Operations ====================

    /// Add a sovereign node from a connection string (owner connection)
    pub fn add_sovereign_node(&self, connection_string: &str) -> std::result::Result<SovereignNode, String> {
        node_service::add_sovereign_node(&self.store, connection_string, ConnectionType::Owner)
    }

    /// Parse a connection string without storing (for viewer connections)
    ///
    /// Returns the parsed connection details for immediate use.
    /// Viewer connections are not persisted - they connect, get data, and that's it.
    pub fn parse_connection_string(&self, connection_string: &str) -> std::result::Result<ConnectionString, String> {
        ConnectionString::parse(connection_string)
    }

    /// Generate a connection string for this node (Node mode)
    ///
    /// **Context**: Node generates connection string for owner to scan/enter
    /// **We return**: Base64-encoded JSON containing keys, permit, etc.
    /// Note: node_id is derived from device_public_key when parsing
    ///
    /// # Arguments
    /// * `relay_url` - Optional relay URL for connection
    pub async fn generate_connection_string(
        &self,
        relay_url: Option<&str>,
    ) -> Result<String> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let identity = self.get_identity().await?;
        let user_info = self.user_info().await?;

        // Issue one-time permit for owner connection
        // Relationship is "node_owner" = node issuing permit TO owner
        let (permit, _pub_key) = self.issue_one_time_permit("node_owner").await?;

        // Encode keys as base64
        let user_pub_key = STANDARD.encode(identity.public_signing_key());
        let device_pub_key = STANDARD.encode(identity.public_device_key());
        let encryption_pub_key = STANDARD.encode(identity.public_encryption_key());

        // Build connection string JSON (matches ConnectionString struct)
        // node_id is derived from device_public_key when parsing
        let connection_details = serde_json::json!({
            "node_public_key": user_pub_key,
            "node_encryption_key": encryption_pub_key,
            "device_public_key": device_pub_key,
            "name": user_info.username,
            "permit": permit,
            "relay": relay_url,
        });

        // Base64 encode the JSON
        let connection_json = connection_details.to_string();
        let encoded = STANDARD.encode(connection_json.as_bytes());

        Ok(encoded)
    }

    /// Generate a viewer connection string for a space (Node mode)
    ///
    /// **Context**: Node generates shareable link for viewers
    /// **We return**: Base64-encoded JSON with keys and viewer permit
    /// Note: node_id is derived from device_public_key when parsing
    ///
    /// # Arguments
    /// * `space_id` - The space to generate viewer permit for
    /// * `relay_url` - Optional relay URL for connection
    pub async fn generate_viewer_connection_string(
        &self,
        space_id: &str,
        relay_url: Option<&str>,
    ) -> Result<String> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let identity = self.get_identity().await?;
        let user_info = self.user_info().await?;

        // Generate viewer permit for this space (aud:* wildcard)
        let (permit, _cid) = self.issue_space_viewer_permit(space_id).await?;

        // Encode keys as base64
        let user_pub_key = STANDARD.encode(identity.public_signing_key());
        let device_pub_key = STANDARD.encode(identity.public_device_key());
        let encryption_pub_key = STANDARD.encode(identity.public_encryption_key());

        // Build connection string JSON
        // node_id is derived from device_public_key when parsing (SovereignNode::from_connection_string)
        let connection_details = serde_json::json!({
            "node_public_key": user_pub_key,
            "node_encryption_key": encryption_pub_key,
            "device_public_key": device_pub_key,
            "name": user_info.username,
            "permit": permit,
            "relay": relay_url,
        });

        // Base64 encode the JSON
        let connection_json = connection_details.to_string();
        let encoded = STANDARD.encode(connection_json.as_bytes());

        Ok(encoded)
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

    /// Update relay URL for a sovereign node
    ///
    /// **Context**: After setup_test_dbs, stored nodes may have invalid relay URLs.
    /// This updates the relay URL with the real one from running kunki.
    pub fn update_node_relay(&self, node_id: &str, relay_url: Option<String>) -> Result<bool> {
        self.store.update_sovereign_node_relay(node_id, relay_url)
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

    /// Store the permit for owner<->node relationship (Node mode)
    pub fn set_owner_permit(&self, permit: String) -> Result<bool> {
        node_service::set_owner_permit(&self.store, permit)
    }

    /// Update owner's last connected timestamp
    pub fn update_owner_last_connected(&self) -> Result<bool> {
        node_service::update_owner_last_connected(&self.store)
    }

    // ==================== Viewer Consent Operations (Node-side) ====================

    /// Store viewer-issued consent permits.
    ///
    /// **Context**: Node receives SyncConsentGrant from viewer.
    /// The permits are viewer-issued, expressing consent for receiving sync updates.
    ///
    /// # Arguments
    /// * `viewer_did` - The viewer's DID
    /// * `space_id` - The space ID
    /// * `space_consent_permit` - Viewer-issued space consent permit
    /// * `page_consent_permits` - Viewer-issued page consent permits (page_id, permit)
    pub async fn store_viewer_consent(
        &self,
        viewer_did: &str,
        space_id: &str,
        space_consent_permit: &str,
        page_consent_permits: &[(String, String)],
    ) -> Result<()> {
        // Store space consent (only if not empty)
        if !space_consent_permit.is_empty() {
            self.store.put_viewer_space_consent(viewer_did, space_id, space_consent_permit)?;
        }

        // Store page consents
        for (page_id, permit) in page_consent_permits {
            self.store.put_viewer_page_consent(viewer_did, page_id, permit)?;
        }

        Ok(())
    }

    /// Get viewer's space consent permit.
    ///
    /// **Context**: Node needs consent permit to send sync updates.
    pub fn get_viewer_space_consent(&self, viewer_did: &str, space_id: &str) -> Result<Option<String>> {
        self.store.get_viewer_space_consent(viewer_did, space_id)
    }

    /// Get viewer's page consent permit.
    ///
    /// **Context**: Node needs consent permit to send layer updates.
    pub fn get_viewer_page_consent(&self, viewer_did: &str, page_id: &str) -> Result<Option<String>> {
        self.store.get_viewer_page_consent(viewer_did, page_id)
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

    /// Store a node as contact (viewer side - for tracking nodes viewer is subscribed to)
    pub fn add_node_contact(
        &self,
        did: &str,
        encryption_key: &str,
        name: &str,
        node_id: &str,
        permit: &str,
    ) -> Result<ContactData> {
        contact_service::upsert_node_contact(&self.store, did, encryption_key, name, node_id, permit)
    }

    /// List node contacts only (viewer side)
    pub fn list_node_contacts(&self) -> Result<Vec<ContactData>> {
        contact_service::list_node_contacts(&self.store)
    }

    /// Resolve user_did to device connection info.
    ///
    /// **Context**: Called by Coordinator when handling EnsureSync.
    /// **We query**: Contact info for user, then connection permit.
    /// **We return**: ConnectionDeviceInfo with node_id and permit.
    pub fn resolve_device_for_user(&self, user_did: &str) -> Result<Option<ConnectionDeviceInfo>> {
        // 1. Try contacts first (for viewer → node connections)
        if let Some(contact) = contact_service::get_contact(&self.store, user_did)? {
            // Node type contacts have node_id and permit directly
            if let (Some(node_id), Some(permit)) = (contact.node_id, contact.permit) {
                return Ok(Some(ConnectionDeviceInfo { node_id, permit }));
            }
        }

        // 2. Try sovereign nodes (for owner → node connections)
        // Sovereign nodes are stored by node_id, but we need to find by DID
        for node in self.list_sovereign_nodes()? {
            if node.did == user_did {
                if let Some(permit) = node.permit {
                    return Ok(Some(ConnectionDeviceInfo {
                        node_id: node.node_id,
                        permit,
                    }));
                }
            }
        }

        Ok(None)
    }

    // Note: add_share_to_contact, get_users_for_page, get_contacts_for_page removed
    // Access tracking moved to SPACE_SUBSCRIPTIONS table

    // ==================== Permit CID Operations (for revocation tracking) ====================

    /// Store a permit CID for a specific page and user.
    ///
    /// **Context**: Called when issuing a permit to track it for potential revocation.
    pub fn put_permit_cid(&self, page_id: &str, user_did: &str, cid: &str) -> Result<()> {
        self.store.put_permit_cid(page_id, user_did, cid)
    }

    /// Get a permit CID for a specific page and user.
    ///
    /// **Context**: Used to check if a permit has been issued and get its CID.
    pub fn get_permit_cid(&self, page_id: &str, user_did: &str) -> Result<Option<String>> {
        self.store.get_permit_cid(page_id, user_did)
    }

    /// Delete a permit CID for a specific page and user.
    ///
    /// **Context**: Called when revoking access - marks the permit as revoked.
    pub fn delete_permit_cid(&self, page_id: &str, user_did: &str) -> Result<bool> {
        self.store.delete_permit_cid(page_id, user_did)
    }

    /// List all permit CIDs for a given page.
    ///
    /// **Context**: Used to enumerate all issued permits for a page.
    /// Returns list of (user_did, cid) tuples.
    pub fn list_permit_cids_for_page(&self, page_id: &str) -> Result<Vec<(String, String)>> {
        self.store.list_permit_cids_for_page(page_id)
    }

    /// Delete all permit CIDs for a page.
    ///
    /// **Context**: Called when a page is deleted.
    pub fn delete_all_permit_cids_for_page(&self, page_id: &str) -> Result<usize> {
        self.store.delete_all_permit_cids_for_page(page_id)
    }

    /// Check if a permit CID exists for a page and user.
    ///
    /// **Context**: Quick check for whether a permit has been issued.
    pub fn has_permit_cid(&self, page_id: &str, user_did: &str) -> Result<bool> {
        self.store.has_permit_cid(page_id, user_did)
    }

    // ==================== User Page Permits (for sync authorization) ====================

    /// Store a user's permit for a page (Node mode - for sync authorization).
    ///
    /// **Context**: Node receives owner's/viewer's permit during publish.
    /// This permit is used for sync authorization (layer permissions).
    /// Replaces DID whitelist with permit-based authorization.
    pub fn store_user_page_permit(&self, page_id: &str, user_did: &str, permit: &str) -> Result<()> {
        self.store.put_user_page_permit(page_id, user_did, permit)
    }

    /// Get a user's permit for a page (Node mode - for sync authorization).
    ///
    /// **Context**: Node needs permit to authorize sync operations.
    pub fn get_user_page_permit(&self, page_id: &str, user_did: &str) -> Result<Option<String>> {
        self.store.get_user_page_permit(page_id, user_did)
    }

    // ==================== Scribe Operations ====================

    /// Open a page and get its Scribe actor
    ///
    /// **Context**: UI or PeerActor wants to interact with a page
    /// **We do**: Spawn Scribe if not exists, return ActorRef
    /// **We return**: ActorRef<ScribeMessage> for sending messages
    ///
    /// Actor naming: `scribe-{user_did_hash}-{page_id}` for deterministic querying
    pub async fn open_page(&self, page_id: &str) -> Result<ActorRef<ScribeMessage>> {
        // Quick check if already open (no lock needed for read)
        {
            let mut scribes = self.scribes.write().await;
            if let Some(entry) = scribes.get_mut(page_id) {
                entry.last_accessed = Instant::now();
                return Ok(entry.actor.clone());
            }
        }

        // Get or create per-page lock to serialize concurrent open_page calls
        let page_lock = {
            let mut locks = self.page_locks.write().await;
            locks.entry(page_id.to_string())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };

        // Acquire per-page lock - this serializes concurrent calls for the same page
        let _guard = page_lock.lock().await;

        // Re-check after acquiring lock (another call may have created it)
        {
            let mut scribes = self.scribes.write().await;
            if let Some(entry) = scribes.get_mut(page_id) {
                entry.last_accessed = Instant::now();
                return Ok(entry.actor.clone());
            }
        }

        // Get user DID for actor naming (use hash to keep name shorter)
        let user_info = self.user_info().await?;
        let user_did_hash = &user_info.did[user_info.did.len().saturating_sub(8)..];
        let actor_name = format!("scribe-{}-{}", user_did_hash, page_id);

        // Check if an old actor with this name is still in the registry
        // (happens when evicted actor hasn't fully shut down yet).
        // If so, stop it and wait for registry cleanup before creating new one.
        if let Some(old_actor) = ractor::registry::where_is(actor_name.clone()) {
            log::info!("Found stale Scribe actor in registry: {} - stopping it", actor_name);
            old_actor.stop(Some("replaced by new open_page call".to_string()));

            // Wait for the old actor to fully unregister (up to 500ms)
            for _ in 0..50 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                if ractor::registry::where_is(actor_name.clone()).is_none() {
                    break;
                }
            }
        }

        // Evict LRU if at capacity
        self.maybe_evict_lru().await;

        // Load page data and decrypt layers (also get AES key for save_layer)
        let page_id_str = page_id.to_string();
        let (decrypted, aes_key) = self.get_decrypted_page(page_id).await?;

        log::info!(
            "open_page: page_id={} loaded {} docs from DB: {:?}",
            page_id,
            decrypted.docs.len(),
            decrypted.docs.iter().map(|(n, b)| (n.as_str(), b.len())).collect::<Vec<_>>()
        );

        // Convert DecryptedPage docs to Layer type
        let mut layers = HashMap::new();
        for (name, doc_bytes) in decrypted.docs {
            // doc_bytes is the raw decrypted LoroDoc snapshot
            let layer = if doc_bytes.is_empty() {
                log::info!("open_page: layer {} is empty, creating new Layer", name);
                Layer::new()
            } else {
                // Create layer from snapshot bytes
                match Layer::from_snapshot(&doc_bytes) {
                    Ok(l) => {
                        let sv = l.version_vector();
                        log::info!(
                            "open_page: layer {} loaded from {} bytes, state_vector={} bytes {:02x?}",
                            name, doc_bytes.len(), sv.len(), &sv[..sv.len().min(32)]
                        );
                        l
                    }
                    Err(e) => {
                        log::error!("open_page: layer {} failed to load from snapshot: {} - creating new Layer", name, e);
                        Layer::new()
                    }
                }
            };
            layers.insert(name, layer);
        }

        // Create injected functions scoped to this page
        let store_for_save = self.store.clone();
        let page_id_for_save = page_id_str.clone();
        let save_layer: SaveLayerFn = Arc::new(move |layer_name: &str, data: &[u8]| {
            // Encrypt layer with the page's AES key
            let encrypted = herald::encrypt_symmetric(&aes_key, data)
                .map_err(|e| ButlerError::Encryption(e.to_string()))?;

            // Save to store
            store_for_save.put_layer(&page_id_for_save, layer_name, &encrypted)?;

            log::info!("Saved layer {} for page {} ({} bytes)", layer_name, page_id_for_save, encrypted.len());
            Ok(())
        });

        let store_for_load_vec = self.store.clone();
        let page_id_for_load = page_id_str.clone();
        let load_peer_vector: LoadPeerVectorFn = Arc::new(move |user_did: &str, device_id: &str| {
            log::debug!("Loading peer vector for {}/{} on page {}", user_did, device_id, page_id_for_load);
            store_for_load_vec.get_peer_vectors(&page_id_for_load, user_did, device_id)
        });

        let store_for_save_vec = self.store.clone();
        let page_id_for_save_vec = page_id_str.clone();
        let save_peer_vector: SavePeerVectorFn = Arc::new(move |user_did: &str, device_id: &str, vectors: &HashMap<String, Vec<u8>>| {
            log::debug!("Saving peer vector for {}/{} on page {} ({} layers)", user_did, device_id, page_id_for_save_vec, vectors.len());
            store_for_save_vec.put_peer_vectors(&page_id_for_save_vec, user_did, device_id, vectors)
        });

        // Resolve SyncConfig from page permit
        //
        // **Context**: Parse permit to extract relationship, then resolve sync_target:
        // - Owner: sync_target = first connected sovereign node
        // - Node: sync_target = None (broadcast only)
        // - Viewer: sync_target = source_node_id
        let sync_config = self.resolve_sync_config(page_id).await;

        // Create callback for listing authorized users (node mode)
        let store_for_list_users = self.store.clone();
        let page_id_for_list_users = page_id_str.clone();
        let list_authorized_users: ListAuthorizedUsersFn = Arc::new(move |_page_id: &str| {
            // Use the page_id from closure, not the parameter (it's the same anyway)
            store_for_list_users.list_authorized_users_for_page(&page_id_for_list_users)
                .unwrap_or_default()
        });

        // Create callback for loading user's permit (node mode - permit-based sync auth)
        let store_for_load_permit = self.store.clone();
        let load_user_permit: LoadUserPermitFn = Arc::new(move |page_id: &str, user_did: &str| {
            store_for_load_permit.get_user_page_permit(page_id, user_did)
                .ok()
                .flatten()
        });

        // Get sync_event_tx (clone the inner Option's Sender)
        let sync_event_tx = self.sync_event_tx.read().await.clone();

        // Load validation.lua and init.lua from app:shared layer (if exists)
        // Debug: log all layers and their sizes
        log::info!(
            "open_page: checking for app:shared in {} layers: {:?}",
            layers.len(),
            layers.keys().collect::<Vec<_>>()
        );

        let (validation_code, init_code) = if let Some(shared_layer) = layers.get("app:shared") {
            let files = shared_layer.get_all_files();
            log::info!(
                "open_page: app:shared layer has {} files: {:?}",
                files.len(),
                files.keys().collect::<Vec<_>>()
            );
            (
                files.get("validation.lua").cloned(),
                files.get("init.lua").cloned(),
            )
        } else {
            log::info!("open_page: app:shared layer not found for page {}", page_id_str);
            (None, None)
        };

        if validation_code.is_some() {
            log::info!("Found validation.lua in app:shared layer for page {}", page_id_str);
        }
        if init_code.is_some() {
            log::info!("Found init.lua ({} bytes) in app:shared layer for page {}",
                init_code.as_ref().map(|c| c.len()).unwrap_or(0), page_id_str);
        }

        // Get our permit for this page (from page metadata)
        let our_permit = self.get_page(page_id).ok()
            .flatten()
            .and_then(|p| p.permit);

        // Get our DID (from identity)
        let our_did = self.get_identity().await
            .map(|id| id.did().to_string())
            .unwrap_or_default();

        // Determine if this Scribe runs on a node (enables derivation engine)
        // Node mode uses SyncMode::Broadcast, owner/customer use SyncMode::ToSource
        let is_node = sync_config.as_ref()
            .map(|c| matches!(c.mode, scribe::state::SyncMode::Broadcast))
            .unwrap_or(false);

        // Spawn Scribe actor
        let args = ScribeArgs {
            page_id: page_id_str.clone(),
            layers,
            save_layer,
            load_peer_vector,
            save_peer_vector,
            sync_config,
            sync_event_tx,
            list_authorized_users: Some(list_authorized_users),
            load_user_permit: Some(load_user_permit),
            validation_code,
            init_code,
            our_permit,
            our_did,
            is_node,
            // Note: ephemeral_broadcast_tx removed - now uses SubscriberInfo.ephemeral_tx
        };

        let (actor, _handle) = ractor::Actor::spawn(
            Some(actor_name),
            Scribe::new(),
            args,
        ).await.map_err(|e| ButlerError::Storage(format!("Failed to spawn Scribe: {:?}", e)))?;

        // Store in map
        {
            let mut scribes = self.scribes.write().await;
            scribes.insert(page_id_str, ScribeEntry {
                actor: actor.clone(),
                last_accessed: Instant::now(),
            });
        }

        Ok(actor)
    }

    /// Close a page and stop its Scribe actor
    ///
    /// **Context**: Page no longer needed, release resources
    /// **We do**: Flush and stop the Scribe actor
    pub async fn close_page(&self, page_id: &str) -> Result<()> {
        let entry = {
            let mut scribes = self.scribes.write().await;
            scribes.remove(page_id)
        };

        if let Some(entry) = entry {
            // Send shutdown message - Scribe will flush before stopping
            let _ = entry.actor.cast(ScribeMessage::Shutdown);
        }

        Ok(())
    }

    /// Get active Scribe count
    pub async fn active_scribe_count(&self) -> usize {
        self.scribes.read().await.len()
    }

    /// List active Scribes with their permits for peer subscription
    ///
    /// **Context**: After handshake, PeerActor needs to subscribe to active Scribes
    /// **We return**: (page_id, permit) for each page with an active Scribe and valid permit
    ///
    /// Note: peer_did parameter reserved for future access control filtering
    pub async fn list_active_scribes_for_peer(&self, _peer_did: &str) -> Result<Vec<(String, String)>> {
        let scribes = self.scribes.read().await;
        let mut result = Vec::new();

        for page_id in scribes.keys() {
            // Get page data with permit
            if let Ok(Some(page_data)) = self.get_page(page_id) {
                if let Some(permit) = page_data.permit {
                    result.push((page_id.clone(), permit));
                }
            }
        }

        Ok(result)
    }

    /// Evict least recently used Scribe if at capacity
    async fn maybe_evict_lru(&self) {
        let mut scribes = self.scribes.write().await;

        if scribes.len() < self.max_open_pages {
            return;
        }

        // Find LRU entry
        let lru_page_id = scribes
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(id, _)| id.clone());

        if let Some(page_id) = lru_page_id {
            if let Some(entry) = scribes.remove(&page_id) {
                log::info!("Evicting LRU Scribe for page {}", page_id);
                let _ = entry.actor.cast(ScribeMessage::Shutdown);
            }
        }
    }

    /// Resolve sync configuration from page permit
    ///
    /// **Context**: Parse permit to determine role-based sync behavior
    /// **Returns**: SyncConfig with mode and resolved sync_target
    async fn resolve_sync_config(&self, page_id: &str) -> Option<SyncConfig> {
        use crate::scribe::state::SyncMode;

        // Get page data with permit
        let page_data = self.get_page(page_id).ok()??;
        let permit_str = page_data.permit.as_ref()?;

        // Parse permit to check capabilities
        let parsed = gurkha::Permit::from_token(permit_str).ok()?;

        // Derive sync mode from peer_capabilities
        // If peer has relay capability (node), use Broadcast mode
        // Otherwise use ToSource mode (sync to our node)
        let peer_caps = parsed.peer_capabilities();
        let mode = if peer_caps.relay {
            // Node mode: broadcast to all authorized peers
            SyncMode::Broadcast
        } else {
            // User mode: sync to source node
            SyncMode::ToSource
        };

        // Resolve sync_target based on mode
        let sync_target = match mode {
            SyncMode::ToSource => {
                // User syncs to source node (where they got the space from)
                // Try source_node first, then sovereign_node
                self.get_source_node_for_page(page_id).ok().flatten()
                    .or_else(|| self.get_sovereign_node_for_sync().ok().flatten())
            }
            SyncMode::Broadcast => {
                // Node broadcasts only - no single outbound sync target
                None
            }
        };

        log::info!(
            "Resolved SyncConfig for page {}: mode={:?}, target={:?}",
            page_id, mode, sync_target
        );

        Some(SyncConfig {
            mode,
            sync_target,
        })
    }

    // ==================== Sync State Vector Operations ====================

    /// Store a single peer state vector for a specific layer.
    ///
    /// **Context**: Called after 3-step sync completes to track peer's state.
    /// **We store**: The peer's state vector so we can compute diffs later.
    pub async fn store_peer_state_vector(
        &self,
        peer_did: &str,
        peer_device_id: &str,
        page_id: &str,
        layer_name: &str,
        state_vector: &[u8],
    ) -> Result<()> {
        self.store.put_peer_vector_for_layer(
            page_id,
            peer_did,
            peer_device_id,
            layer_name,
            state_vector.to_vec(),
        )
    }

    /// Store peer's state vectors after successful content transfer.
    ///
    /// **Context**: Called after publishing/receiving pages to track what the peer has.
    /// **We store**: The state vector for each layer so incremental sync works later.
    pub async fn store_peer_vectors_from_page(
        &self,
        page_id: &str,
        peer_did: &str,
        peer_device_id: &str,
    ) -> Result<()> {
        // Get decrypted page to access layer data
        let (decrypted_page, _aes_key) = self.get_decrypted_page(page_id).await?;

        // For each layer, extract state vector and store it
        for (layer_name, layer_data) in decrypted_page.docs {
            let temp_doc = loro::LoroDoc::new();
            if temp_doc.import(&layer_data).is_ok() {
                let vector = temp_doc.oplog_vv().encode();
                let _ = self.store.put_peer_vector_for_layer(
                    page_id,
                    peer_did,
                    peer_device_id,
                    &layer_name,
                    vector,
                );
            }
        }

        Ok(())
    }

    // ==================== Internal Accessors (rarely needed) ====================

    pub fn store(&self) -> &Arc<RedbStore> {
        &self.store
    }
}
