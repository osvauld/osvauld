//! RedbStore - Main persistent storage for Osvauld using redb
//!
//! Hierarchical key design:
//! - identity: "self" → IdentityData, "keystore" → EncryptedKeyStore
//! - spaces: {space_id} → SpaceData
//! - space_children: {parent_id}/{child_id} → () (index)
//! - pages: {space_id}/{page_id} → PageData
//! - layers: {page_id}/{layer_name} → encrypted bytes (hierarchical!)
//! - devices: {device_id} → DeviceData
//! - contacts: {user_did} → ContactData
//! - nodes: {user_did} → NodeInfo
//!
//! ## Layer Storage
//! Layers use hierarchical keys: `{page_id}/{layer_name}`
//! - Layer names come from permit template (e.g., "content", "comments")
//! - All layers for a page are encrypted with the same AES key
//! - AES key stored (encrypted) in PageMeta.encrypted_key

use crate::error::{ButlerError, Result};
use crate::models::{
    ContactData, DeviceData, EncryptedKeyStore, SpaceData, IdentityData, NodeInfo, OwnerInfo,
    PageData, SovereignNode,
};
use redb::{Database, ReadableTable, TableDefinition, ReadableDatabase};
use std::path::Path;
use std::sync::Arc;

// Table definitions with typed keys and values
const IDENTITY: TableDefinition<&str, &[u8]> = TableDefinition::new("identity");
const SPACES: TableDefinition<&str, &[u8]> = TableDefinition::new("spaces");
const SPACE_CHILDREN: TableDefinition<&str, &str> = TableDefinition::new("space_children");
const PAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("pages");
const LAYERS: TableDefinition<&str, &[u8]> = TableDefinition::new("layers");
const DEVICES: TableDefinition<&str, &[u8]> = TableDefinition::new("devices");
const CONTACTS: TableDefinition<&str, &[u8]> = TableDefinition::new("contacts");
const NODES: TableDefinition<&str, &[u8]> = TableDefinition::new("nodes");
const SOVEREIGN_NODES: TableDefinition<&str, &[u8]> = TableDefinition::new("sovereign_nodes");
const OWNER_INFO: TableDefinition<&str, &[u8]> = TableDefinition::new("owner_info");

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
            let _ = write_txn.open_table(IDENTITY)?;
            let _ = write_txn.open_table(SPACES)?;
            let _ = write_txn.open_table(SPACE_CHILDREN)?;
            let _ = write_txn.open_table(PAGES)?;
            let _ = write_txn.open_table(LAYERS)?;
            let _ = write_txn.open_table(DEVICES)?;
            let _ = write_txn.open_table(CONTACTS)?;
            let _ = write_txn.open_table(NODES)?;
            let _ = write_txn.open_table(SOVEREIGN_NODES)?;
            let _ = write_txn.open_table(OWNER_INFO)?;
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

            // Update parent-child index if has parent
            if let Some(ref parent_id) = space.meta.parent_space_id {
                let mut children = write_txn.open_table(SPACE_CHILDREN)?;
                let child_key = format!("{}/{}", parent_id, key);
                children.insert(child_key.as_str(), key.as_str())?;
            }
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

            // Get space to check parent
            if let Some(guard) = spaces.get(space_id)? {
                let space: SpaceData = bincode::deserialize(guard.value())
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;

                // Remove from parent-child index
                if let Some(ref parent_id) = space.meta.parent_space_id {
                    let mut children = write_txn.open_table(SPACE_CHILDREN)?;
                    let child_key = format!("{}/{}", parent_id, space_id);
                    children.remove(child_key.as_str())?;
                }
            }

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
        let children_table = read_txn.open_table(SPACE_CHILDREN)?;
        let spaces_table = read_txn.open_table(SPACES)?;

        let prefix = format!("{}/", parent_id);
        let mut spaces = Vec::new();

        for result in children_table.range(prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();

            // Stop if we've passed the prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            let space_id = value.value();
            if let Some(space_guard) = spaces_table.get(space_id)? {
                let space: SpaceData = bincode::deserialize(space_guard.value())
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
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
    // Key: {space_id}/{page_id}
    // =========================================================================

    pub fn put_page(&self, page: &PageData) -> Result<()> {
        let key = format!("{}/{}", page.meta.space_id, page.meta.id);
        let value = bincode::serialize(page)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(PAGES)?;
            table.insert(key.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_page(&self, space_id: &str, page_id: &str) -> Result<Option<PageData>> {
        let key = format!("{}/{}", space_id, page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PAGES)?;

        match table.get(key.as_str())? {
            Some(guard) => {
                let bytes = guard.value();
                let page: PageData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(page))
            }
            None => Ok(None),
        }
    }

    pub fn delete_page(&self, space_id: &str, page_id: &str) -> Result<bool> {
        let key = format!("{}/{}", space_id, page_id);
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(PAGES)?;
            let result = table.remove(key.as_str())?.is_some(); result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    pub fn list_pages_in_space(&self, space_id: &str) -> Result<Vec<PageData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PAGES)?;

        let prefix = format!("{}/", space_id);
        let mut pages = Vec::new();

        for result in table.range(prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();

            // Stop if we've passed the prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            let page: PageData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            pages.push(page);
        }
        Ok(pages)
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

    /// Find a page by ID without knowing the space_id (scans all pages)
    pub fn find_page_by_id(&self, page_id: &str) -> Result<Option<PageData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PAGES)?;

        for result in table.iter()? {
            let (_, value) = result?;
            let page: PageData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if page.meta.id == page_id {
                return Ok(Some(page));
            }
        }
        Ok(None)
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

    // =========================================================================
    // Node Registry Operations
    // Key: {user_did}
    // =========================================================================

    pub fn put_node(&self, node: &NodeInfo) -> Result<()> {
        let value = bincode::serialize(node)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(NODES)?;
            table.insert(node.user_did.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_node(&self, user_did: &str) -> Result<Option<NodeInfo>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(NODES)?;

        match table.get(user_did)? {
            Some(guard) => {
                let bytes = guard.value();
                let node: NodeInfo = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(node))
            }
            None => Ok(None),
        }
    }

    pub fn delete_node(&self, user_did: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(NODES)?;
            let result = table.remove(user_did)?.is_some(); result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    pub fn list_nodes(&self) -> Result<Vec<NodeInfo>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(NODES)?;

        let mut nodes = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let node: NodeInfo = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            nodes.push(node);
        }
        Ok(nodes)
    }

    pub fn list_online_nodes(&self) -> Result<Vec<NodeInfo>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(NODES)?;

        let mut nodes = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let node: NodeInfo = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if node.is_online {
                nodes.push(node);
            }
        }
        Ok(nodes)
    }

    pub fn set_node_online(&self, user_did: &str, online: bool) -> Result<bool> {
        if let Some(mut node) = self.get_node(user_did)? {
            node.set_online(online);
            self.put_node(&node)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

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
            let (_, value) = result?;
            let node: SovereignNode = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            nodes.push(node);
        }
        Ok(nodes)
    }

    pub fn get_connected_sovereign_nodes(&self) -> Result<Vec<SovereignNode>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SOVEREIGN_NODES)?;

        let mut nodes = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let node: SovereignNode = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            if node.is_connected {
                nodes.push(node);
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

    /// Store the permit we received FROM the node (our_permit - for reconnection)
    pub fn set_sovereign_node_permit(&self, node_id: &str, permit: String) -> Result<bool> {
        if let Some(mut node) = self.get_sovereign_node(node_id)? {
            node.set_our_permit(permit);
            self.put_sovereign_node(&node)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Store the permit we issued TO the node (permit_for_them)
    pub fn set_sovereign_node_permit_for_them(&self, node_id: &str, permit: String) -> Result<bool> {
        if let Some(mut node) = self.get_sovereign_node(node_id)? {
            node.set_permit_for_them(permit);
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

    /// Update the permit we issued TO the owner
    pub fn set_owner_permit_for_owner(&self, permit: String) -> Result<bool> {
        if let Some(mut owner) = self.get_owner()? {
            owner.set_permit_for_owner(permit);
            self.set_owner(&owner)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Update the permit we received FROM the owner
    pub fn set_owner_permit_from_owner(&self, permit: String) -> Result<bool> {
        if let Some(mut owner) = self.get_owner()? {
            owner.set_permit_from_owner(permit);
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
}
