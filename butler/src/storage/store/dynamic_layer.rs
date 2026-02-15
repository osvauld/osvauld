//! Dynamic Layer Storage Operations
//!
//! Three tables for the two-tier permit model:
//! - **Layer permits**: per-peer per-dynamic-layer UCAN tokens
//! - **Dynamic layer metadata**: tracks which layers are dynamic (for reconnect)
//! - **Viewer layer consents**: per-viewer per-layer consent tokens

use super::{
    RedbStore, DYNAMIC_LAYER_META, LAYER_AUTHORITY_PERMITS, LAYER_PERMITS, VIEWER_LAYER_CONSENTS,
};
use crate::error::Result;
use redb::{ReadableDatabase, ReadableTable};
use tracing::instrument;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LayerAuthorityRecord {
    version: u64,
    permit: String,
}

impl RedbStore {
    // ── Layer Permit Operations ──────────────────────────────────────────
    // Key: {page_id}/{user_did}/{layer_name} → permit token string
    // Multiple permits per peer per page (one per dynamic layer)

    /// Store a layer permit for a peer (additive).
    ///
    /// **Context**: Node detected new dynamic layer, issued permit for peer.
    #[instrument(skip_all)]
    pub fn store_layer_permit(
        &self,
        page_id: &str,
        user_did: &str,
        layer_name: &str,
        permit_token: &str,
    ) -> Result<()> {
        let key = format!("{}/{}/{}", page_id, user_did, layer_name);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LAYER_PERMITS)?;
            table.insert(key.as_str(), permit_token)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Load all layer permits for a peer on a page.
    ///
    /// **Context**: Used during reconnect to determine which dynamic layers peer has access to.
    /// **Returns**: Vec of (layer_name, permit_token) tuples.
    #[instrument(skip_all)]
    pub fn get_layer_permits(
        &self,
        page_id: &str,
        user_did: &str,
    ) -> Result<Vec<(String, String)>> {
        let prefix = format!("{}/{}/", page_id, user_did);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYER_PERMITS)?;

