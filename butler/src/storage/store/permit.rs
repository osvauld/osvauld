//! Permit and Subscription table operations
//!
//! Includes:
//! - Permit CID Operations (for revocation tracking)
//! - Space Subscription Operations (user access to spaces)
//! - Viewer Consent Operations (viewer-issued permits stored by node)
//! - User Page Permits (for sync authorization)
//! - User Space Permits (node permits for spaces)

use super::{
    RedbStore, CONNECTION_PERMITS, PERMIT_CIDS, SPACE_SUBSCRIPTIONS, USER_PAGE_PERMITS,
    USER_SPACE_PERMITS, VIEWER_CONSENT_PAGE, VIEWER_CONSENT_SPACE,
};
use crate::error::Result;
use redb::ReadableDatabase;
use tracing::instrument;

impl RedbStore {
    // Permit CID Operations (for revocation tracking)
    // Key: {page_id}/{user_did} → cid string (content-addressed permit hash)
    // Used to track issued permits for revocation

    /// Store a permit CID for a specific page and user.
    ///
    /// **Context**: Called when issuing a permit to track it for potential revocation.
    #[instrument(skip_all)]
    pub fn put_permit_cid(&self, page_id: &str, user_did: &str, cid: &str) -> Result<()> {
        let key = Self::key2(page_id, user_did);
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
    #[instrument(skip_all)]
    pub fn get_permit_cid(&self, page_id: &str, user_did: &str) -> Result<Option<String>> {
        let key = Self::key2(page_id, user_did);
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
    #[instrument(skip_all)]
    pub fn delete_permit_cid(&self, page_id: &str, user_did: &str) -> Result<bool> {
        let key = Self::key2(page_id, user_did);
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
    #[instrument(skip_all)]
    pub fn list_permit_cids_for_page(&self, page_id: &str) -> Result<Vec<(String, String)>> {
        let prefix = Self::prefix1(page_id);
        self.scan_prefix_str(PERMIT_CIDS, prefix.as_str())
    }

    /// List all user_dids authorized to access a page.
    ///
    /// **Context**: Called by Scribe in node mode to get sync targets.
    /// **We query**: PERMIT_CIDS table for this page.
    /// **We return**: List of user_dids (Coordinator handles device resolution).
    #[instrument(skip_all)]
    pub fn list_authorized_users_for_page(&self, page_id: &str) -> Result<Vec<String>> {
        let prefix = Self::prefix1(page_id);
        Ok(self
            .scan_prefix_str(PERMIT_CIDS, prefix.as_str())?
            .into_iter()
            .map(|(user_did, _)| user_did)
            .collect())
    }

    /// Delete all permit CIDs for a page.
    ///
    /// **Context**: Called when a page is deleted.
    #[instrument(skip_all)]
    pub fn delete_all_permit_cids_for_page(&self, page_id: &str) -> Result<usize> {
        let prefix = Self::prefix1(page_id);
        let keys_to_delete: Vec<String> = self
            .scan_prefix_str(PERMIT_CIDS, prefix.as_str())?
            .into_iter()
            .map(|(user_did, _)| Self::key2(page_id, user_did.as_str()))
            .collect();

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
    #[instrument(skip_all)]
    pub fn has_permit_cid(&self, page_id: &str, user_did: &str) -> Result<bool> {
        let key = Self::key2(page_id, user_did);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PERMIT_CIDS)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    // Space Subscription Operations (user access to spaces)
    // Key: {space_id}/{user_did} → SubscriptionData (serialized)
    // Tracks which users have access to which spaces

    /// Store a space subscription.
    ///
    /// **Context**: Called when granting a user access to a space.
    #[instrument(skip_all)]
    pub fn put_space_subscription(
        &self,
        space_id: &str,
        user_did: &str,
        data: &[u8],
    ) -> Result<()> {
        let key = Self::key2(space_id, user_did);
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
    #[instrument(skip_all)]
    pub fn get_space_subscription(
        &self,
        space_id: &str,
        user_did: &str,
    ) -> Result<Option<Vec<u8>>> {
        let key = Self::key2(space_id, user_did);
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
    #[instrument(skip_all)]
    pub fn delete_space_subscription(&self, space_id: &str, user_did: &str) -> Result<bool> {
        let key = Self::key2(space_id, user_did);
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
    #[instrument(skip_all)]
    pub fn list_space_subscribers(&self, space_id: &str) -> Result<Vec<String>> {
        let prefix = Self::prefix1(space_id);
        Ok(self
            .scan_prefix_bytes(SPACE_SUBSCRIPTIONS, prefix.as_str())?
            .into_iter()
            .map(|(user_did, _)| user_did)
            .collect())
    }

    /// Check if a user is subscribed to a space.
    ///
    /// **Context**: Quick check for space access.
    #[instrument(skip_all)]
    pub fn has_space_subscription(&self, space_id: &str, user_did: &str) -> Result<bool> {
        let key = Self::key2(space_id, user_did);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPACE_SUBSCRIPTIONS)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    /// Delete all subscriptions for a space.
    ///
    /// **Context**: Called when a space is deleted.
    #[instrument(skip_all)]
    pub fn delete_all_space_subscriptions(&self, space_id: &str) -> Result<usize> {
        let prefix = Self::prefix1(space_id);
        let keys_to_delete: Vec<String> = self
            .scan_prefix_bytes(SPACE_SUBSCRIPTIONS, prefix.as_str())?
            .into_iter()
            .map(|(user_did, _)| Self::key2(space_id, user_did.as_str()))
            .collect();

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

    // Viewer Consent Operations (viewer-issued permits stored by node)
    // Key: {viewer_did}/{resource_id} → consent permit string
    // Tracks viewer consent for receiving sync updates

    /// Store a viewer's space consent permit.
    ///
    /// **Context**: Node receives SyncConsentGrant from viewer.
    /// The permit is viewer-issued, expressing consent for sync updates.
    #[instrument(skip_all)]
    pub fn put_viewer_space_consent(
        &self,
        viewer_did: &str,
        space_id: &str,
        consent_permit: &str,
    ) -> Result<()> {
        let key = Self::key2(viewer_did, space_id);
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
    #[instrument(skip_all)]
    pub fn get_viewer_space_consent(
        &self,
        viewer_did: &str,
        space_id: &str,
    ) -> Result<Option<String>> {
        let key = Self::key2(viewer_did, space_id);
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
    #[instrument(skip_all)]
    pub fn put_viewer_page_consent(
        &self,
        viewer_did: &str,
        page_id: &str,
        consent_permit: &str,
    ) -> Result<()> {
        let key = Self::key2(viewer_did, page_id);
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
    #[instrument(skip_all)]
    pub fn get_viewer_page_consent(
        &self,
        viewer_did: &str,
        page_id: &str,
    ) -> Result<Option<String>> {
        let key = Self::key2(viewer_did, page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VIEWER_CONSENT_PAGE)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_string())),
            None => Ok(None),
        }
    }

    /// Check if viewer has space consent.
    #[instrument(skip_all)]
    pub fn has_viewer_space_consent(&self, viewer_did: &str, space_id: &str) -> Result<bool> {
        let key = Self::key2(viewer_did, space_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VIEWER_CONSENT_SPACE)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    /// Check if viewer has page consent.
    #[instrument(skip_all)]
    pub fn has_viewer_page_consent(&self, viewer_did: &str, page_id: &str) -> Result<bool> {
        let key = Self::key2(viewer_did, page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VIEWER_CONSENT_PAGE)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    // User Page Permits (for sync authorization - Node stores permits)
    // Key: {page_id}/{user_did} → permit string
    // Used for permit-based sync auth (replaces DID whitelist)

    /// Store a user's permit for a page (Node mode - for sync authorization).
    ///
    /// **Context**: Node receives owner's permit during PublishPage.
    /// This permit is used for sync authorization (layer permissions).
    #[instrument(skip_all)]
    pub fn put_user_page_permit(&self, page_id: &str, user_did: &str, permit: &str) -> Result<()> {
        let key = Self::key2(page_id, user_did);
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
    #[instrument(skip_all)]
    pub fn get_user_page_permit(&self, page_id: &str, user_did: &str) -> Result<Option<String>> {
        let key = Self::key2(page_id, user_did);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(USER_PAGE_PERMITS)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_string())),
            None => Ok(None),
        }
    }

    /// List all user page permits for a given page.
    ///
    /// **Context**: Node needs to reissue permits when new app layers are added.
    /// **Returns**: Vec of (user_did, permit_token) tuples.
    #[instrument(skip_all)]
    pub fn list_user_page_permits(&self, page_id: &str) -> Result<Vec<(String, String)>> {
        let prefix = Self::prefix1(page_id);
        self.scan_prefix_str(USER_PAGE_PERMITS, prefix.as_str())
    }

    // User Space Permits
    // Key: {space_id}/{node_did} → permit string

    /// Store a node's permit for a space (Owner mode - proves space is published)
    ///
    /// **Context**: Owner receives permit from node after PublishSpaceAck.
    /// This permit proves the space is published to that node.
    #[instrument(skip_all)]
    pub fn put_user_space_permit(
        &self,
        space_id: &str,
        node_did: &str,
        permit: &str,
    ) -> Result<()> {
        let key = Self::key2(space_id, node_did);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(USER_SPACE_PERMITS)?;
            table.insert(key.as_str(), permit)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a node's permit for a space (Owner mode - check if published)
    #[instrument(skip_all)]
    pub fn get_user_space_permit(&self, space_id: &str, node_did: &str) -> Result<Option<String>> {
        let key = Self::key2(space_id, node_did);
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
    #[instrument(skip_all)]
    pub fn list_nodes_with_space_permits(&self, space_id: &str) -> Result<Vec<String>> {
        let prefix = Self::prefix1(space_id);
        Ok(self
            .scan_prefix_str(USER_SPACE_PERMITS, prefix.as_str())?
            .into_iter()
            .map(|(node_did, _)| node_did)
            .collect())
    }

    // Connection Permits (for node → viewer reconnection)
    // Key: user_did → permit (issued by viewer during PermitGrant)
    // Used by node to reconnect to viewer devices

    /// Store a connection permit for a user (Node mode).
    ///
    /// **Context**: Node receives PermitGrant from viewer.
    /// This permit is viewer-issued, allowing node to reconnect to the viewer.
    #[instrument(skip_all)]
    pub fn put_connection_permit(&self, user_did: &str, permit: &str) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CONNECTION_PERMITS)?;
            table.insert(user_did, permit)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a connection permit for a user (Node mode).
    ///
    /// **Context**: Node needs permit to reconnect to viewer.
    #[instrument(skip_all)]
    pub fn get_connection_permit(&self, user_did: &str) -> Result<Option<String>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CONNECTION_PERMITS)?;

        match table.get(user_did)? {
            Some(guard) => Ok(Some(guard.value().to_string())),
            None => Ok(None),
        }
    }
}
