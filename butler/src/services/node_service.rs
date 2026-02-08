//! NodeService - Stateless functions for managing nodes
//!
//! All functions take store as first parameter - Butler injects this.

use crate::error::Result;
use crate::storage::RedbStore;
use crate::models::{OwnerInfo, SovereignNode, ConnectionString, ConnectionType, SovereignNodeExt};
use tracing::instrument;

/// Add a sovereign node from a connection string
#[instrument(skip(store), fields(connection_type = ?connection_type))]
pub fn add_sovereign_node(
    store: &RedbStore,
    connection_string: &str,
    connection_type: ConnectionType,
) -> std::result::Result<SovereignNode, String> {
    // Parse connection string
    let conn = ConnectionString::parse(connection_string)?;

    // Create and store sovereign node
    let node = SovereignNode::from_connection_string(&conn, connection_type);
    store.put_sovereign_node(&node)
        .map_err(|e| format!("Failed to store sovereign node: {}", e))?;

    Ok(node)
}

/// Get a sovereign node by its iroh NodeId
#[instrument(skip(store), fields(node_id = %node_id))]
pub fn get_sovereign_node(store: &RedbStore, node_id: &str) -> Result<Option<SovereignNode>> {
    store.get_sovereign_node(node_id)
}

/// List all sovereign nodes
#[instrument(skip_all)]
pub fn list_sovereign_nodes(store: &RedbStore) -> Result<Vec<SovereignNode>> {
    store.list_sovereign_nodes()
}

/// List connected sovereign nodes
#[instrument(skip_all)]
pub fn list_connected_sovereign_nodes(store: &RedbStore) -> Result<Vec<SovereignNode>> {
    store.get_connected_sovereign_nodes()
}

/// Update sovereign node connection status
#[instrument(skip(store), fields(node_id = %node_id, connected = %connected))]
pub fn set_sovereign_node_connected(store: &RedbStore, node_id: &str, connected: bool) -> Result<bool> {
    store.set_sovereign_node_connected(node_id, connected)
}

/// Store the permit for owner<->node relationship
#[instrument(skip(store, permit), fields(node_id = %node_id))]
pub fn set_sovereign_node_permit(store: &RedbStore, node_id: &str, permit: String) -> Result<bool> {
    store.set_sovereign_node_permit(node_id, permit)
}

/// Delete a sovereign node
#[instrument(skip(store), fields(node_id = %node_id))]
pub fn delete_sovereign_node(store: &RedbStore, node_id: &str) -> Result<bool> {
    store.delete_sovereign_node(node_id)
}

/// Get the owner of this node (Node side)
#[instrument(skip_all)]
pub fn get_owner(store: &RedbStore) -> Result<Option<OwnerInfo>> {
    store.get_owner()
}

/// Store owner info when owner first connects (Node side)
#[instrument(skip_all)]
pub fn set_owner(store: &RedbStore, owner: &OwnerInfo) -> Result<()> {
    store.set_owner(owner)
}

/// Check if this node has an owner
#[instrument(skip_all)]
pub fn has_owner(store: &RedbStore) -> Result<bool> {
    store.has_owner()
}

/// Store the permit for owner<->node relationship (Node side)
#[instrument(skip(store, permit))]
pub fn set_owner_permit(store: &RedbStore, permit: String) -> Result<bool> {
    store.set_owner_permit(permit)
}

/// Update owner's last connected timestamp (Node side)
#[instrument(skip_all)]
pub fn update_owner_last_connected(store: &RedbStore) -> Result<bool> {
    store.update_owner_last_connected()
}