        let mut permits = Vec::new();
        for result in table.range(prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();

            if !key_str.starts_with(&prefix) {
                break;
            }

            if let Some(layer_name) = key_str.strip_prefix(&prefix) {
                permits.push((layer_name.to_string(), value.value().to_string()));
            }
        }
        Ok(permits)
    }

    /// Remove a layer permit for a peer.
    ///
    /// **Context**: Called during revocation or cleanup.
    #[instrument(skip_all)]
    pub fn remove_layer_permit(
        &self,
        page_id: &str,
        user_did: &str,
        layer_name: &str,
    ) -> Result<bool> {
        let key = format!("{}/{}/{}", page_id, user_did, layer_name);
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(LAYER_PERMITS)?;
            let result = table.remove(key.as_str())?.is_some();
            result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    /// Check if a peer has a layer permit for a specific dynamic layer.
    #[instrument(skip_all)]
    pub fn has_layer_permit(
        &self,
        page_id: &str,
        user_did: &str,
        layer_name: &str,
    ) -> Result<bool> {
        let key = format!("{}/{}/{}", page_id, user_did, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYER_PERMITS)?;
        let result = table.get(key.as_str())?.is_some();
        Ok(result)
    }

    // ── Dynamic Layer Metadata Operations ────────────────────────────────
    // Key: {page_id}/{layer_name} → meta JSON bytes
    // Tracks which layers are dynamic (schema, creator, grant type)

    /// Record that a layer is dynamic.
    ///
    /// **Context**: Node detected new dynamic layer from peer sync.
    /// Stores metadata for reconnect enumeration and permit recomputation.
    #[instrument(skip_all)]
    pub fn store_dynamic_layer_meta(
        &self,
        page_id: &str,
        layer_name: &str,
        meta_json: &[u8],
    ) -> Result<()> {
        let key = format!("{}/{}", page_id, layer_name);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(DYNAMIC_LAYER_META)?;
            table.insert(key.as_str(), meta_json)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// List all dynamic layers for a page.
    ///
    /// **Context**: Used during peer reconnect to enumerate dynamic layers
    /// and compute which permits are missing.
    /// **Returns**: Vec of (layer_name, meta_json_bytes) tuples.
    #[instrument(skip_all)]
    pub fn list_dynamic_layers(&self, page_id: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let prefix = format!("{}/", page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DYNAMIC_LAYER_META)?;

        let mut layers = Vec::new();
        for result in table.range(prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();

            if !key_str.starts_with(&prefix) {
                break;
            }

            if let Some(layer_name) = key_str.strip_prefix(&prefix) {
                layers.push((layer_name.to_string(), value.value().to_vec()));
            }
        }
        Ok(layers)
    }

    /// Get metadata for a specific dynamic layer.
    #[instrument(skip_all)]
    pub fn get_dynamic_layer_meta(
        &self,
        page_id: &str,
        layer_name: &str,
    ) -> Result<Option<Vec<u8>>> {
        let key = format!("{}/{}", page_id, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DYNAMIC_LAYER_META)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    // ── Viewer Layer Consent Operations ──────────────────────────────────
    // Key: {viewer_did}/{page_id}/{layer_name} → consent token string
    // Tracks per-layer consent from viewer (dynamic layers need separate consent)

    /// Store viewer's consent for a dynamic layer.
    ///
    /// **Context**: Node receives LayerConsentGrant from viewer.
    /// Required before node can sync that dynamic layer to the viewer.
    #[instrument(skip_all)]
    pub fn store_viewer_layer_consent(
        &self,
        viewer_did: &str,
        page_id: &str,
        layer_name: &str,
        consent_token: &str,
    ) -> Result<()> {
        let key = format!("{}/{}/{}", viewer_did, page_id, layer_name);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VIEWER_LAYER_CONSENTS)?;
            table.insert(key.as_str(), consent_token)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Check if viewer has consented to a dynamic layer.
    ///
    /// **Context**: Node checks before syncing a dynamic layer to a peer.
    /// Both layer permit AND consent must be present before sync.
    #[instrument(skip_all)]
    pub fn has_viewer_layer_consent(
        &self,
        viewer_did: &str,
        page_id: &str,
        layer_name: &str,
    ) -> Result<bool> {
        let key = format!("{}/{}/{}", viewer_did, page_id, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VIEWER_LAYER_CONSENTS)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    /// Get viewer's layer consent token.
    ///
    /// **Context**: Node needs consent token for proof chain during sync.
    #[instrument(skip_all)]
    pub fn get_viewer_layer_consent(
        &self,
        viewer_did: &str,
        page_id: &str,
        layer_name: &str,
    ) -> Result<Option<String>> {
        let key = format!("{}/{}/{}", viewer_did, page_id, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VIEWER_LAYER_CONSENTS)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_string())),
            None => Ok(None),
        }
    }

    // ── Layer Authority Permit Operations ─────────────────────────────────
    // Key: {page_id}/{audience}/{layer_name} -> JSON {version, permit}

    /// Store latest layer authority permit for page/layer/audience.
    ///
    /// Older or equal versions are ignored.
    #[instrument(skip_all)]
    pub fn store_layer_authority_permit(
        &self,
        page_id: &str,
        layer_name: &str,
        audience: &str,
        version: u64,
        permit_token: &str,
    ) -> Result<()> {
        let key = format!("{}/{}/{}", page_id, audience, layer_name);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LAYER_AUTHORITY_PERMITS)?;

            if let Some(existing) = table.get(key.as_str())? {
                if let Ok(record) = serde_json::from_slice::<LayerAuthorityRecord>(existing.value())
                {
                    if record.version >= version {
                        return Ok(());
                    }
                }
            }

            let record = LayerAuthorityRecord {
                version,
                permit: permit_token.to_string(),
            };
            let record_bytes = serde_json::to_vec(&record)
                .map_err(|e| crate::error::ButlerError::Serialization(e.to_string()))?;
            table.insert(key.as_str(), record_bytes.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get latest layer authority permit for page/layer/audience.
    #[instrument(skip_all)]
    pub fn get_layer_authority_permit(
        &self,
        page_id: &str,
        layer_name: &str,
        audience: &str,
    ) -> Result<Option<(u64, String)>> {
        let key = format!("{}/{}/{}", page_id, audience, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYER_AUTHORITY_PERMITS)?;

        let Some(guard) = table.get(key.as_str())? else {
            return Ok(None);
        };

        let record = serde_json::from_slice::<LayerAuthorityRecord>(guard.value())
            .map_err(|e| crate::error::ButlerError::Serialization(e.to_string()))?;
        Ok(Some((record.version, record.permit)))
    }

    /// List latest layer authority permits for an audience on a page.
    ///
    /// Returns (layer_name, version, permit_token).
    #[instrument(skip_all)]
    pub fn list_layer_authority_permits_for_audience(
        &self,
        page_id: &str,
        audience: &str,
    ) -> Result<Vec<(String, u64, String)>> {
        let prefix = format!("{}/{}/", page_id, audience);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYER_AUTHORITY_PERMITS)?;

        let mut permits = Vec::new();
        for result in table.range(prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();

            if !key_str.starts_with(&prefix) {
                break;
            }

            let Some(layer_name) = key_str.strip_prefix(&prefix) else {
                continue;
            };

            let record = serde_json::from_slice::<LayerAuthorityRecord>(value.value())
                .map_err(|e| crate::error::ButlerError::Serialization(e.to_string()))?;
            permits.push((layer_name.to_string(), record.version, record.permit));
        }

        Ok(permits)
    }
    /// Get stored authority for a layer regardless of audience (creator).
    ///
    /// **Context**: Node needs to find ANY stored authority for a given layer.
    /// Scans all authority permits for the page. Key format: `{page_id}/{audience}/{layer_name}`.
    /// **Returns**: (creator_did, version, authority_token) if found.
    #[instrument(skip_all)]
    pub fn get_authority_for_layer(
        &self,
        page_id: &str,
        layer_name: &str,
    ) -> Result<Option<(String, u64, String)>> {
        let page_prefix = format!("{}/", page_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYER_AUTHORITY_PERMITS)?;

        for result in table.range(page_prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();

            if !key_str.starts_with(&page_prefix) {
                break;
            }

            // Key format: {page_id}/{audience}/{layer_name}
            // We need to find entries where the suffix after {page_id}/{audience}/ matches layer_name
            let rest = &key_str[page_prefix.len()..];
            // rest = {audience}/{layer_name}
            // Find the first '/' after audience (audience is a DID, contains colons but no slashes until the layer_name part)
            // Actually DIDs don't contain '/' but layer_name can have '/'.
            // The audience is always a DID like "did:key:..." which doesn't contain '/'.
            // So the first '/' separates audience from layer_name.
            if let Some(slash_pos) = rest.find('/') {
                let audience = &rest[..slash_pos];
                let stored_layer = &rest[slash_pos + 1..];
                if stored_layer == layer_name {
                    let record = serde_json::from_slice::<LayerAuthorityRecord>(value.value())
                        .map_err(|e| crate::error::ButlerError::Serialization(e.to_string()))?;
                    return Ok(Some((audience.to_string(), record.version, record.permit)));
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> RedbStore {
        let temp_path = std::env::temp_dir().join(format!(
            "butler_dyn_layer_test_{}.redb",
            uuid::Uuid::new_v4()
        ));
        RedbStore::open(&temp_path).unwrap()
    }

    // ── C1: Layer permit CRUD ─────────────────────────────────────────

    #[test]
    fn test_store_and_retrieve_layer_permits() {
        let store = test_store();
        store
            .store_layer_permit("page1", "did:key:user", "page1/channels/foo", "token1")
            .unwrap();
        store
            .store_layer_permit("page1", "did:key:user", "page1/dms/bar", "token2")
            .unwrap();

        let permits = store.get_layer_permits("page1", "did:key:user").unwrap();
        assert_eq!(permits.len(), 2);

        // Sorted by key (lexicographic on layer_name)
        let names: Vec<&str> = permits.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"page1/channels/foo"));
        assert!(names.contains(&"page1/dms/bar"));
    }

    #[test]
    fn test_layer_permits_isolated_per_user() {
        let store = test_store();
        store
            .store_layer_permit("page1", "did:key:alice", "page1/ch/foo", "token-a")
            .unwrap();
        store
            .store_layer_permit("page1", "did:key:bob", "page1/ch/foo", "token-b")
            .unwrap();

        let alice = store.get_layer_permits("page1", "did:key:alice").unwrap();
        assert_eq!(alice.len(), 1);
        assert_eq!(alice[0].1, "token-a");

        let bob = store.get_layer_permits("page1", "did:key:bob").unwrap();
        assert_eq!(bob.len(), 1);
        assert_eq!(bob[0].1, "token-b");
    }

    // ── C2: Layer permit removal ──────────────────────────────────────

    #[test]
    fn test_remove_layer_permit() {
        let store = test_store();
        store
            .store_layer_permit("page1", "did:key:user", "page1/channels/foo", "token")
            .unwrap();
        assert!(store
            .has_layer_permit("page1", "did:key:user", "page1/channels/foo")
            .unwrap());

        let removed = store
            .remove_layer_permit("page1", "did:key:user", "page1/channels/foo")
            .unwrap();
        assert!(removed);

        let permits = store.get_layer_permits("page1", "did:key:user").unwrap();
        assert!(permits.is_empty());

        // Removing non-existent returns false
        let removed2 = store
            .remove_layer_permit("page1", "did:key:user", "page1/channels/foo")
            .unwrap();
        assert!(!removed2);
    }

    // ── C3: Dynamic layer metadata ────────────────────────────────────

    #[test]
    fn test_dynamic_layer_metadata_storage() {
        let store = test_store();
        let meta = serde_json::json!({
            "schema_pattern": "channels/{id}/messages",
            "creator_did": "did:key:alice",
            "grant_type": "role",
        });
        let meta_bytes = serde_json::to_vec(&meta).unwrap();

        store
            .store_dynamic_layer_meta(
                "page1",
                "page1/channels/did:key:alice/general/messages",
                &meta_bytes,
            )
            .unwrap();

        let layers = store.list_dynamic_layers("page1").unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].0, "page1/channels/did:key:alice/general/messages");

        let retrieved: serde_json::Value = serde_json::from_slice(&layers[0].1).unwrap();
        assert_eq!(retrieved["creator_did"], "did:key:alice");
        assert_eq!(retrieved["grant_type"], "role");
    }

    #[test]
    fn test_get_dynamic_layer_meta() {
        let store = test_store();
        let meta = b"{\"schema\":\"orders/{id}\"}";
        store
            .store_dynamic_layer_meta("page1", "page1/orders/did:key:a/uuid1", meta)
            .unwrap();

        let result = store
            .get_dynamic_layer_meta("page1", "page1/orders/did:key:a/uuid1")
            .unwrap();
        assert!(result.is_some());

        let missing = store
            .get_dynamic_layer_meta("page1", "page1/nonexistent")
            .unwrap();
        assert!(missing.is_none());
    }

    // ── C4: Viewer layer consent ──────────────────────────────────────

    #[test]
    fn test_viewer_layer_consent_storage() {
        let store = test_store();
        store
            .store_viewer_layer_consent(
                "did:key:viewer",
                "page1",
                "page1/channels/foo",
                "consent-token-1",
            )
            .unwrap();

        assert!(store
            .has_viewer_layer_consent("did:key:viewer", "page1", "page1/channels/foo")
            .unwrap());
        assert!(!store
            .has_viewer_layer_consent("did:key:viewer", "page1", "page1/channels/bar")
            .unwrap());
        assert!(!store
            .has_viewer_layer_consent("did:key:other", "page1", "page1/channels/foo")
            .unwrap());

        let token = store
            .get_viewer_layer_consent("did:key:viewer", "page1", "page1/channels/foo")
            .unwrap();
        assert_eq!(token, Some("consent-token-1".to_string()));

        let missing = store
            .get_viewer_layer_consent("did:key:viewer", "page1", "page1/channels/bar")
            .unwrap();
        assert!(missing.is_none());
    }
}
