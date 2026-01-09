//! RedbStore - Main persistent storage for Osvauld using redb
//!
//! ## Schema (v2 - Live Sync)
//!
//! ### Core Tables
//! - IDENTITY: "self" → IdentityData, "keystore" → EncryptedKeyStore
//! - SPACES: {space_id} → SpaceData
//! - PAGES: {page_id} → PageData (note: key is just page_id, not space_id/page_id)
//! - LAYERS: {page_id}/{layer_name} → encrypted bytes
//!
//! ### Index Tables
//! - SPACE_PAGES: {space_id}/{page_id} → () (list pages in a space)
//!
//! ### Access Control Tables
//! - CONTACTS: {user_did} → ContactData (includes devices)
//! - SPACE_SUBSCRIPTIONS: {space_id}/{user_did} → SubscriptionData (user access to space)
//! - PERMIT_CIDS: {page_id}/{user_did} → cid string (issued permit record for revocation)
//!
//! ### Sync Tables
//! - VECTORS: {page_id}/{user_did}/{device_id} → HashMap<layer_name, state_vector_bytes>
//!
//! ### Node Tables
//! - SOVEREIGN_NODES: {node_id} → SovereignNode (external nodes owner connects to)
//! - OWNER_INFO: "owner" → OwnerInfo (owner info stored on node side)

use crate::error::{ButlerError, Result};
use crate::models::{
    ContactData, DeviceData, EncryptedKeyStore, SpaceData, IdentityData, OwnerInfo,
    PageData, SovereignNode,
};
use redb::{Database, ReadableTable, TableDefinition, ReadableDatabase};
use std::path::Path;
use std::sync::Arc;

// ==================== Table Definitions ====================

// Core tables
const IDENTITY: TableDefinition<&str, &[u8]> = TableDefinition::new("identity");
const SPACES: TableDefinition<&str, &[u8]> = TableDefinition::new("spaces");
const PAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("pages");
const LAYERS: TableDefinition<&str, &[u8]> = TableDefinition::new("layers");

// Index tables
/// SPACE_PAGES: {space_id}/{page_id} → () - List all pages in a space
const SPACE_PAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("space_pages");

// Access control tables
const CONTACTS: TableDefinition<&str, &[u8]> = TableDefinition::new("contacts");
/// SPACE_SUBSCRIPTIONS: {space_id}/{user_did} → SubscriptionData - User access to space
const SPACE_SUBSCRIPTIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("space_subscriptions");
/// PERMIT_CIDS: {page_id}/{user_did} → cid string - Record of issued permits
const PERMIT_CIDS: TableDefinition<&str, &str> = TableDefinition::new("permit_cids");

// Sync tables
/// VECTORS: {page_id}/{user_did}/{device_id} → HashMap<layer_name, state_vector_bytes>
const VECTORS: TableDefinition<&str, &[u8]> = TableDefinition::new("vectors");

// Node tables
const SOVEREIGN_NODES: TableDefinition<&str, &[u8]> = TableDefinition::new("sovereign_nodes");
const OWNER_INFO: TableDefinition<&str, &[u8]> = TableDefinition::new("owner_info");

// Viewer consent tables (viewer-issued permits stored by node)
/// VIEWER_CONSENT_SPACE: {viewer_did}/{space_id} → consent permit string
const VIEWER_CONSENT_SPACE: TableDefinition<&str, &str> = TableDefinition::new("viewer_consent_space");
/// VIEWER_CONSENT_PAGE: {viewer_did}/{page_id} → consent permit string
const VIEWER_CONSENT_PAGE: TableDefinition<&str, &str> = TableDefinition::new("viewer_consent_page");

// User page permits (for sync authorization - Node stores permits for owner/viewers)
/// USER_PAGE_PERMITS: {page_id}/{user_did} → permit string - User's permit for sync auth
const USER_PAGE_PERMITS: TableDefinition<&str, &str> = TableDefinition::new("user_page_permits");

// User space permits (Owner stores permits from nodes - proves space is published)
/// USER_SPACE_PERMITS: {space_id}/{node_did} → permit string - Node's permit for space
const USER_SPACE_PERMITS: TableDefinition<&str, &str> = TableDefinition::new("user_space_permits");

// Legacy tables (kept for migration, will be removed)
const DEVICES: TableDefinition<&str, &[u8]> = TableDefinition::new("devices");

/// RedbStore - Main persistent storage using redb
pub struct RedbStore {
    db: Arc<Database>,
}

impl RedbStore {
    /// Open or create a redb database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = Database::create(path)?;

