//! Nodes API - Sovereign node and owner operations

use crate::models::ConnectionType;
use crate::services::node_service;
use crate::{Butler, ConnectionString, OwnerInfo, Result, SovereignNode};

/// Nodes API facade
///
/// Access via `butler.nodes()`
pub struct NodesApi<'a> {
    pub(crate) butler: &'a Butler,
}

impl<'a> NodesApi<'a> {
    /// Add a sovereign node from connection string
    pub fn add(&self, connection_string: &str) -> std::result::Result<SovereignNode, String> {
        node_service::add_sovereign_node(
            self.butler.store(),
            connection_string,
            ConnectionType::Owner,
        )
    }

    /// Parse a connection string without storing (for viewer connections)
    pub fn parse_connection_string(
        &self,
        connection_string: &str,
    ) -> std::result::Result<ConnectionString, String> {
        ConnectionString::parse(connection_string)
    }

    /// Get a sovereign node by ID
    pub fn get(&self, id: &str) -> Result<Option<SovereignNode>> {
        node_service::get_sovereign_node(self.butler.store(), id)
    }

    /// List all sovereign nodes
    pub fn list(&self) -> Result<Vec<SovereignNode>> {
        node_service::list_sovereign_nodes(self.butler.store())
    }

    /// List connected sovereign nodes
    pub fn list_connected(&self) -> Result<Vec<SovereignNode>> {
        node_service::list_connected_sovereign_nodes(self.butler.store())
    }

    /// Set node connection status
    pub fn set_connected(&self, id: &str, connected: bool) -> Result<bool> {
        node_service::set_sovereign_node_connected(self.butler.store(), id, connected)
    }

    /// Set node permit
    pub fn set_permit(&self, id: &str, permit: String) -> Result<bool> {
        node_service::set_sovereign_node_permit(self.butler.store(), id, permit)
    }

    /// Delete a sovereign node
    pub fn delete(&self, id: &str) -> Result<bool> {
        node_service::delete_sovereign_node(self.butler.store(), id)
    }

    /// Update node relay URL
    pub fn update_relay(&self, id: &str, relay_url: Option<String>) -> Result<bool> {
        self.butler
            .store()
            .update_sovereign_node_relay(id, relay_url)
    }

    /// Get sovereign node for owner sync
    ///
    /// Returns the first connected sovereign node's DID, if any.
    pub fn get_for_sync(&self) -> Result<Option<String>> {
        let nodes = self.list_connected()?;
        Ok(nodes.into_iter().next().map(|n| n.did))
    }

    /// Get the owner (node mode)
    pub fn get_owner(&self) -> Result<Option<OwnerInfo>> {
        node_service::get_owner(self.butler.store())
    }

    /// Set the owner (node mode)
    pub fn set_owner(&self, owner: &OwnerInfo) -> Result<()> {
        node_service::set_owner(self.butler.store(), owner)
    }

    /// Check if owner is set
    pub fn has_owner(&self) -> Result<bool> {
        node_service::has_owner(self.butler.store())
    }

    /// Set owner permit
    pub fn set_owner_permit(&self, permit: String) -> Result<bool> {
        node_service::set_owner_permit(self.butler.store(), permit)
    }

    /// Update owner last connected timestamp
    pub fn update_owner_last_connected(&self) -> Result<bool> {
        node_service::update_owner_last_connected(self.butler.store())
    }

    /// Generate a connection string for this node (Node mode)
    ///
    /// **Context**: Node generates connection string for owner to scan/enter
    /// **Returns**: Base64-encoded JSON containing keys, permit, etc.
    pub async fn generate_connection_string(&self, relay_url: Option<&str>) -> Result<String> {
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let identity = self.butler.get_identity().await?;
        let user_info = self.butler.user_info().await?;

        // Issue one-time permit for owner connection
        let (permit, _pub_key) = self.butler.issue_one_time_permit("node_owner").await?;

        // Encode keys as base64
        let user_pub_key = STANDARD.encode(identity.public_signing_key());
        let device_pub_key = STANDARD.encode(identity.public_device_key());
        let encryption_pub_key = STANDARD.encode(identity.public_encryption_key());

        // Build connection string JSON
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
    /// **Returns**: Base64-encoded JSON with keys and viewer permit
    pub async fn generate_viewer_connection_string(
        &self,
        space_id: &str,
        relay_url: Option<&str>,
    ) -> Result<String> {
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let identity = self.butler.get_identity().await?;
        let user_info = self.butler.user_info().await?;

        // Generate viewer permit for this space
        let (permit, _cid) = self.butler.issue_space_viewer_permit(space_id).await?;

        // Encode keys as base64
        let user_pub_key = STANDARD.encode(identity.public_signing_key());
        let device_pub_key = STANDARD.encode(identity.public_device_key());
        let encryption_pub_key = STANDARD.encode(identity.public_encryption_key());

        // Build connection string JSON
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
}
