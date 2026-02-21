//! Spaces API - Space CRUD and publishing operations

use crate::services::space_service;
use crate::{Butler, ButlerError, Result, Space, SpaceData};

/// Spaces API facade
///
/// Access via `butler.spaces()`
pub struct SpacesApi<'a> {
    pub(crate) butler: &'a Butler,
}

impl<'a> SpacesApi<'a> {
    /// Create a new space
    pub async fn create(
        &self,
        name: String,
        owner_did: String,
    ) -> Result<Space> {
        let signing_key = self.butler.signing_key().await?;
        space_service::create_space(self.butler.store(), name, owner_did, &signing_key).await
    }

    /// Get a space by ID
    pub fn get(&self, id: &str) -> Result<Option<Space>> {
        space_service::get_space(self.butler.store(), id)
    }

    /// Get space data (with permit and shares)
    pub fn get_data(&self, id: &str) -> Result<Option<SpaceData>> {
        space_service::get_space_with_shares(self.butler.store(), id)
    }

    /// List all spaces
    pub fn list(&self) -> Result<Vec<Space>> {
        space_service::list_all_spaces(self.butler.store())
    }

    /// List root-level spaces
    pub fn list_root(&self) -> Result<Vec<Space>> {
        space_service::list_root_spaces(self.butler.store())
    }

    /// Delete a space
    pub fn delete(&self, id: &str) -> Result<bool> {
        space_service::delete_space(self.butler.store(), id, true)
    }

    /// Share a space with a user (stores pubkey only)
    pub fn share(&self, id: &str, user_pubkey: String) -> Result<()> {
        space_service::share_space(self.butler.store(), id, user_pubkey)
    }

    /// Set permit for a space
    pub fn set_permit(&self, id: &str, permit: String) -> Result<()> {
        space_service::set_space_permit(self.butler.store(), id, permit)
    }

    /// Delegate space permit to node for publishing
    ///
    /// Creates a delegated permit with relationship="node" for the given node pubkey.
    pub async fn delegate_to_node(&self, id: &str, node_pubkey: &str) -> Result<String> {
        let signing_key = self.butler.signing_key().await?;

        // Get the space with permit data
        let space_data = self
            .butler
            .store()
            .get_space(id)?
            .ok_or_else(|| ButlerError::NotFound(format!("Space {} not found", id)))?;

        let space_permit = space_data
            .permit
            .ok_or_else(|| ButlerError::permit_error("Space has no permit"))?;

        // Delegate to node using the "node" template
        let (delegated, _cid) =
            gurkha::delegate_space(&signing_key, &space_permit, "node", node_pubkey)
                .await
                .map_err(|e| ButlerError::permit_error(format!("Failed to delegate: {:?}", e)))?;

        Ok(delegated)
    }

    /// Delegate space permit to viewer
    ///
    /// Creates a delegated permit with relationship="viewer" for the given viewer pubkey.
    pub async fn delegate_to_viewer(&self, id: &str, viewer_pubkey: &str) -> Result<String> {
        let signing_key = self.butler.signing_key().await?;

        // Get the space with permit data
        let space_data = self
            .butler
            .store()
            .get_space(id)?
            .ok_or_else(|| ButlerError::NotFound(format!("Space {} not found", id)))?;

        let space_permit = space_data
            .permit
            .ok_or_else(|| ButlerError::permit_error("Space has no permit"))?;

        // Delegate to viewer using the "viewer" template
        let (delegated, _cid) =
            gurkha::delegate_space(&signing_key, &space_permit, "viewer", viewer_pubkey)
                .await
                .map_err(|e| ButlerError::permit_error(format!("Failed to delegate: {:?}", e)))?;

        Ok(delegated)
    }

    /// Mark space as published on a specific node
    pub fn mark_published(&self, id: &str, node_id: &str) -> Result<()> {
        space_service::mark_space_published(self.butler.store(), id, node_id)
    }

    /// Store a node's permit for a space (Owner mode - proves space is published)
    ///
    /// **Context**: Owner receives permit from node after PublishSpaceAck.
    pub fn store_permit(&self, space_id: &str, node_did: &str, permit: &str) -> Result<()> {
        self.butler
            .store()
            .put_user_space_permit(space_id, node_did, permit)
    }

    /// Get a node's permit for a space (Owner mode - check if published)
    pub fn get_permit(&self, space_id: &str, node_did: &str) -> Result<Option<String>> {
        self.butler
            .store()
            .get_user_space_permit(space_id, node_did)
    }

    /// List all nodes that have issued permits for a space (= published nodes)
    pub fn get_nodes_with_permits(&self, space_id: &str) -> Result<Vec<String>> {
        self.butler.store().list_nodes_with_space_permits(space_id)
    }

    /// List page IDs for a space (for sync)
    pub fn list_page_ids(&self, space_id: &str) -> Result<Vec<String>> {
        use crate::services::page_service;
        let pages = page_service::list_pages(self.butler.store(), space_id)?;
        Ok(pages.into_iter().map(|p| p.id).collect())
    }
}
