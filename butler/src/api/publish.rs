//! Publish API - Space and page publishing operations
//!
//! Groups all publish-related operations:
//! - Prepare spaces/pages for publishing
//! - Store received published content
//! - Track published state

use crate::services::{publish_service, space_service};
use crate::{Butler, Page, PageMeta, PreparedPage, Result, Space, SpaceMeta};

/// Publish API facade
///
/// Access via `butler.publish()`
pub struct PublishApi<'a> {
    pub(crate) butler: &'a Butler,
}

impl<'a> PublishApi<'a> {
    /// Prepare a space for publishing to a node
    ///
    /// **Context**: Owner wants to publish space to their node
    /// **Returns**: (SpaceMeta, delegated_permit) for transmission
    pub async fn prepare_space(
        &self,
        space_id: &str,
        node_pubkey: &str,
    ) -> Result<(SpaceMeta, String)> {
        let identity = self.butler.get_identity().await?;
        let owner_did = identity.did().to_string();
        publish_service::prepare_space_for_publish(
            self.butler.store(),
            space_id,
            &owner_did,
            node_pubkey,
            &identity.secret_signing_key(),
        )
        .await
    }

    /// Prepare a page for publishing to a node
    ///
    /// **Context**: Owner wants to publish page to their node
    /// **We do**:
    ///   1. Get page and owner's permit
    ///   2. Delegate permit to node
    ///   3. Filter out local_only layers
    ///   4. Encrypt layers for transit using ephemeral ECDH
    /// **Returns**: PreparedPage with transit-encrypted layers
    pub async fn prepare_page(
        &self,
        page_id: &str,
        node_pubkey: &str,
        node_enc_key: &[u8; 32],
    ) -> Result<PreparedPage> {
        let identity = self.butler.get_identity().await?;
        let owner_did = identity.did().to_string();
        publish_service::prepare_page_for_publish(
            self.butler.store(),
            page_id,
            &owner_did,
            node_pubkey,
            node_enc_key,
            &identity.secret_signing_key(),
            &identity.secret_encryption_key(),
        )
        .await
    }

    /// Prepare a page for a viewer (Node mode)
    ///
    /// **Context**: Node is responding to viewer's SpaceRequest
    /// **We do**:
    ///   1. Get page and node's permit
    ///   2. Delegate permit to viewer using "viewer" template
    ///   3. Filter out local_only layers
    ///   4. Encrypt layers for transit using ephemeral ECDH
    /// **Returns**: PreparedPage with transit-encrypted layers for viewer
    pub async fn prepare_page_for_viewer(
        &self,
        page_id: &str,
        viewer_pubkey: &str,
        viewer_enc_key: &[u8; 32],
    ) -> Result<PreparedPage> {
        let identity = self.butler.get_identity().await?;
        publish_service::prepare_page_for_viewer(
            self.butler.store(),
            page_id,
            viewer_pubkey,
            viewer_enc_key,
            &identity.secret_signing_key(),
            &identity.secret_encryption_key(),
        )
        .await
    }

    /// Store a space received via publish (Node mode)
    ///
    /// **Context**: Node receives PublishSpace from owner
    pub fn store_space(&self, space: &Space, permit: &str) -> Result<()> {
        space_service::store_published_space(self.butler.store(), space, permit)
    }

    /// Store a space with source tracking (Viewer mode)
    ///
    /// **Context**: Viewer receives SpaceData from node
    /// **source_node_id**: Stored so viewer knows which node to sync back to
    pub fn store_space_with_source(
        &self,
        space: &Space,
        permit: &str,
        source_node_id: Option<&str>,
    ) -> Result<()> {
        space_service::store_published_space_with_source(
            self.butler.store(),
            space,
            permit,
            source_node_id,
        )
    }

    /// Store a page received via publish (Node/Viewer mode)
    ///
    /// **Context**: Node received PublishPage from owner, or Viewer received PageData from node
    /// **We do**: Decrypt transit layers, re-encrypt with new AES key, store
    pub async fn store_page(
        &self,
        page_meta: PageMeta,
        permit: &str,
        ephemeral_public: &[u8; 32],
        transit_layers: Vec<(String, Vec<u8>)>,
        source_node_did: Option<&str>,
        sender_did: &str,
        sender_device_id: &str,
    ) -> Result<Page> {
        let identity = self.butler.get_identity().await?;
        publish_service::store_published_page(
            self.butler.store(),
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

    /// Mark a space as published to a node
    pub fn mark_space_published(&self, space_id: &str, node_id: &str) -> Result<()> {
        space_service::mark_space_published(self.butler.store(), space_id, node_id)
    }

    /// Mark a page as published to a node
    pub fn mark_page_published(&self, page_id: &str, node_id: &str) -> Result<()> {
        publish_service::mark_page_published(self.butler.store(), page_id, node_id)
    }

    /// Issue a space permit from node to owner
    ///
    /// **Context**: Node receives PublishSpace, issues permit back to owner
    pub async fn issue_space_permit_to_owner(
        &self,
        space_id: &str,
        owner_pubkey: &str,
    ) -> Result<String> {
        use crate::ButlerError;
        let signing_key = self.butler.signing_key().await?;

        let (permit, _cid) =
            gurkha::issue_space_node_to_owner(&signing_key, space_id, owner_pubkey)
                .await
                .map_err(|e| {
                    ButlerError::permit_error(format!("Failed to issue permit: {:?}", e))
                })?;

        Ok(permit)
    }

    /// Get nodes with space permits
    ///
    /// **Context**: UI needs to show which nodes a space is published to
    /// **Returns**: DIDs of nodes with permits for this space
    pub fn get_nodes_with_permits(&self, space_id: &str) -> Result<Vec<String>> {
        self.butler.store().list_nodes_with_space_permits(space_id)
    }
}
