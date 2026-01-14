//! RedbStore - Main persistent storage for Osvauld using redb
//!
//! Split into logical submodules by table type.

mod identity;
mod space;
mod page;
mod layer;
mod contact;
mod node;
mod sync;
mod permit;

use crate::error::{ButlerError, Result};
use redb::{Database, TableDefinition};
use std::path::Path;
use std::sync::Arc;

// ==================== Table Definitions ====================

// Core tables
pub(crate) const IDENTITY: TableDefinition<&str, &[u8]> = TableDefinition::new("identity");
pub(crate) const SPACES: TableDefinition<&str, &[u8]> = TableDefinition::new("spaces");
pub(crate) const PAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("pages");
pub(crate) const LAYERS: TableDefinition<&str, &[u8]> = TableDefinition::new("layers");

// Index tables
pub(crate) const SPACE_PAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("space_pages");

// Access control tables
pub(crate) const CONTACTS: TableDefinition<&str, &[u8]> = TableDefinition::new("contacts");
pub(crate) const SPACE_SUBSCRIPTIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("space_subscriptions");
pub(crate) const PERMIT_CIDS: TableDefinition<&str, &str> = TableDefinition::new("permit_cids");

// Sync tables
pub(crate) const VECTORS: TableDefinition<&str, &[u8]> = TableDefinition::new("vectors");

// Node tables
pub(crate) const SOVEREIGN_NODES: TableDefinition<&str, &[u8]> = TableDefinition::new("sovereign_nodes");
pub(crate) const OWNER_INFO: TableDefinition<&str, &[u8]> = TableDefinition::new("owner_info");

// Viewer consent tables
pub(crate) const VIEWER_CONSENT_SPACE: TableDefinition<&str, &str> = TableDefinition::new("viewer_consent_space");
pub(crate) const VIEWER_CONSENT_PAGE: TableDefinition<&str, &str> = TableDefinition::new("viewer_consent_page");

// User permits tables
pub(crate) const USER_PAGE_PERMITS: TableDefinition<&str, &str> = TableDefinition::new("user_page_permits");
pub(crate) const USER_SPACE_PERMITS: TableDefinition<&str, &str> = TableDefinition::new("user_space_permits");

// Legacy tables
pub(crate) const DEVICES: TableDefinition<&str, &[u8]> = TableDefinition::new("devices");

/// RedbStore - Main persistent storage using redb
pub struct RedbStore {
    pub(crate) db: Arc<Database>,
}

impl RedbStore {
    /// Open or create a redb database at the given path
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

            // Legacy tables
            let _ = write_txn.open_table(DEVICES)?;
        }
        write_txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<()> {
        // redb commits are durable by default
        Ok(())
    }
}
