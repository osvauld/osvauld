//! Permits API - Permit issuance and management

use crate::{Butler, Result};

/// Permits API facade
///
/// Access via `butler.permits()`
pub struct PermitsApi<'a> {
    pub(crate) butler: &'a Butler,
}

impl<'a> PermitsApi<'a> {

    /// Issue a one-time permit for node connection
    pub async fn issue_one_time(&self, role: &str) -> Result<(String, String)> {
        self.butler.issue_one_time_permit(role).await
    }

    /// Issue a space viewer permit
    pub async fn issue_space_viewer(&self, space_id: &str) -> Result<(String, String)> {
        self.butler.issue_space_viewer_permit(space_id).await
    }

    /// Issue a peer connection permit (for long-lived owner/node connections)
    pub async fn issue_peer_connection(&self, peer_pubkey: &str, relationship: &str) -> Result<(String, String)> {
        self.butler.issue_peer_connection_permit(peer_pubkey, relationship).await
    }

    /// Store a user's page permit (Node mode - for sync authorization)
    ///
    /// **Context**: Node receives owner's/viewer's permit during publish.
    pub fn store_page_permit(&self, page_id: &str, user_did: &str, permit: &str) -> Result<()> {
        self.butler.store().put_user_page_permit(page_id, user_did, permit)
    }

    /// Get a user's page permit (Node mode - for sync authorization)
    pub fn get_page_permit(&self, page_id: &str, user_did: &str) -> Result<Option<String>> {
        self.butler.store().get_user_page_permit(page_id, user_did)
    }

    /// Store a user's space permit (Owner mode - proves space is published)
    pub fn store_space_permit(&self, space_id: &str, node_did: &str, permit: &str) -> Result<()> {
        self.butler.store().put_user_space_permit(space_id, node_did, permit)
    }

    /// Get a user's space permit
    pub fn get_space_permit(&self, space_id: &str, node_did: &str) -> Result<Option<String>> {
        self.butler.store().get_user_space_permit(space_id, node_did)
    }

    /// Store a permit CID for a specific page and user.
    ///
    /// **Context**: Called when issuing a permit to track it for potential revocation.
    pub fn put_cid(&self, page_id: &str, user_did: &str, cid: &str) -> Result<()> {
        self.butler.store().put_permit_cid(page_id, user_did, cid)
    }

    /// Get a permit CID for a specific page and user.
    pub fn get_cid(&self, page_id: &str, user_did: &str) -> Result<Option<String>> {
        self.butler.store().get_permit_cid(page_id, user_did)
    }

    /// Delete a permit CID for a specific page and user.
    ///
    /// **Context**: Called when revoking access - marks the permit as revoked.
    pub fn delete_cid(&self, page_id: &str, user_did: &str) -> Result<bool> {
        self.butler.store().delete_permit_cid(page_id, user_did)
    }

    /// List all permit CIDs for a given page.
    ///
    /// Returns list of (user_did, cid) tuples.
    pub fn list_cids(&self, page_id: &str) -> Result<Vec<(String, String)>> {
        self.butler.store().list_permit_cids_for_page(page_id)
    }

    /// Delete all permit CIDs for a page.
    ///
    /// **Context**: Called when a page is deleted.
    pub fn delete_all_cids(&self, page_id: &str) -> Result<usize> {
        self.butler.store().delete_all_permit_cids_for_page(page_id)
    }

    /// Check if a permit CID exists for a page and user.
    pub fn has_cid(&self, page_id: &str, user_did: &str) -> Result<bool> {
        self.butler.store().has_permit_cid(page_id, user_did)
    }
}