        // Initialize tables by opening them once
        let write_txn = db.begin_write()?;
        {
            // Core tables
            let _ = write_txn.open_table(IDENTITY)?;
            let _ = write_txn.open_table(SPACES)?;
            let _ = write_txn.open_table(PAGES)?;
            let _ = write_txn.open_table(LAYERS)?;

            // Index tables
            let _ = write_txn.open_table(SPACE_PAGES)?;

            // Access control tables
            let _ = write_txn.open_table(CONTACTS)?;
            let _ = write_txn.open_table(SPACE_SUBSCRIPTIONS)?;
            let _ = write_txn.open_table(PERMIT_CIDS)?;

            // Sync tables
            let _ = write_txn.open_table(VECTORS)?;

            // Node tables
            let _ = write_txn.open_table(SOVEREIGN_NODES)?;
            let _ = write_txn.open_table(OWNER_INFO)?;

            // Viewer consent tables
            let _ = write_txn.open_table(VIEWER_CONSENT_SPACE)?;
            let _ = write_txn.open_table(VIEWER_CONSENT_PAGE)?;

            // User page permits (for sync authorization)
            let _ = write_txn.open_table(USER_PAGE_PERMITS)?;

            // User space permits (for published state tracking)
            let _ = write_txn.open_table(USER_SPACE_PERMITS)?;

            // Legacy tables (kept for migration)
            let _ = write_txn.open_table(DEVICES)?;
        }
        write_txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Flush all pending writes to disk (redb auto-commits, but this ensures durability)
    pub fn flush(&self) -> Result<()> {
        // redb commits are durable by default, nothing extra needed
        Ok(())
    }

    // =========================================================================
    // Identity Operations
    // =========================================================================

    pub fn get_identity(&self) -> Result<Option<IdentityData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(IDENTITY)?;

