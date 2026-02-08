//! Composable test strategies for Butler property-based testing
//!
//! Layered design:
//!   Level 1 (Primitives): ids, names - re-exported from gurkha
//!   Level 2 (Models): Space, Page, PageMeta
//!   Level 3 (Complex): Full scenarios

#![allow(dead_code)]

use proptest::prelude::*;
use domains::{Space, Page, PageMeta};

// Re-export shared primitives from gurkha
pub use gurkha::test_strategies::{did_strategy, segment_strategy, page_id_strategy, expand_pattern_concrete};

/// Valid ID (UUID-like hex string)
pub fn id_strategy() -> impl Strategy<Value = String> {
    "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}"
}

/// Human-readable name
pub fn name_strategy() -> impl Strategy<Value = String> {
    "[A-Za-z][A-Za-z0-9 ]{2,30}"
}

/// Timestamp (recent, within last year)
pub fn timestamp_strategy() -> impl Strategy<Value = i64> {
    let now = chrono::Utc::now().timestamp();
    let year_ago = now - 365 * 24 * 60 * 60;
    year_ago..=now
}

/// Space strategy
pub fn space_strategy() -> impl Strategy<Value = Space> {
    (id_strategy(), name_strategy(), did_strategy(), timestamp_strategy())
        .prop_map(|(id, name, owner_did, created_at)| Space {
            id,
            name,
            parent_space_id: None,
            owner_did,
            is_default: false,
            description: None,
            created_at,
            updated_at: created_at,
        })
}

/// Page strategy
pub fn page_strategy() -> impl Strategy<Value = Page> {
    (id_strategy(), id_strategy(), name_strategy(), did_strategy(), timestamp_strategy())
        .prop_map(|(id, space_id, name, owner_did, created_at)| Page {
            id,
            space_id,
            name,
            owner_did,
            is_private: false,
            created_at,
            updated_at: created_at,
        })
}

/// PageMeta strategy (with encrypted key)
pub fn page_meta_strategy() -> impl Strategy<Value = PageMeta> {
    (
        id_strategy(),
        id_strategy(),
        name_strategy(),
        did_strategy(),
        timestamp_strategy(),
        prop::collection::vec(any::<u8>(), 32..=32),
    )
        .prop_map(|(id, space_id, name, owner_did, created_at, encrypted_key)| PageMeta {
            id,
            space_id,
            name,
            encrypted_key,
            owner_did,
            is_private: false,
            created_at,
            updated_at: created_at,
        })
}

/// Generate a space with N pages
pub fn space_with_pages_strategy(max_pages: usize) -> impl Strategy<Value = (Space, Vec<Page>)> {
    space_strategy().prop_flat_map(move |space| {
        let space_id = space.id.clone();
        let owner_did = space.owner_did.clone();
        let created_at = space.created_at;

        prop::collection::vec(
            (name_strategy(), id_strategy()).prop_map(move |(name, id)| Page {
                id,
                space_id: space_id.clone(),
                name,
                owner_did: owner_did.clone(),
                is_private: false,
                created_at,
                updated_at: created_at,
            }),
            0..max_pages,
        ).prop_map(move |pages| (space.clone(), pages))
    })
}
