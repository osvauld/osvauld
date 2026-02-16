//! RedbStore - Main persistent storage for Osvauld using redb
//!
//! Split into logical submodules by table type.

mod contact;
mod dynamic_layer;
mod identity;
mod layer;
mod node;
mod page;
mod permit;
mod space;
mod sync;

use crate::error::Result;
use redb::{Database, ReadableDatabase, TableDefinition};
use std::path::Path;
use std::sync::Arc;
use tracing::instrument;

// Core tables
pub(crate) const IDENTITY: TableDefinition<&str, &[u8]> = TableDefinition::new("identity");
pub(crate) const SPACES: TableDefinition<&str, &[u8]> = TableDefinition::new("spaces");
pub(crate) const PAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("pages");
pub(crate) const LAYERS: TableDefinition<&str, &[u8]> = TableDefinition::new("layers");

// Index tables
pub(crate) const SPACE_PAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("space_pages");

// Access control tables
pub(crate) const CONTACTS: TableDefinition<&str, &[u8]> = TableDefinition::new("contacts");
pub(crate) const SPACE_SUBSCRIPTIONS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("space_subscriptions");
pub(crate) const PERMIT_CIDS: TableDefinition<&str, &str> = TableDefinition::new("permit_cids");

// Sync tables
pub(crate) const VECTORS: TableDefinition<&str, &[u8]> = TableDefinition::new("vectors");

// Node tables
pub(crate) const SOVEREIGN_NODES: TableDefinition<&str, &[u8]> =
    TableDefinition::new("sovereign_nodes");
pub(crate) const OWNER_INFO: TableDefinition<&str, &[u8]> = TableDefinition::new("owner_info");

// Viewer consent tables
pub(crate) const VIEWER_CONSENT_SPACE: TableDefinition<&str, &str> =
    TableDefinition::new("viewer_consent_space");
pub(crate) const VIEWER_CONSENT_PAGE: TableDefinition<&str, &str> =
    TableDefinition::new("viewer_consent_page");

// User permits tables
pub(crate) const USER_PAGE_PERMITS: TableDefinition<&str, &str> =
    TableDefinition::new("user_page_permits");
pub(crate) const USER_SPACE_PERMITS: TableDefinition<&str, &str> =
    TableDefinition::new("user_space_permits");

// Connection permits (for node → viewer reconnection)
// Key: user_did → permit (issued by viewer during PermitGrant)
pub(crate) const CONNECTION_PERMITS: TableDefinition<&str, &str> =
    TableDefinition::new("connection_permits");

pub(crate) const DEVICES: TableDefinition<&str, &[u8]> = TableDefinition::new("devices");

// App layer content hashes: "{page_id}/{layer_name}" -> [u8; 32] SHA-256 hash
// Used for app layer change detection (skip sync if hash matches)
pub(crate) const APP_HASHES: TableDefinition<&str, &[u8]> = TableDefinition::new("app_hashes");

// Dynamic layer tables (Phase 2: two-tier permit model)

// Layer permits: "{page_id}/{user_did}/{layer_name}" -> permit token string
// Multiple permits per peer per page (one per dynamic layer)
pub(crate) const LAYER_PERMITS: TableDefinition<&str, &str> = TableDefinition::new("layer_permits");

// Dynamic layer metadata: "{page_id}/{layer_name}" -> meta JSON bytes
// Tracks which layers are dynamic (for reconnect enumeration)
pub(crate) const DYNAMIC_LAYER_META: TableDefinition<&str, &[u8]> =
    TableDefinition::new("dynamic_layer_meta");

// Viewer layer consents: "{viewer_did}/{page_id}/{layer_name}" -> consent token string
// Per-layer consent from viewer (dynamic layers require separate consent)
pub(crate) const VIEWER_LAYER_CONSENTS: TableDefinition<&str, &str> =
    TableDefinition::new("viewer_layer_consents");

// Layer authority permits: "{page_id}/{audience}/{layer_name}" -> JSON {"version": u64, "permit": string}
// Stores creator -> node authority grants used to issue per-audience access permits.
pub(crate) const LAYER_AUTHORITY_PERMITS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("layer_authority_permits");