        match table.get("self")? {
            Some(guard) => {
                let bytes = guard.value();
                let identity: IdentityData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(identity))
            }
            None => Ok(None),
        }
    }

    pub fn set_identity(&self, identity: &IdentityData) -> Result<()> {
        let value = bincode::serialize(identity)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(IDENTITY)?;
            table.insert("self", value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_keystore(&self) -> Result<Option<EncryptedKeyStore>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(IDENTITY)?;

        match table.get("keystore")? {
            Some(guard) => {
                let bytes = guard.value();
                let keystore: EncryptedKeyStore = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(keystore))
            }
            None => Ok(None),
        }
    }

    pub fn set_keystore(&self, keystore: &EncryptedKeyStore) -> Result<()> {
        let value = bincode::serialize(keystore)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(IDENTITY)?;
            table.insert("keystore", value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn is_signed_up(&self) -> Result<bool> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(IDENTITY)?;

        let has_self = table.get("self")?.is_some();
        let has_keystore = table.get("keystore")?.is_some();
        Ok(has_self && has_keystore)
    }

    // =========================================================================
    // Space Operations
    // Key: {space_id}
    // =========================================================================

    pub fn put_space(&self, space: &SpaceData) -> Result<()> {
        let key = &space.meta.id;
        let value = bincode::serialize(space)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut spaces = write_txn.open_table(SPACES)?;
            spaces.insert(key.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_space(&self, space_id: &str) -> Result<Option<SpaceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACES)?;

        match table.get(space_id)? {
            Some(guard) => {
                let bytes = guard.value();
                let space: SpaceData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(space))
            }
            None => Ok(None),
        }
    }

    pub fn delete_space(&self, space_id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut spaces = write_txn.open_table(SPACES)?;
            let result = spaces.remove(space_id)?.is_some();
            result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    pub fn list_spaces(&self) -> Result<Vec<SpaceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACES)?;

        let mut spaces = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let space: SpaceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            spaces.push(space);
        }
        Ok(spaces)
    }

    pub fn list_child_spaces(&self, parent_id: &str) -> Result<Vec<SpaceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACES)?;

        let mut spaces = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let space: SpaceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if space.meta.parent_space_id.as_deref() == Some(parent_id) {
                spaces.push(space);
            }
        }
        Ok(spaces)
    }

    pub fn list_root_spaces(&self) -> Result<Vec<SpaceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACES)?;

        let mut spaces = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let space: SpaceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if space.meta.parent_space_id.is_none() {
                spaces.push(space);
            }
        }
        Ok(spaces)
    }

    // =========================================================================
    // Page Operations
    // Key: {page_id} (v2 schema - page_id is globally unique)
    // Also maintains SPACE_PAGES index: {space_id}/{page_id} → ()
    // =========================================================================

    pub fn put_page(&self, page: &PageData) -> Result<()> {
        let value = bincode::serialize(page)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            // Store page by page_id
            let mut pages_table = write_txn.open_table(PAGES)?;
            pages_table.insert(page.meta.id.as_str(), value.as_slice())?;

            // Update SPACE_PAGES index
            let mut index_table = write_txn.open_table(SPACE_PAGES)?;
            let index_key = format!("{}/{}", page.meta.space_id, page.meta.id);
            index_table.insert(index_key.as_str(), &[] as &[u8])?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get page by page_id (v2 - no space_id needed)
    pub fn get_page_by_id(&self, page_id: &str) -> Result<Option<PageData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PAGES)?;

        match table.get(page_id)? {
            Some(guard) => {
                let bytes = guard.value();
                let page: PageData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(page))
            }
            None => Ok(None),
        }
    }

    /// Get page (legacy API - space_id ignored, uses page_id only)
    pub fn get_page(&self, _space_id: &str, page_id: &str) -> Result<Option<PageData>> {
        self.get_page_by_id(page_id)
    }

    pub fn delete_page(&self, space_id: &str, page_id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            // Remove from PAGES table
            let mut pages_table = write_txn.open_table(PAGES)?;
            let result = pages_table.remove(page_id)?.is_some();

            // Remove from SPACE_PAGES index
            let mut index_table = write_txn.open_table(SPACE_PAGES)?;
            let index_key = format!("{}/{}", space_id, page_id);
            let _ = index_table.remove(index_key.as_str())?;

            result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    /// List pages in space using SPACE_PAGES index
    pub fn list_pages_in_space(&self, space_id: &str) -> Result<Vec<PageData>> {
        let read_txn = self.db.begin_read()?;
        let index_table = read_txn.open_table(SPACE_PAGES)?;
        let pages_table = read_txn.open_table(PAGES)?;

        let prefix = format!("{}/", space_id);
        let mut pages = Vec::new();

        for result in index_table.range(prefix.as_str()..)? {
            let (key, _) = result?;
            let key_str = key.value();

            // Stop if we've passed the prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract page_id from index key
            if let Some(page_id) = key_str.strip_prefix(&prefix) {
                if let Some(page_guard) = pages_table.get(page_id)? {
                    let page: PageData = bincode::deserialize(page_guard.value())
                        .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                    pages.push(page);
                }
            }
        }
        Ok(pages)
    }

    /// List page IDs in space (without loading full PageData)
    pub fn list_page_ids_in_space(&self, space_id: &str) -> Result<Vec<String>> {
        let read_txn = self.db.begin_read()?;
        let index_table = read_txn.open_table(SPACE_PAGES)?;

        let prefix = format!("{}/", space_id);
        let mut page_ids = Vec::new();

        for result in index_table.range(prefix.as_str()..)? {
            let (key, _) = result?;
            let key_str = key.value();

            if !key_str.starts_with(&prefix) {
                break;
            }

            if let Some(page_id) = key_str.strip_prefix(&prefix) {
                page_ids.push(page_id.to_string());
            }
        }
        Ok(page_ids)
    }

    pub fn list_all_pages(&self) -> Result<Vec<PageData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PAGES)?;

        let mut pages = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let page: PageData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            pages.push(page);
        }
        Ok(pages)
    }

    /// Find a page by ID (v2 - direct lookup, no scan needed)
    pub fn find_page_by_id(&self, page_id: &str) -> Result<Option<PageData>> {
        self.get_page_by_id(page_id)
    }

    /// Get all layers for a page (returns encrypted bytes mapped by layer name)
    pub fn get_all_layers(&self, page_id: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYERS)?;

        let prefix = format!("{}/", page_id);
        let mut layers = Vec::new();

        for result in table.range(prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();

            // Stop if we've passed the prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract layer_name from key
            if let Some(layer_name) = key_str.strip_prefix(&prefix) {
                layers.push((layer_name.to_string(), value.value().to_vec()));
            }
        }
        Ok(layers)
    }

    // =========================================================================
    // Layer Operations (encrypted snapshot bytes)
    // Key: {page_id}/{layer_name} (hierarchical)
    // =========================================================================

    /// Store a layer with hierarchical key: {page_id}/{layer_name}
    pub fn put_layer(
        &self,
        page_id: &str,
        layer_name: &str,
        encrypted_bytes: &[u8],
    ) -> Result<()> {
        let key = format!("{}/{}", page_id, layer_name);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LAYERS)?;
            table.insert(key.as_str(), encrypted_bytes)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a layer by page_id and layer_name
    pub fn get_layer(
        &self,
        page_id: &str,
        layer_name: &str,
    ) -> Result<Option<Vec<u8>>> {
        let key = format!("{}/{}", page_id, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYERS)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Delete a specific layer
    pub fn delete_layer(&self, page_id: &str, layer_name: &str) -> Result<bool> {
        let key = format!("{}/{}", page_id, layer_name);
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(LAYERS)?;
            let result = table.remove(key.as_str())?.is_some(); result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    /// List all layer names for a page
    pub fn list_layer_names(&self, page_id: &str) -> Result<Vec<String>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYERS)?;

        let prefix = format!("{}/", page_id);
        let mut layer_names = Vec::new();

        for result in table.range(prefix.as_str()..)? {
            let (key, _value) = result?;
            let key_str = key.value();

            // Stop if we've passed the prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract layer_name from key
            if let Some(layer_name) = key_str.strip_prefix(&prefix) {
                layer_names.push(layer_name.to_string());
            }
        }
        Ok(layer_names)
    }

    /// Delete all layers for a page
    pub fn delete_all_layers(&self, page_id: &str) -> Result<usize> {
        let layer_names = self.list_layer_names(page_id)?;
        let count = layer_names.len();

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LAYERS)?;
            for layer_name in &layer_names {
                let key = format!("{}/{}", page_id, layer_name);
                table.remove(key.as_str())?;
            }
        }
        write_txn.commit()?;
        Ok(count)
    }

    /// Check if a layer exists
    pub fn layer_exists(&self, page_id: &str, layer_name: &str) -> Result<bool> {
        let key = format!("{}/{}", page_id, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYERS)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    // =========================================================================
    // Device Operations
    // Key: {device_id}
    // =========================================================================

    pub fn put_device(&self, device: &DeviceData) -> Result<()> {
        let value = bincode::serialize(device)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(DEVICES)?;
            table.insert(device.id.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_device(&self, device_id: &str) -> Result<Option<DeviceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DEVICES)?;

        match table.get(device_id)? {
            Some(guard) => {
                let bytes = guard.value();
                let device: DeviceData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(device))
            }
            None => Ok(None),
        }
    }

    pub fn delete_device(&self, device_id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(DEVICES)?;
            let result = table.remove(device_id)?.is_some(); result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DEVICES)?;

        let mut devices = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let device: DeviceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            devices.push(device);
        }
        Ok(devices)
    }

    pub fn get_current_device(&self) -> Result<Option<DeviceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DEVICES)?;

        for result in table.iter()? {
            let (_, value) = result?;
            let device: DeviceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if device.is_current {
                return Ok(Some(device));
            }
        }
        Ok(None)
    }

    pub fn get_node_device(&self) -> Result<Option<DeviceData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DEVICES)?;

        for result in table.iter()? {
            let (_, value) = result?;
            let device: DeviceData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if device.is_node {
                return Ok(Some(device));
            }
        }
        Ok(None)
    }

    // =========================================================================
    // Contact Operations
    // Key: {user_did}
    // =========================================================================

    pub fn put_contact(&self, contact: &ContactData) -> Result<()> {
        let value = bincode::serialize(contact)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CONTACTS)?;
            table.insert(contact.did.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_contact(&self, user_did: &str) -> Result<Option<ContactData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CONTACTS)?;

        match table.get(user_did)? {
            Some(guard) => {
                let bytes = guard.value();
                let contact: ContactData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(contact))
            }
            None => Ok(None),
        }
    }

    pub fn delete_contact(&self, user_did: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(CONTACTS)?;
            let result = table.remove(user_did)?.is_some(); result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    pub fn list_contacts(&self) -> Result<Vec<ContactData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CONTACTS)?;

        let mut contacts = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let contact: ContactData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            contacts.push(contact);
        }
        Ok(contacts)
    }

    // Note: add_share_to_contact removed - share tracking moved to SPACE_SUBSCRIPTIONS
    // Note: get_users_for_page removed - use SPACE_SUBSCRIPTIONS instead
    // Note: Node Registry Operations (NODES table) removed - identity stored in IDENTITY table

    // =========================================================================
    // Sovereign Node Operations (external nodes we connect to)
    // Key: {node_id} (iroh NodeId)
    // =========================================================================

    pub fn put_sovereign_node(&self, node: &SovereignNode) -> Result<()> {
        let value = bincode::serialize(node)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(SOVEREIGN_NODES)?;
            table.insert(node.node_id.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_sovereign_node(&self, node_id: &str) -> Result<Option<SovereignNode>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SOVEREIGN_NODES)?;

        match table.get(node_id)? {
            Some(guard) => {
                let bytes = guard.value();
                let node: SovereignNode = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(node))
            }
            None => Ok(None),
        }
    }

    pub fn delete_sovereign_node(&self, node_id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(SOVEREIGN_NODES)?;
            let result = table.remove(node_id)?.is_some();
            result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    pub fn list_sovereign_nodes(&self) -> Result<Vec<SovereignNode>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SOVEREIGN_NODES)?;

        let mut nodes = Vec::new();
        for result in table.iter()? {
            let (key, value) = result?;
            match bincode::deserialize::<SovereignNode>(value.value()) {
                Ok(node) => nodes.push(node),
                Err(e) => {
                    // Skip records that fail to deserialize (likely schema mismatch from old version)
                    tracing::warn!(
                        "Skipping corrupt sovereign node record '{}': {}",
                        key.value(),
                        e
                    );
                }
            }
        }
        Ok(nodes)
    }

    pub fn get_connected_sovereign_nodes(&self) -> Result<Vec<SovereignNode>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SOVEREIGN_NODES)?;

        let mut nodes = Vec::new();
        for result in table.iter()? {
            let (key, value) = result?;
            match bincode::deserialize::<SovereignNode>(value.value()) {
                Ok(node) => {
                    if node.is_connected {
                        nodes.push(node);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Skipping corrupt sovereign node record '{}': {}",
                        key.value(),
                        e
                    );
                }
            }
        }
        Ok(nodes)
    }

    pub fn set_sovereign_node_connected(&self, node_id: &str, connected: bool) -> Result<bool> {
        if let Some(mut node) = self.get_sovereign_node(node_id)? {
            node.set_connected(connected);
            self.put_sovereign_node(&node)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Store the permit for this sovereign node (single permit for owner<->node)
    pub fn set_sovereign_node_permit(&self, node_id: &str, permit: String) -> Result<bool> {
        if let Some(mut node) = self.get_sovereign_node(node_id)? {
            node.set_permit(permit);
            self.put_sovereign_node(&node)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // =========================================================================
    // Owner Info Operations (Node side - stores info about the owner)
    // Key: "owner" (only one owner per node)
    // =========================================================================

    /// Get the owner info (only one owner per node)
    pub fn get_owner(&self) -> Result<Option<OwnerInfo>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(OWNER_INFO)?;

        match table.get("owner")? {
            Some(guard) => {
                let bytes = guard.value();
                let owner: OwnerInfo = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(owner))
            }
            None => Ok(None),
        }
    }

    /// Store owner info (only one owner per node)
    pub fn set_owner(&self, owner: &OwnerInfo) -> Result<()> {
        let value = bincode::serialize(owner)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(OWNER_INFO)?;
            table.insert("owner", value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Update the permit for owner<->node relationship
    pub fn set_owner_permit(&self, permit: String) -> Result<bool> {
        if let Some(mut owner) = self.get_owner()? {
            owner.set_permit(permit);
            self.set_owner(&owner)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Update owner's last connected timestamp
    pub fn update_owner_last_connected(&self) -> Result<bool> {
        if let Some(mut owner) = self.get_owner()? {
            owner.update_last_connected();
            self.set_owner(&owner)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if this node has an owner
    pub fn has_owner(&self) -> Result<bool> {
        Ok(self.get_owner()?.is_some())
    }

    // =========================================================================
    // Peer Vector Operations (for CRDT sync state tracking)
    // Key: {page_id}/{user_did}/{device_id}
    // Value: HashMap<layer_name, state_vector_bytes>
    // =========================================================================

    /// Get peer vectors for a specific peer on a page.
    ///
    /// **Context**: Used during sync to determine what updates a peer needs.
    /// Returns None if we haven't synced with this peer before.
    pub fn get_peer_vectors(
        &self,
        page_id: &str,
        user_did: &str,
        device_id: &str,
    ) -> Result<Option<std::collections::HashMap<String, Vec<u8>>>> {
        let key = format!("{}/{}/{}", page_id, user_did, device_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VECTORS)?;

        match table.get(key.as_str())? {
            Some(guard) => {
                let bytes = guard.value();
                let vectors: std::collections::HashMap<String, Vec<u8>> =
                    bincode::deserialize(bytes)
                        .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(vectors))
            }
            None => Ok(None),
        }
    }

    /// Store peer vectors for a specific peer on a page.
    ///
    /// **Context**: Called after successful sync to record the peer's state.
    /// Enables efficient delta sync on next connection.
    pub fn put_peer_vectors(
        &self,
        page_id: &str,
        user_did: &str,
        device_id: &str,
        vectors: &std::collections::HashMap<String, Vec<u8>>,
    ) -> Result<()> {
        let key = format!("{}/{}/{}", page_id, user_did, device_id);
        let value = bincode::serialize(vectors)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VECTORS)?;
            table.insert(key.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a single layer's peer vector.
    ///
    /// **Context**: Convenience method for single-layer sync operations.
    pub fn get_peer_vector_for_layer(
        &self,
        page_id: &str,
        user_did: &str,
        device_id: &str,
        layer_name: &str,
    ) -> Result<Option<Vec<u8>>> {
        if let Some(vectors) = self.get_peer_vectors(page_id, user_did, device_id)? {
            Ok(vectors.get(layer_name).cloned())
        } else {
            Ok(None)
        }
    }

    /// Update a single layer's peer vector.
    ///
    /// **Context**: Called after syncing a specific layer.
    /// Preserves other layer vectors in the map.
    pub fn put_peer_vector_for_layer(
        &self,
        page_id: &str,
        user_did: &str,
        device_id: &str,
        layer_name: &str,
        vector: Vec<u8>,
    ) -> Result<()> {
        let mut vectors = self
            .get_peer_vectors(page_id, user_did, device_id)?
            .unwrap_or_default();
        vectors.insert(layer_name.to_string(), vector);
        self.put_peer_vectors(page_id, user_did, device_id, &vectors)
    }

    /// Delete all peer vectors for a specific peer on a page.
    ///
    /// **Context**: Called when a peer's access is revoked.
    pub fn delete_peer_vectors(
        &self,
        page_id: &str,
        user_did: &str,
        device_id: &str,
    ) -> Result<bool> {
        let key = format!("{}/{}/{}", page_id, user_did, device_id);
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(VECTORS)?;
            let result = table.remove(key.as_str())?.is_some();
            result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    /// Delete all peer vectors for a page (all peers).
    ///
    /// **Context**: Called when a page is deleted.
    pub fn delete_all_peer_vectors_for_page(&self, page_id: &str) -> Result<usize> {
        let prefix = format!("{}/", page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VECTORS)?;

        // Collect keys to delete
        let mut keys_to_delete = Vec::new();
        for result in table.range(prefix.as_str()..)? {
            let (key, _) = result?;
            let key_str = key.value();
            if !key_str.starts_with(&prefix) {
                break;
            }
            keys_to_delete.push(key_str.to_string());
        }
        drop(table);
        drop(read_txn);

        let count = keys_to_delete.len();
        if count > 0 {
            let write_txn = self.db.begin_write()?;
            {
                let mut table = write_txn.open_table(VECTORS)?;
                for key in &keys_to_delete {
                    table.remove(key.as_str())?;
                }
            }
            write_txn.commit()?;
        }
        Ok(count)
    }

    /// List all peers with vectors for a given page.
    ///
    /// **Context**: Used to iterate over all known peers for a page.
    /// Returns tuples of (user_did, device_id).
    pub fn list_peers_for_page(&self, page_id: &str) -> Result<Vec<(String, String)>> {
        let prefix = format!("{}/", page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VECTORS)?;

        let mut peers = Vec::new();
        for result in table.range(prefix.as_str()..)? {
            let (key, _) = result?;
            let key_str = key.value();
            if !key_str.starts_with(&prefix) {
                break;
            }

            // Parse {page_id}/{user_did}/{device_id}
            let rest = key_str.strip_prefix(&prefix).unwrap_or("");
            if let Some((user_did, device_id)) = rest.split_once('/') {
                peers.push((user_did.to_string(), device_id.to_string()));
            }
        }
        Ok(peers)
    }

    // =========================================================================
    // Permit CID Operations (for revocation tracking)
    // Key: {page_id}/{user_did} → cid string (content-addressed permit hash)
    // Used to track issued permits for revocation
    // =========================================================================

    /// Store a permit CID for a specific page and user.
    ///
    /// **Context**: Called when issuing a permit to track it for potential revocation.
    pub fn put_permit_cid(
        &self,
        page_id: &str,
        user_did: &str,
        cid: &str,
    ) -> Result<()> {
        let key = format!("{}/{}", page_id, user_did);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(PERMIT_CIDS)?;
            table.insert(key.as_str(), cid)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a permit CID for a specific page and user.
    ///
    /// **Context**: Used to check if a permit has been issued and get its CID.
    pub fn get_permit_cid(
        &self,
        page_id: &str,
        user_did: &str,
    ) -> Result<Option<String>> {
        let key = format!("{}/{}", page_id, user_did);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PERMIT_CIDS)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_string())),
            None => Ok(None),
        }
    }

    /// Delete a permit CID for a specific page and user.
    ///
    /// **Context**: Called when revoking access - marks the permit as revoked.
    pub fn delete_permit_cid(
        &self,
        page_id: &str,
        user_did: &str,
    ) -> Result<bool> {
        let key = format!("{}/{}", page_id, user_did);
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(PERMIT_CIDS)?;
            let result = table.remove(key.as_str())?.is_some();
            result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    /// List all permit CIDs for a given page.
    ///
    /// **Context**: Used to enumerate all issued permits for a page.
    pub fn list_permit_cids_for_page(&self, page_id: &str) -> Result<Vec<(String, String)>> {
        let prefix = format!("{}/", page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PERMIT_CIDS)?;

        let mut cids = Vec::new();
        for result in table.range(prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();

            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract user_did from key
            if let Some(user_did) = key_str.strip_prefix(&prefix) {
                cids.push((user_did.to_string(), value.value().to_string()));
            }
        }
        Ok(cids)
    }

    /// List all user_dids authorized to access a page.
    ///
    /// **Context**: Called by Scribe in node mode to get sync targets.
    /// **We query**: PERMIT_CIDS table for this page.
    /// **We return**: List of user_dids (Coordinator handles device resolution).
    pub fn list_authorized_users_for_page(&self, page_id: &str) -> Result<Vec<String>> {
        let prefix = format!("{}/", page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PERMIT_CIDS)?;

        let mut users = Vec::new();
        for result in table.range(prefix.as_str()..)? {
            let (key, _cid) = result?;
            let key_str = key.value();

            if !key_str.starts_with(&prefix) {
                break;
            }

            if let Some(user_did) = key_str.strip_prefix(&prefix) {
                users.push(user_did.to_string());
            }
        }
        Ok(users)
    }

    /// Delete all permit CIDs for a page.
    ///
    /// **Context**: Called when a page is deleted.
    pub fn delete_all_permit_cids_for_page(&self, page_id: &str) -> Result<usize> {
        let prefix = format!("{}/", page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PERMIT_CIDS)?;

        // Collect keys to delete
        let mut keys_to_delete = Vec::new();
        for result in table.range(prefix.as_str()..)? {
            let (key, _) = result?;
            let key_str = key.value();
            if !key_str.starts_with(&prefix) {
                break;
            }
            keys_to_delete.push(key_str.to_string());
        }
        drop(table);
        drop(read_txn);

        let count = keys_to_delete.len();
        if count > 0 {
            let write_txn = self.db.begin_write()?;
            {
                let mut table = write_txn.open_table(PERMIT_CIDS)?;
                for key in &keys_to_delete {
                    table.remove(key.as_str())?;
                }
            }
            write_txn.commit()?;
        }
        Ok(count)
    }

    /// Check if a permit CID exists for a page and user.
    ///
    /// **Context**: Quick check for whether a permit has been issued.
    pub fn has_permit_cid(&self, page_id: &str, user_did: &str) -> Result<bool> {
        let key = format!("{}/{}", page_id, user_did);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PERMIT_CIDS)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    // =========================================================================
    // Space Subscription Operations (user access to spaces)
    // Key: {space_id}/{user_did} → SubscriptionData (serialized)
    // Tracks which users have access to which spaces
    // =========================================================================

    /// Store a space subscription.
    ///
    /// **Context**: Called when granting a user access to a space.
    pub fn put_space_subscription(
        &self,
        space_id: &str,
        user_did: &str,
        data: &[u8],
    ) -> Result<()> {
        let key = format!("{}/{}", space_id, user_did);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(SPACE_SUBSCRIPTIONS)?;
            table.insert(key.as_str(), data)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a space subscription.
    ///
    /// **Context**: Check if a user has access to a space.
    pub fn get_space_subscription(
        &self,
        space_id: &str,
        user_did: &str,
    ) -> Result<Option<Vec<u8>>> {
        let key = format!("{}/{}", space_id, user_did);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACE_SUBSCRIPTIONS)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Delete a space subscription.
    ///
    /// **Context**: Called when revoking a user's access to a space.
    pub fn delete_space_subscription(
        &self,
        space_id: &str,
        user_did: &str,
    ) -> Result<bool> {
        let key = format!("{}/{}", space_id, user_did);
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(SPACE_SUBSCRIPTIONS)?;
            let result = table.remove(key.as_str())?.is_some();
            result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    /// List all user DIDs subscribed to a space.
    ///
    /// **Context**: Used to enumerate all users with access to a space.
    pub fn list_space_subscribers(&self, space_id: &str) -> Result<Vec<String>> {
        let prefix = format!("{}/", space_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACE_SUBSCRIPTIONS)?;

        let mut subscribers = Vec::new();
        for result in table.range(prefix.as_str()..)? {
            let (key, _) = result?;
            let key_str = key.value();

            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract user_did from key
            if let Some(user_did) = key_str.strip_prefix(&prefix) {
                subscribers.push(user_did.to_string());
            }
        }
        Ok(subscribers)
    }

    /// Check if a user is subscribed to a space.
    ///
    /// **Context**: Quick check for space access.
    pub fn has_space_subscription(&self, space_id: &str, user_did: &str) -> Result<bool> {
        let key = format!("{}/{}", space_id, user_did);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACE_SUBSCRIPTIONS)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    /// Delete all subscriptions for a space.
    ///
    /// **Context**: Called when a space is deleted.
    pub fn delete_all_space_subscriptions(&self, space_id: &str) -> Result<usize> {
        let prefix = format!("{}/", space_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACE_SUBSCRIPTIONS)?;

        // Collect keys to delete
        let mut keys_to_delete = Vec::new();
        for result in table.range(prefix.as_str()..)? {
            let (key, _) = result?;
            let key_str = key.value();
            if !key_str.starts_with(&prefix) {
                break;
            }
            keys_to_delete.push(key_str.to_string());
        }
        drop(table);
        drop(read_txn);

        let count = keys_to_delete.len();
        if count > 0 {
            let write_txn = self.db.begin_write()?;
            {
                let mut table = write_txn.open_table(SPACE_SUBSCRIPTIONS)?;
                for key in &keys_to_delete {
                    table.remove(key.as_str())?;
                }
            }
            write_txn.commit()?;
        }
        Ok(count)
    }

    // =========================================================================
    // Viewer Consent Operations (viewer-issued permits stored by node)
    // Key: {viewer_did}/{resource_id} → consent permit string
    // Tracks viewer consent for receiving sync updates
    // =========================================================================

    /// Store a viewer's space consent permit.
    ///
    /// **Context**: Node receives SyncConsentGrant from viewer.
    /// The permit is viewer-issued, expressing consent for sync updates.
    pub fn put_viewer_space_consent(
        &self,
        viewer_did: &str,
        space_id: &str,
        consent_permit: &str,
    ) -> Result<()> {
        let key = format!("{}/{}", viewer_did, space_id);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VIEWER_CONSENT_SPACE)?;
            table.insert(key.as_str(), consent_permit)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a viewer's space consent permit.
    ///
    /// **Context**: Node needs consent permit to send sync updates.
    pub fn get_viewer_space_consent(
        &self,
        viewer_did: &str,
        space_id: &str,
    ) -> Result<Option<String>> {
        let key = format!("{}/{}", viewer_did, space_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VIEWER_CONSENT_SPACE)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_string())),
            None => Ok(None),
        }
    }

    /// Store a viewer's page consent permit.
    ///
    /// **Context**: Node receives SyncConsentGrant from viewer.
    /// The permit is viewer-issued, expressing consent for page layer updates.
    pub fn put_viewer_page_consent(
        &self,
        viewer_did: &str,
        page_id: &str,
        consent_permit: &str,
    ) -> Result<()> {
        let key = format!("{}/{}", viewer_did, page_id);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VIEWER_CONSENT_PAGE)?;
            table.insert(key.as_str(), consent_permit)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a viewer's page consent permit.
    ///
    /// **Context**: Node needs consent permit to send layer updates.
    pub fn get_viewer_page_consent(
        &self,
        viewer_did: &str,
        page_id: &str,
    ) -> Result<Option<String>> {
        let key = format!("{}/{}", viewer_did, page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VIEWER_CONSENT_PAGE)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_string())),
            None => Ok(None),
        }
    }

    /// Check if viewer has space consent.
    pub fn has_viewer_space_consent(&self, viewer_did: &str, space_id: &str) -> Result<bool> {
        let key = format!("{}/{}", viewer_did, space_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VIEWER_CONSENT_SPACE)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    /// Check if viewer has page consent.
    pub fn has_viewer_page_consent(&self, viewer_did: &str, page_id: &str) -> Result<bool> {
        let key = format!("{}/{}", viewer_did, page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VIEWER_CONSENT_PAGE)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    // =========================================================================
    // User Page Permits (for sync authorization - Node stores permits)
    // Key: {page_id}/{user_did} → permit string
    // Used for permit-based sync auth (replaces DID whitelist)
    // =========================================================================

    /// Store a user's permit for a page (Node mode - for sync authorization).
    ///
    /// **Context**: Node receives owner's permit during PublishPage.
    /// This permit is used for sync authorization (layer permissions).
    pub fn put_user_page_permit(
        &self,
        page_id: &str,
        user_did: &str,
        permit: &str,
    ) -> Result<()> {
        let key = format!("{}/{}", page_id, user_did);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(USER_PAGE_PERMITS)?;
            table.insert(key.as_str(), permit)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a user's permit for a page (Node mode - for sync authorization).
    ///
    /// **Context**: Node needs permit to authorize sync operations.
    pub fn get_user_page_permit(
        &self,
        page_id: &str,
        user_did: &str,
    ) -> Result<Option<String>> {
        let key = format!("{}/{}", page_id, user_did);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(USER_PAGE_PERMITS)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_string())),
            None => Ok(None),
        }
    }

    // ==================== User Space Permits ====================

    /// Store a node's permit for a space (Owner mode - proves space is published)
    ///
    /// **Context**: Owner receives permit from node after PublishSpaceAck.
    /// This permit proves the space is published to that node.
    pub fn put_user_space_permit(
        &self,
        space_id: &str,
        node_did: &str,
        permit: &str,
    ) -> Result<()> {
        let key = format!("{}/{}", space_id, node_did);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(USER_SPACE_PERMITS)?;
            table.insert(key.as_str(), permit)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a node's permit for a space (Owner mode - check if published)
    pub fn get_user_space_permit(
        &self,
        space_id: &str,
        node_did: &str,
    ) -> Result<Option<String>> {
        let key = format!("{}/{}", space_id, node_did);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(USER_SPACE_PERMITS)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_string())),
            None => Ok(None),
        }
    }

    /// List all nodes with permits for a space (Owner mode - get published nodes)
    ///
    /// Returns DIDs of nodes that have issued permits for this space.
    pub fn list_nodes_with_space_permits(&self, space_id: &str) -> Result<Vec<String>> {
        let prefix = format!("{}/", space_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(USER_SPACE_PERMITS)?;

        let mut nodes = Vec::new();
        for entry in table.iter()? {
            let (key, _value) = entry?;
            let key_str = key.value();
            if key_str.starts_with(&prefix) {
                // Extract node_did from key (space_id/node_did)
                if let Some(node_did) = key_str.strip_prefix(&prefix) {
                    nodes.push(node_did.to_string());
                }
            }
        }
        Ok(nodes)
    }
}
