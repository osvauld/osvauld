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
//! ## API Organization
//! Butler uses sub-object APIs for grouped operations:
//! - `butler.spaces()` - Space CRUD and publishing
//! - `butler.pages()` - Page CRUD and lifecycle
//! - `butler.apps()` - App layer operations
//! - `butler.files()` - File storage operations
//! - `butler.publish()` - Publish operations
//! - `butler.assets()` - Asset upload
//! - `butler.nodes()` - Node and owner operations
//! - `butler.contacts()` - Contact operations
//! - `butler.permits()` - Permit issuance and CID tracking

pub mod api;
pub mod error;
pub mod merge;
pub mod models;
pub(crate) mod scribe_manager;
pub mod scribe_storage;
pub mod services;
pub mod storage;
pub mod sync;

// Refresh module (filesystem access stays in butler)
pub mod refresh;

// Test infrastructure (only compiled in test mode)
#[cfg(test)]
pub mod test_fixtures;
#[cfg(test)]
pub mod test_strategies;

pub use api::{
    AppsApi, AssetsApi, ContactsApi, FilesApi, NodesApi, PagesApi, PermitsApi, PublishApi,
    SpacesApi,
};
pub use error::{ButlerError, Result};
pub use models::*;
pub use storage::{AssetStore, CachedLayer, LayerCache, LayerCacheStats, RedbStore};
// Auth functions are standalone (don't need Butler instance)
pub use services::{
    change_passphrase, get_identity_data, is_signed_up, login, recover, signup, SignupResult,
};
// Scribe actor exports - re-exported from scribe crate
use herald::Identity;
use ractor::ActorRef;
pub use scribe::{
    BroadcastPayload, DynamicLayerMeta, EphemeralBroadcast, EphemeralOutbound, JsonOp,
    LayerStorage, LayerStorageRef, ListOp, LoroDelta, NullPermitIssuer, PageUpdate, PageUpdateTx,
    PeerConnection, PeerResolver, PeerResolverRef, PeerVectorStorage, PeerVectorStorageRef,
    PermitIssuer, PermitIssuerRef, Scribe, ScribeArgs, ScribeError, ScribeMessage, SyncConfig,
    SyncEvent, SyncMode, ValidationHandle, ValidationRequest,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::instrument;

use scribe_manager::ScribeManager;

/// Butler - Main entry point for storage and services
///
/// Owns identity state and storage. All operations go through Butler,
/// which injects dependencies into stateless service functions.
pub struct Butler {
    store: Arc<RedbStore>,
    identity: Arc<RwLock<Option<Identity>>>,
    /// Filesystem storage for encrypted assets (images, PDFs, etc.)
    asset_store: Arc<AssetStore>,
    /// Scribe actor lifecycle manager
    scribe_manager: ScribeManager,
    /// Channel to emit sync events (EnsureSync) from Scribes to Coordinator
    sync_event_tx: Option<tokio::sync::mpsc::Sender<SyncEvent>>,
    /// Channel to notify when a page is opened (for kunki node runtime)
    page_opened_tx: RwLock<Option<tokio::sync::mpsc::Sender<String>>>,
    /// Validation handle (for kunki node mode validation)
    validation_handle: RwLock<Option<ValidationHandle>>,
    /// Broadcast channel for capture system (pre-serialized JSON lines)
    /// Set via set_capture_tx(), threaded into ScribeArgs for event capture
    capture_tx: RwLock<Option<tokio::sync::broadcast::Sender<String>>>,
}

impl Butler {
    /// Create a new Butler instance
    ///
    /// **sync_event_tx**: Channel for Scribe-to-Coordinator communication (EnsureSync events).
    /// Pass None if not using Courier/sync (e.g., standalone mode).
    pub fn new(
        store: Arc<RedbStore>,
        _layer_cache: Arc<RwLock<LayerCache>>,
        asset_store: Arc<AssetStore>,
        sync_event_tx: Option<tokio::sync::mpsc::Sender<SyncEvent>>,
    ) -> Self {
        Self {
            store,
            identity: Arc::new(RwLock::new(None)),
            asset_store,
            scribe_manager: ScribeManager::new(),
            sync_event_tx,
            page_opened_tx: RwLock::new(None),
            validation_handle: RwLock::new(None),
            capture_tx: RwLock::new(None),
        }
    }

    /// Create a new Butler instance with custom max open pages
    pub fn with_max_pages(
        store: Arc<RedbStore>,
        _layer_cache: Arc<RwLock<LayerCache>>,
        asset_store: Arc<AssetStore>,
        max_open_pages: usize,
        sync_event_tx: Option<tokio::sync::mpsc::Sender<SyncEvent>>,
    ) -> Self {
        Self {
            store,
            identity: Arc::new(RwLock::new(None)),
            asset_store,
            scribe_manager: ScribeManager::with_max_pages(max_open_pages),
            sync_event_tx,
            page_opened_tx: RwLock::new(None),
            validation_handle: RwLock::new(None),
            capture_tx: RwLock::new(None),
        }
    }

    /// Set page opened callback channel (used by kunki for node runtime)
    #[instrument(skip_all)]
    pub async fn set_page_opened_tx(&self, tx: tokio::sync::mpsc::Sender<String>) {
        *self.page_opened_tx.write().await = Some(tx);
    }

    /// Set capture broadcast channel (for event capture system)
    ///
    /// **Context**: Called by kunki/sthalam after creating the capture broadcast channel.
    /// Threaded into ScribeArgs so Scribe actors emit to the capture system.
    pub async fn set_capture_tx(&self, tx: tokio::sync::broadcast::Sender<String>) {
        *self.capture_tx.write().await = Some(tx);
    }

    /// Set validation handle (for kunki node mode)
    ///
    /// **Context**: kunki creates ValidationService and passes handle to Butler
    /// **Usage**: Butler passes handle to Scribe for remote validation
    #[instrument(skip_all)]
    pub async fn set_validation_handle(&self, handle: ValidationHandle) {
        *self.validation_handle.write().await = Some(handle);
    }

    /// Spaces API
    pub fn spaces(&self) -> api::SpacesApi<'_> {
        api::SpacesApi { butler: self }
    }

    /// Pages API
    pub fn pages(&self) -> api::PagesApi<'_> {
        api::PagesApi { butler: self }
    }

    /// Nodes API
    pub fn nodes(&self) -> api::NodesApi<'_> {
        api::NodesApi { butler: self }
    }

    /// Contacts API
    pub fn contacts(&self) -> api::ContactsApi<'_> {
        api::ContactsApi { butler: self }
    }

    /// Permits API
    pub fn permits(&self) -> api::PermitsApi<'_> {
        api::PermitsApi { butler: self }
    }

    /// Apps API
    pub fn apps(&self) -> api::AppsApi<'_> {
        api::AppsApi { butler: self }
    }

    /// Files API
    pub fn files(&self) -> api::FilesApi<'_> {
        api::FilesApi { butler: self }
    }

    /// Publish API
    pub fn publish(&self) -> api::PublishApi<'_> {
        api::PublishApi { butler: self }
    }

    /// Assets API
    pub fn assets(&self) -> api::AssetsApi<'_> {
        api::AssetsApi { butler: self }
    }

    /// Set identity after login
    #[instrument(skip_all)]
    pub async fn set_identity(&self, identity: Identity) {
        let mut guard = self.identity.write().await;
        *guard = Some(identity);
    }

    /// Get identity (errors if not logged in)
    #[instrument(skip_all)]
    pub async fn get_identity(&self) -> Result<Identity> {
        let guard = self.identity.read().await;
        guard.clone().ok_or_else(ButlerError::not_logged_in)
    }

    /// Clear identity on logout
    #[instrument(skip_all)]
    pub async fn clear_identity(&self) {
        let mut guard = self.identity.write().await;
        *guard = None;
    }

    /// Check if logged in
    #[instrument(skip_all)]
    pub async fn is_logged_in(&self) -> bool {
        self.identity.read().await.is_some()
    }

    /// Check if user has signed up
    #[instrument(skip_all)]
    pub fn is_signed_up(&self) -> Result<bool> {
        is_signed_up(&self.store)
    }

    /// Sign up a new user (blocking)
    #[instrument(skip(self, passphrase), fields(username = %username))]
    pub fn signup_sync(&self, username: &str, passphrase: &str) -> Result<SignupResult> {
        signup(&self.store, username, passphrase)
    }

    /// Login and return identity (blocking)
    #[instrument(skip_all)]
    pub fn login_sync(&self, passphrase: &str) -> Result<Identity> {
        login(&self.store, passphrase)
    }

    /// Get stored identity data
    #[instrument(skip_all)]
    pub fn identity_data(&self) -> Result<Option<IdentityData>> {
        get_identity_data(&self.store)
    }

    /// Get device key for Transport initialization
    #[instrument(skip_all)]
    pub async fn device_key(&self) -> Result<[u8; 32]> {
        let identity = self.get_identity().await?;
        Ok(identity.secret_device_key())
    }

    /// Get signing key for permit operations
    #[instrument(skip_all)]
    pub async fn signing_key(&self) -> Result<[u8; 32]> {
        let identity = self.get_identity().await?;
        Ok(identity.secret_signing_key())
    }

    /// Get encryption key for crypto operations
    #[instrument(skip_all)]
    pub async fn encryption_key(&self) -> Result<[u8; 32]> {
        let identity = self.get_identity().await?;
        Ok(identity.secret_encryption_key())
    }

    /// Get user info for UI display
    #[instrument(skip_all)]
    pub async fn user_info(&self) -> Result<UserInfo> {
        let identity = self.get_identity().await?;
        let identity_data =
            get_identity_data(&self.store)?.ok_or_else(ButlerError::not_logged_in)?;

        Ok(UserInfo {
            did: identity.did().to_string(),
            username: identity_data.username,
            public_key: identity.public_signing_key().to_vec(),
            encryption_key: identity.public_encryption_key().to_vec(),
        })
    }

    /// Build app context for launching a renderer
    ///
    /// **Context**: Extracts user identity and role from Butler state for a given page.
    /// **Used by**: renderer_slint and renderer_raylib before creating LuaRuntime/GameLoop.
    #[instrument(skip(self), fields(page_id = %page_id))]
    pub fn app_context(&self, page_id: &str) -> AppContext {
        let identity_data = get_identity_data(&self.store).ok().flatten();

        let user_did = identity_data
            .as_ref()
            .map(|data| data.did.clone())
            .unwrap_or_else(|| {
                let prefix = if page_id.len() >= 8 {
                    &page_id[..8]
                } else {
                    page_id
                };
                format!("did:key:unknown-{}", prefix)
            });

        let user_name = identity_data
            .as_ref()
            .map(|data| data.username.clone())
            .unwrap_or_else(|| "Player".to_string());

        let user_role = self
            .pages()
            .get(page_id)
            .ok()
            .flatten()
            .and_then(|page| page.get_permit().cloned())
            .and_then(|permit| {
                gurkha::PolicyPermit::from_token(&permit)
                    .ok()
                    .map(|p| p.token_type().unwrap_or("owner").to_string())
            })
            .unwrap_or_else(|| "owner".to_string());

        AppContext {
            user_did,
            user_name,
            user_role,
        }
    }

    /// Issue one-time permit for node connection
    #[instrument(skip(self), fields(role = %role))]
    pub async fn issue_one_time_permit(&self, role: &str) -> Result<(String, String)> {
        let signing_key = self.signing_key().await?;
        let (permit, _cid) = gurkha::issue_one_time(&signing_key, role)
            .await
            .map_err(|e| ButlerError::permit_error(format!("{:?}", e)))?;
        let pub_key = gurkha::get_public_key(&signing_key);
        Ok((permit, pub_key))
    }

    /// Issue space viewer permit
    #[instrument(skip(self), fields(space_id = %space_id))]
    pub async fn issue_space_viewer_permit(&self, space_id: &str) -> Result<(String, String)> {
        let signing_key = self.signing_key().await?;
        let (permit, _cid) = gurkha::issue_space_viewer_auth(&signing_key, space_id)
            .await
            .map_err(|e| ButlerError::permit_error(format!("{:?}", e)))?;
        let pub_key = gurkha::get_public_key(&signing_key);
        Ok((permit, pub_key))
    }

    /// Issue peer connection permit (for long-lived owner/node connections)
    #[instrument(skip(self), fields(relationship = %relationship))]
    pub async fn issue_peer_connection_permit(
        &self,
        peer_pubkey: &str,
        relationship: &str,
    ) -> Result<(String, String)> {
        let signing_key = self.signing_key().await?;
        let permit = gurkha::issue_peer_connection(&signing_key, peer_pubkey, relationship)
            .await
            .map_err(|e| ButlerError::permit_error(format!("{:?}", e)))?;
        let pub_key = gurkha::get_public_key(&signing_key);
        Ok((permit, pub_key))
    }

    /// Open a page and get its Scribe actor (spawns if needed)
    ///
    /// **Context**: Called when a page needs to be accessed for sync or queries
    /// **Notifies**: page_opened_tx channel for kunki to start node runtime
    #[instrument(skip(self), fields(page_id = %page_id))]
    pub async fn open_page(&self, page_id: &str) -> Result<ActorRef<ScribeMessage>> {
        // Quick check if already open
        if let Some(actor) = self.scribe_manager.get(page_id).await {
            return Ok(actor);
        }

        // Get per-page lock to serialize concurrent open_page calls
        let page_lock = self.scribe_manager.get_page_lock(page_id).await;
        let _guard = page_lock.lock().await;

        // Re-check after acquiring lock
        if let Some(actor) = self.scribe_manager.get(page_id).await {
            return Ok(actor);
        }

        // Get user DID for actor naming
        let user_info = self.user_info().await?;
        let user_did_hash = &user_info.did[user_info.did.len().saturating_sub(8)..];
        let actor_name = format!("scribe-{}-{}", user_did_hash, page_id);

        // Build ScribeArgs and spawn
        let args = self.build_scribe_args(page_id).await?;
        let actor = self.scribe_manager.spawn(page_id, actor_name, args).await?;

        // Notify page opened (for kunki node runtime)
        if let Some(ref tx) = *self.page_opened_tx.read().await {
            let _ = tx.try_send(page_id.to_string());
        }

        Ok(actor)
    }

    /// Build ScribeArgs for a page
    #[instrument(skip(self), fields(page_id = %page_id))]
    async fn build_scribe_args(&self, page_id: &str) -> Result<ScribeArgs> {
        let page_id_str = page_id.to_string();
        let (layers, aes_key) = self.load_decrypted_layers(page_id).await?;

        // Resolve SyncConfig from page permit
        let sync_config = self.resolve_sync_config(page_id).await;

        // Create peer resolver only if this is node mode (relay capability)
        let is_node = sync_config
            .as_ref()
            .map(|c| matches!(c.mode, scribe::state::SyncMode::Broadcast))
            .unwrap_or(false);

        let (layer_storage, vector_storage, peer_resolver) =
            self.create_scribe_storage(page_id_str.as_str(), aes_key, is_node);

        let sync_event_tx = self.sync_event_tx.clone();

        // Note: validation.lua and init.lua (derivation) are now loaded by kunki/LuaRuntime

        // Get our permit and identity info
        let our_permit = self
            .pages()
            .get(page_id)
            .ok()
            .flatten()
            .and_then(|p| p.permit);

        let our_did = self
            .get_identity()
            .await
            .map(|id| id.did().to_string())
            .unwrap_or_default();

        let our_username = self
            .identity_data()
            .ok()
            .flatten()
            .map(|d| d.username)
            .unwrap_or_default();

        // Get validation handle if set (node mode)
        let validation_handle = self.validation_handle.read().await.clone();

        // Create permit issuer whenever we have signing key + our permit.
        // Node mode uses this to issue recipient layer permits.
        // User mode uses this for creator-side AddLayerAccess issuance.
        let permit_issuer = self
            .create_permit_issuer(page_id, our_permit.as_ref())
            .await;

        Ok(ScribeArgs {
            page_id: page_id_str,
            layers,
            layer_storage,
            vector_storage,
            peer_resolver,
            permit_issuer,
            sync_config,
            sync_event_tx,
            capture_tx: self.capture_tx.read().await.clone(),
            validation_handle,
            our_permit,
            our_did,
            our_username,
            is_node,
        })
    }

    #[instrument(skip(self), fields(page_id = %page_id))]
    async fn load_decrypted_layers(
        &self,
        page_id: &str,
    ) -> Result<(HashMap<String, Layer>, [u8; 32])> {
        let (decrypted, aes_key) = self.pages().get_decrypted(page_id).await?;

        log::info!(
            "open_page: page_id={} loaded {} docs from DB (bare layer names)",
            page_id,
            decrypted.docs.len()
        );

        let mut layers = HashMap::new();
        for (name, doc_bytes) in decrypted.docs {
            let layer = if doc_bytes.is_empty() {
                Layer::new()
            } else {
                Layer::from_snapshot(&doc_bytes).unwrap_or_else(|_| Layer::new())
            };
            layers.insert(name, layer);
        }

        Ok((layers, aes_key))
    }

    fn create_scribe_storage(
        &self,
        page_id: &str,
        aes_key: [u8; 32],
        is_node: bool,
    ) -> (
        LayerStorageRef,
        PeerVectorStorageRef,
        Option<PeerResolverRef>,
    ) {
        use scribe_storage::{ButlerLayerStorage, ButlerPeerResolver, ButlerPeerVectorStorage};

        let layer_storage: LayerStorageRef = Arc::new(ButlerLayerStorage::new(
            self.store.clone(),
            page_id.to_string(),
            aes_key,
        ));

        let vector_storage: PeerVectorStorageRef = Arc::new(ButlerPeerVectorStorage::new(
            self.store.clone(),
            page_id.to_string(),
        ));

        let peer_resolver = if is_node {
            Some(Arc::new(ButlerPeerResolver::new(
                self.store.clone(),
                page_id.to_string(),
            )) as PeerResolverRef)
        } else {
            None
        };

        (layer_storage, vector_storage, peer_resolver)
    }

    async fn create_permit_issuer(
        &self,
        page_id: &str,
        permit_token: Option<&String>,
    ) -> Option<PermitIssuerRef> {
        use scribe_storage::ButlerPermitIssuer;

        match (self.signing_key().await, permit_token) {
            (Ok(signing_key), Some(token)) => Some(Arc::new(ButlerPermitIssuer::new(
                signing_key,
                token.clone(),
                page_id.to_string(),
                self.store.clone(),
            ))),
            _ => None,
        }
    }

    /// Close a page and stop its Scribe actor
    #[instrument(skip(self), fields(page_id = %page_id))]
    pub async fn close_page(&self, page_id: &str) -> Result<()> {
        self.scribe_manager.close(page_id).await;
        Ok(())
    }

    /// Get active Scribe count
    #[instrument(skip_all)]
    pub async fn active_scribe_count(&self) -> usize {
        self.scribe_manager.active_count().await
    }

    /// List active Scribes with their permits for peer subscription
    #[instrument(skip_all)]
    pub async fn list_active_scribes_for_peer(
        &self,
        _peer_did: &str,
    ) -> Result<Vec<(String, String)>> {
        let page_ids = self.scribe_manager.list_active_page_ids().await;
        let mut result = Vec::new();

        for page_id in page_ids {
            if let Ok(Some(page_data)) = self.pages().get(&page_id) {
                if let Some(permit) = page_data.permit {
                    result.push((page_id, permit));
                }
            }
        }

        Ok(result)
    }

    /// Resolve sync configuration from page permit
    #[instrument(skip(self), fields(page_id = %page_id))]
    async fn resolve_sync_config(&self, page_id: &str) -> Option<SyncConfig> {
        let page_data = self.pages().get(page_id).ok()??;
        let permit_str = page_data.permit.as_ref()?;
        let parsed = gurkha::PolicyPermit::from_token(permit_str).ok()?;

        let peer_caps = parsed.peer_capabilities();
        let mode = if peer_caps.relay {
            SyncMode::Broadcast
        } else {
            SyncMode::ToSource
        };

        let sync_target = match mode {
            SyncMode::ToSource => self
                .pages()
                .get_source_node(page_id)
                .ok()
                .flatten()
                .or_else(|| self.nodes().get_for_sync().ok().flatten()),
            SyncMode::Broadcast => None,
        };

        Some(SyncConfig { mode, sync_target })
    }

    /// Store a single peer state vector for a specific layer
    #[instrument(skip(self, state_vector), fields(peer_did = %peer_did, page_id = %page_id, layer_name = %layer_name))]
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

    /// Store peer's state vectors after successful content transfer
    #[instrument(skip(self), fields(page_id = %page_id, peer_did = %peer_did, peer_device_id = %peer_device_id))]
    pub async fn store_peer_vectors_from_page(
        &self,
        page_id: &str,
        peer_did: &str,
        peer_device_id: &str,
    ) -> Result<()> {
        let (decrypted_page, _aes_key) = self.pages().get_decrypted(page_id).await?;

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

    pub fn store(&self) -> &Arc<RedbStore> {
        &self.store
    }

    pub fn asset_store(&self) -> &Arc<AssetStore> {
        &self.asset_store
    }
}
