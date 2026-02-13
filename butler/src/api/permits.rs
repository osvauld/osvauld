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

    /// List all user page permits for a page.
    ///
    /// **Returns**: Vec of (user_did, permit_token) tuples.
    /// **Context**: Node needs to reissue permits when new app layers are added.
    pub fn list_page_permits(&self, page_id: &str) -> Result<Vec<(String, String)>> {
        self.butler.store().list_user_page_permits(page_id)
    }

    /// Reissue all page permits with new layers added.
    ///
    /// **Context**: New app installed on page — adds layers to all peers' page permits.
    /// **We do**: For each peer, reissue their permit with merged layers, store updated permit.
    /// **Returns**: Vec of (user_did, new_permit_token) for distribution.
    pub async fn reissue_all_page_permits(
        &self,
        page_id: &str,
        new_layers: std::collections::HashMap<String, gurkha::LayerConfig>,
    ) -> Result<Vec<(String, String)>> {
        let signing_key = self.butler.signing_key().await?;
        let existing_permits = self.butler.store().list_user_page_permits(page_id)?;

        let mut reissued = Vec::new();
        for (user_did, old_token) in &existing_permits {
            match gurkha::reissue_permit_with_layers(
                &signing_key,
                old_token,
                user_did,
                new_layers.clone(),
            ).await {
                Ok((new_token, _cid)) => {
                    // Store updated permit
                    self.butler.store().put_user_page_permit(page_id, user_did, &new_token)?;
                    reissued.push((user_did.clone(), new_token));
                }
                Err(e) => {
                    tracing::warn!(
                        user_did = %user_did,
                        error = %e,
                        "Failed to reissue page permit"
                    );
                }
            }
        }

        Ok(reissued)
    }

    // ── Layer Permits (dynamic layers — two-tier model) ────────────────

    /// Store a layer permit for a peer (additive).
    ///
    /// **Context**: Node detected new dynamic layer, issued permit for peer.
    pub fn store_layer_permit(
        &self,
        page_id: &str,
        user_did: &str,
        layer_name: &str,
        permit_token: &str,
    ) -> Result<()> {
        self.butler.store().store_layer_permit(page_id, user_did, layer_name, permit_token)
    }

    /// Load all layer permits for a peer on a page.
    ///
    /// **Returns**: Vec of (layer_name, permit_token) tuples.
    pub fn get_layer_permits(
        &self,
        page_id: &str,
        user_did: &str,
    ) -> Result<Vec<(String, String)>> {
        self.butler.store().get_layer_permits(page_id, user_did)
    }

    /// Remove a layer permit for a peer.
    pub fn remove_layer_permit(
        &self,
        page_id: &str,
        user_did: &str,
        layer_name: &str,
    ) -> Result<bool> {
        self.butler.store().remove_layer_permit(page_id, user_did, layer_name)
    }

    /// Check if a peer has a layer permit for a specific dynamic layer.
    pub fn has_layer_permit(
        &self,
        page_id: &str,
        user_did: &str,
        layer_name: &str,
    ) -> Result<bool> {
        self.butler.store().has_layer_permit(page_id, user_did, layer_name)
    }

    // ── Dynamic Layer Metadata ─────────────────────────────────────────

    /// Record that a layer is dynamic (for reconnect enumeration).
    pub fn store_dynamic_layer_meta(
        &self,
        page_id: &str,
        layer_name: &str,
        meta_json: &[u8],
    ) -> Result<()> {
        self.butler.store().store_dynamic_layer_meta(page_id, layer_name, meta_json)
    }

    /// List all dynamic layers for a page.
    ///
    /// **Returns**: Vec of (layer_name, meta_json_bytes) tuples.
    pub fn list_dynamic_layers(&self, page_id: &str) -> Result<Vec<(String, Vec<u8>)>> {
        self.butler.store().list_dynamic_layers(page_id)
    }

    /// Get metadata for a specific dynamic layer.
    pub fn get_dynamic_layer_meta(
        &self,
        page_id: &str,
        layer_name: &str,
    ) -> Result<Option<Vec<u8>>> {
        self.butler.store().get_dynamic_layer_meta(page_id, layer_name)
    }

    // ── Viewer Layer Consent ───────────────────────────────────────────

    /// Store viewer's consent for a dynamic layer.
    ///
    /// **Context**: Node receives LayerConsentGrant from viewer.
    pub fn store_viewer_layer_consent(
        &self,
        viewer_did: &str,
        page_id: &str,
        layer_name: &str,
        consent_token: &str,
    ) -> Result<()> {
        self.butler.store().store_viewer_layer_consent(viewer_did, page_id, layer_name, consent_token)
    }

    /// Check if viewer has consented to a dynamic layer.
    pub fn has_viewer_layer_consent(
        &self,
        viewer_did: &str,
        page_id: &str,
        layer_name: &str,
    ) -> Result<bool> {
        self.butler.store().has_viewer_layer_consent(viewer_did, page_id, layer_name)
    }

    /// Get viewer's layer consent token.
    pub fn get_viewer_layer_consent(
        &self,
        viewer_did: &str,
        page_id: &str,
        layer_name: &str,
    ) -> Result<Option<String>> {
        self.butler.store().get_viewer_layer_consent(viewer_did, page_id, layer_name)
    }

    // ── Layer Authority Permits (creator -> node) ───────────────────────

    /// Store a layer authority permit for page/layer/audience.
    ///
    /// **Context**: Node receives creator consent grant scoped to a specific layer.
    /// Newer versions supersede older versions.
    pub fn store_layer_authority_permit(
        &self,
        page_id: &str,
        layer_name: &str,
        audience: &str,
        version: u64,
        permit_token: &str,
    ) -> Result<()> {
        self.butler.store().store_layer_authority_permit(
            page_id,
            layer_name,
            audience,
            version,
            permit_token,
        )
    }

    /// Get latest layer authority permit for page/layer/audience.
    pub fn get_layer_authority_permit(
        &self,
        page_id: &str,
        layer_name: &str,
        audience: &str,
    ) -> Result<Option<(u64, String)>> {
        self.butler
            .store()
            .get_layer_authority_permit(page_id, layer_name, audience)
    }

    /// List latest layer authority permits for an audience on a page.
    ///
    /// **Returns**: Vec of (layer_name, version, permit_token).
    pub fn list_layer_authority_permits_for_audience(
        &self,
        page_id: &str,
        audience: &str,
    ) -> Result<Vec<(String, u64, String)>> {
        self.butler
            .store()
            .list_layer_authority_permits_for_audience(page_id, audience)
    }
}