/// RedbStore - Main persistent storage using redb
pub struct RedbStore {
    pub(crate) db: Arc<Database>,
}

impl RedbStore {
    /// Open or create a redb database at the given path
    #[instrument(skip_all)]
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = Database::create(path)?;

        // Initialize tables by opening them once
        let write_txn = db.begin_write()?;
        {
            // Core tables
            let _ = write_txn.open_table(IDENTITY)?;
            let _ = write_txn.open_table(SPACES)?;
            let _ = write_txn.open_table(PAGES)?;
            let _ = write_txn.open_table(LAYERS)?;

            // Index tables
            let _ = write_txn.open_table(SPACE_PAGES)?;

            // Access control tables
            let _ = write_txn.open_table(CONTACTS)?;
            let _ = write_txn.open_table(SPACE_SUBSCRIPTIONS)?;
            let _ = write_txn.open_table(PERMIT_CIDS)?;

            // Sync tables
            let _ = write_txn.open_table(VECTORS)?;

            // Node tables
            let _ = write_txn.open_table(SOVEREIGN_NODES)?;
            let _ = write_txn.open_table(OWNER_INFO)?;

            // Viewer consent tables
            let _ = write_txn.open_table(VIEWER_CONSENT_SPACE)?;
            let _ = write_txn.open_table(VIEWER_CONSENT_PAGE)?;

            // User permits tables
            let _ = write_txn.open_table(USER_PAGE_PERMITS)?;
            let _ = write_txn.open_table(USER_SPACE_PERMITS)?;

            // Connection permits table
            let _ = write_txn.open_table(CONNECTION_PERMITS)?;

            // App hash tracking
            let _ = write_txn.open_table(APP_HASHES)?;

            // Dynamic layer tables
            let _ = write_txn.open_table(LAYER_PERMITS)?;
            let _ = write_txn.open_table(DYNAMIC_LAYER_META)?;
            let _ = write_txn.open_table(VIEWER_LAYER_CONSENTS)?;
            let _ = write_txn.open_table(LAYER_AUTHORITY_PERMITS)?;
            let _ = write_txn.open_table(DEVICES)?;
        }
        write_txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Flush all pending writes to disk
    #[instrument(skip_all)]
    pub fn flush(&self) -> Result<()> {
        // redb commits are durable by default
        Ok(())
    }

    pub(crate) fn key2(a: &str, b: &str) -> String {
        format!("{}/{}", a, b)
    }

    pub(crate) fn key3(a: &str, b: &str, c: &str) -> String {
        format!("{}/{}/{}", a, b, c)
    }

    pub(crate) fn prefix1(a: &str) -> String {
        format!("{}/", a)
    }

    pub(crate) fn prefix2(a: &str, b: &str) -> String {
        format!("{}/{}/", a, b)
    }

    pub(crate) fn scan_prefix_str(
        &self,
        table_def: TableDefinition<&str, &str>,
        prefix: &str,
    ) -> Result<Vec<(String, String)>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(table_def)?;

        let mut rows = Vec::new();
        for result in table.range(prefix..)? {
            let (key, value) = result?;
            let key_str = key.value();
            if !key_str.starts_with(prefix) {
                break;
            }
            if let Some(suffix) = key_str.strip_prefix(prefix) {
                rows.push((suffix.to_string(), value.value().to_string()));
            }
        }
        Ok(rows)
    }

    pub(crate) fn scan_prefix_bytes(
        &self,
        table_def: TableDefinition<&str, &[u8]>,
        prefix: &str,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(table_def)?;

        let mut rows = Vec::new();
        for result in table.range(prefix..)? {
            let (key, value) = result?;
            let key_str = key.value();
            if !key_str.starts_with(prefix) {
                break;
            }
            if let Some(suffix) = key_str.strip_prefix(prefix) {
                rows.push((suffix.to_string(), value.value().to_vec()));
            }
        }
        Ok(rows)
    }
}
