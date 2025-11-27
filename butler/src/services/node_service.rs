//! NodeService - Service for managing nodes (my node and external sovereign nodes)

use crate::error::Result;
use crate::storage::RedbStore;
use crate::models::{NodeInfo, OwnerInfo, SovereignNode, ConnectionString};
use std::sync::Arc;

/// NodeService - Manages nodes (my node and external sovereign nodes)
pub struct NodeService {
    store: Arc<RedbStore>,
}

impl NodeService {
    pub fn new(store: Arc<RedbStore>) -> Self {
        Self { store }
    }

    // ==================== MY NODE ====================

    /// Register my sovereign node
    pub fn register_my_node(
        &self,
        user_did: String,
        node_did: String,
        device_id: String,
        iroh_node_id: String,
        node_addr: Option<String>,
    ) -> Result<NodeInfo> {
        let mut node = NodeInfo::new(user_did, node_did, device_id, iroh_node_id);
        if let Some(addr) = node_addr {
            node = node.with_node_addr(addr);
        }
        node.set_online(true);

        self.store.put_node(&node)?;
        Ok(node)
    }

    /// Get my node info
    pub fn get_my_node(&self, my_did: &str) -> Result<Option<NodeInfo>> {
        self.store.get_node(my_did)
    }

    // ==================== SOVEREIGN NODES (external) ====================

    /// Add a sovereign node from a connection string
    ///
    /// Parses the connection string and stores the sovereign node info.
    /// Returns the SovereignNode for use in establishing connection.
    pub fn add_sovereign_node(&self, connection_string: &str) -> std::result::Result<SovereignNode, String> {
        // Parse connection string
        let conn = ConnectionString::parse(connection_string)?;

        // Create and store sovereign node
        let node = SovereignNode::from_connection_string(&conn);
        self.store.put_sovereign_node(&node)
            .map_err(|e| format!("Failed to store sovereign node: {}", e))?;

        Ok(node)
    }

    /// Get a sovereign node by its iroh NodeId
    pub fn get_sovereign_node(&self, node_id: &str) -> Result<Option<SovereignNode>> {
        self.store.get_sovereign_node(node_id)
    }

    /// List all sovereign nodes
    pub fn list_sovereign_nodes(&self) -> Result<Vec<SovereignNode>> {
        self.store.list_sovereign_nodes()
    }

    /// List connected sovereign nodes
    pub fn list_connected_sovereign_nodes(&self) -> Result<Vec<SovereignNode>> {
        self.store.get_connected_sovereign_nodes()
    }

    /// Update sovereign node connection status
    pub fn set_sovereign_node_connected(&self, node_id: &str, connected: bool) -> Result<bool> {
        self.store.set_sovereign_node_connected(node_id, connected)
    }

    /// Store the long-lived permit we received from sovereign node (our_permit)
    pub fn set_sovereign_node_permit(&self, node_id: &str, permit: String) -> Result<bool> {
        self.store.set_sovereign_node_permit(node_id, permit)
    }

    /// Store the permit we issued TO sovereign node (permit_for_them)
    pub fn set_sovereign_node_permit_for_them(&self, node_id: &str, permit: String) -> Result<bool> {
        self.store.set_sovereign_node_permit_for_them(node_id, permit)
    }

    /// Delete a sovereign node
    pub fn delete_sovereign_node(&self, node_id: &str) -> Result<bool> {
        self.store.delete_sovereign_node(node_id)
    }

    // ==================== OWNER (Node side - info about owner) ====================

    /// Get the owner of this node (Node side)
    pub fn get_owner(&self) -> Result<Option<OwnerInfo>> {
        self.store.get_owner()
    }

    /// Store owner info when owner first connects (Node side)
    pub fn set_owner(&self, owner: &OwnerInfo) -> Result<()> {
        self.store.set_owner(owner)
    }

    /// Check if this node has an owner
    pub fn has_owner(&self) -> Result<bool> {
        self.store.has_owner()
    }

    /// Store the permit we issued TO the owner (Node side)
    pub fn set_owner_permit_for_owner(&self, permit: String) -> Result<bool> {
        self.store.set_owner_permit_for_owner(permit)
    }

    /// Store the permit we received FROM the owner (Node side)
    pub fn set_owner_permit_from_owner(&self, permit: String) -> Result<bool> {
        self.store.set_owner_permit_from_owner(permit)
    }

    /// Update owner's last connected timestamp (Node side)
    pub fn update_owner_last_connected(&self) -> Result<bool> {
        self.store.update_owner_last_connected()
    }
}
