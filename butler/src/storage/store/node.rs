//! Sovereign Node and Owner Info table operations

use redb::{ReadableTable, ReadableDatabase};
use crate::error::{ButlerError, Result};
use crate::models::{SovereignNode, OwnerInfo};
use super::{RedbStore, SOVEREIGN_NODES, OWNER_INFO};

impl RedbStore {
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
            let result = table.remove(node_id)?;
            result.is_some()
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
                    // Skip records that fail to deserialize (likely schema mismatch)
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
}
