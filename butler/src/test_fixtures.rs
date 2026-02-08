//! Test fixtures for Butler
//!
//! Re-exports gurkha fixtures and provides butler-specific test helpers.

// Re-export gurkha test fixtures (permit creation)
pub use gurkha::test_fixtures::*;

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::storage::{RedbStore, LayerCache, AssetStore};
use domains::{Space, Page, PageMeta};

// Test Store Creation

/// Create an in-memory RedbStore for testing
pub fn test_store() -> Arc<RedbStore> {
    // RedbStore requires a path, use temp file for tests
    let temp_path = std::env::temp_dir()
        .join(format!("butler_test_{}.redb", uuid::Uuid::new_v4()));
    Arc::new(RedbStore::open(&temp_path).expect("Failed to create test store"))
}

/// Create a test layer cache
pub fn test_layer_cache(store: Arc<RedbStore>) -> Arc<RwLock<LayerCache>> {
    Arc::new(RwLock::new(LayerCache::new(store, 100)))
}

/// Create a test asset store (uses temp directory)
pub fn test_asset_store() -> Arc<AssetStore> {
    let temp_dir = std::env::temp_dir().join(format!("butler_assets_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    Arc::new(AssetStore::new(temp_dir).expect("Failed to create asset store"))
}

// Model Builders

/// Create a test Space
pub fn test_space(id: &str, name: &str, owner_did: &str) -> Space {
    Space {
        id: id.to_string(),
        name: name.to_string(),
        parent_space_id: None,
        owner_did: owner_did.to_string(),
        is_default: false,
        description: None,
        created_at: chrono::Utc::now().timestamp(),
        updated_at: chrono::Utc::now().timestamp(),
    }
}

/// Create a test Page
pub fn test_page(id: &str, space_id: &str, name: &str, owner_did: &str) -> Page {
    Page {
        id: id.to_string(),
        space_id: space_id.to_string(),
        name: name.to_string(),
        owner_did: owner_did.to_string(),
        is_private: false,
        created_at: chrono::Utc::now().timestamp(),
        updated_at: chrono::Utc::now().timestamp(),
    }
}

/// Create a test PageMeta (with encrypted_key)
pub fn test_page_meta(id: &str, space_id: &str, name: &str, owner_did: &str) -> PageMeta {
    PageMeta {
        id: id.to_string(),
        space_id: space_id.to_string(),
        name: name.to_string(),
        encrypted_key: vec![0u8; 32], // Dummy encrypted key
        owner_did: owner_did.to_string(),
        is_private: false,
        created_at: chrono::Utc::now().timestamp(),
        updated_at: chrono::Utc::now().timestamp(),
    }
}

// Scenario Builders

/// Create a space with multiple pages
pub fn test_space_with_pages(
    space_id: &str,
    space_name: &str,
    owner_did: &str,
    page_names: &[&str],
) -> (Space, Vec<Page>) {
    let space = test_space(space_id, space_name, owner_did);

    let pages: Vec<Page> = page_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            test_page(
                &format!("{}_{}", space_id, i),
                space_id,
                name,
                owner_did,
            )
        })
        .collect();

    (space, pages)
}

/// Create shop scenario (owner + customers)
pub fn shop_scenario(page_id: &str) -> ShopScenario {
    ShopScenario {
        page_id: page_id.to_string(),
        owner_did: format!("did:key:shop_owner_{}", page_id),
        customer_dids: vec![
            format!("did:key:customer_a_{}", page_id),
            format!("did:key:customer_b_{}", page_id),
        ],
    }
}

/// Shop scenario data
pub struct ShopScenario {
    pub page_id: String,
    pub owner_did: String,
    pub customer_dids: Vec<String>,
}

impl ShopScenario {
    /// Create owner permit
    pub fn owner_permit(&self) -> gurkha::Permit {
        gurkha::test_fixtures::shop_owner(&self.page_id, &self.owner_did)
    }

    /// Create customer permit at index
    pub fn customer_permit(&self, index: usize) -> gurkha::Permit {
        let did = &self.customer_dids[index];
        gurkha::test_fixtures::shop_customer(&self.page_id, did)
    }

    /// Get customer's order layer name
    pub fn customer_order_layer(&self, index: usize) -> String {
        format!("{}/orders/{}", self.page_id, self.customer_dids[index])
    }
}

// Collaborative Scenario

/// Create collaborative document scenario
pub fn collab_scenario(page_id: &str) -> CollabScenario {
    CollabScenario {
        page_id: page_id.to_string(),
        owner_did: format!("did:key:collab_owner_{}", page_id),
        collaborator_dids: vec![
            format!("did:key:collab_a_{}", page_id),
            format!("did:key:collab_b_{}", page_id),
        ],
    }
}

/// Collaborative scenario data
pub struct CollabScenario {
    pub page_id: String,
    pub owner_did: String,
    pub collaborator_dids: Vec<String>,
}

impl CollabScenario {
    /// Create owner permit
    pub fn owner_permit(&self) -> gurkha::Permit {
        gurkha::test_fixtures::collab_owner(&self.page_id, &self.owner_did)
    }

    /// Create collaborator permit at index
    pub fn collaborator_permit(&self, index: usize) -> gurkha::Permit {
        let did = &self.collaborator_dids[index];
        gurkha::test_fixtures::collab_collaborator(&self.page_id, did)
    }
}
