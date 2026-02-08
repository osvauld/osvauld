//! SpaceService - Stateless functions for Space operations
//!
//! All functions take store as first parameter. Butler injects dependencies.

use crate::error::{ButlerError, Result};
use crate::storage::RedbStore;
use tracing::instrument;
use crate::models::{
    SpaceData, SpaceMeta, Space,
};

/// Create a new space with owner permit
#[instrument(skip(store, signing_key, permit_template_json), fields(name = %name, owner_did = %owner_did))]
pub async fn create_space(
    store: &RedbStore,
    name: String,
    owner_did: String,
    signing_key: &[u8; 32],
    permit_template_json: &str,
) -> Result<Space> {
    let meta = SpaceMeta::new(name, owner_did);

    // Issue space owner permit via gurkha
    let (owner_permit, _cid) = gurkha::issue_space_owner_token(
        signing_key,
        &meta.id,
        permit_template_json,
    ).await.map_err(|e| ButlerError::permit_error(e.to_string()))?;

    // Store space with owner's permit
    let mut data = SpaceData::new(meta.clone());
    data.set_permit(owner_permit);
    store.put_space(&data)?;

    Ok(Space::from(meta))
}

/// Create a space with a parent
#[instrument(skip(store), fields(name = %name, parent_id = %parent_id, owner_did = %owner_did))]
pub fn create_child_space(
    store: &RedbStore,
    name: String,
    parent_id: String,
    owner_did: String,
) -> Result<Space> {
    // Verify parent exists
    if store.get_space(&parent_id)?.is_none() {
        return Err(ButlerError::space_not_found(&parent_id));
    }

    let meta = SpaceMeta::new(name, owner_did).with_parent(parent_id);
    let data = SpaceData::new(meta.clone());
    store.put_space(&data)?;
    Ok(Space::from(meta))
}

/// Get a space by ID
#[instrument(skip(store), fields(space_id = %space_id))]
pub fn get_space(store: &RedbStore, space_id: &str) -> Result<Option<Space>> {
    Ok(store.get_space(space_id)?.map(Space::from))
}

/// Get space with its shares
#[instrument(skip(store), fields(space_id = %space_id))]
pub fn get_space_with_shares(store: &RedbStore, space_id: &str) -> Result<Option<SpaceData>> {
    store.get_space(space_id)
}

/// List all root spaces
#[instrument(skip_all)]
pub fn list_root_spaces(store: &RedbStore) -> Result<Vec<Space>> {
    Ok(store.list_root_spaces()?
        .into_iter()
        .map(Space::from)
        .collect())
}

/// List child spaces of a parent
#[instrument(skip(store), fields(parent_id = %parent_id))]
pub fn list_child_spaces(store: &RedbStore, parent_id: &str) -> Result<Vec<Space>> {
    Ok(store.list_child_spaces(parent_id)?
        .into_iter()
        .map(Space::from)
        .collect())
}

/// List all spaces
#[instrument(skip_all)]
pub fn list_all_spaces(store: &RedbStore) -> Result<Vec<Space>> {
    Ok(store.list_spaces()?
        .into_iter()
        .map(Space::from)
        .collect())
}

/// Delete a space (and optionally its pages)
#[instrument(skip(store), fields(space_id = %space_id, delete_pages = %delete_pages))]
pub fn delete_space(store: &RedbStore, space_id: &str, delete_pages: bool) -> Result<bool> {
    if delete_pages {
        // Delete all pages in this space
        let pages = store.list_pages_in_space(space_id)?;
        for page in pages {
            // Delete layers first
            store.delete_all_layers(&page.meta.id)?;
            // Then delete page
            store.delete_page(space_id, &page.meta.id)?;
        }
    }
    store.delete_space(space_id)
}

/// Track that space was shared with a user (stores pubkey only)
///
/// Note: Actual permits are stored elsewhere (Contact.shares on Node-side)
#[instrument(skip(store), fields(space_id = %space_id))]
pub fn share_space(store: &RedbStore, space_id: &str, user_pubkey: String) -> Result<()> {
    let mut space = store.get_space(space_id)?
        .ok_or_else(|| ButlerError::space_not_found(space_id))?;
    space.add_share(user_pubkey);
    store.put_space(&space)?;
    Ok(())
}

/// Remove share tracking for a user from space
#[instrument(skip(store), fields(space_id = %space_id, user_pubkey = %user_pubkey))]
pub fn unshare_space(store: &RedbStore, space_id: &str, user_pubkey: &str) -> Result<bool> {
    let mut space = store.get_space(space_id)?
        .ok_or_else(|| ButlerError::space_not_found(space_id))?;
    let removed = space.remove_share(user_pubkey);
    store.put_space(&space)?;
    Ok(removed)
}

/// Set permit for a space (stores MY permit)
#[instrument(skip(store, permit), fields(space_id = %space_id))]
pub fn set_space_permit(store: &RedbStore, space_id: &str, permit: String) -> Result<()> {
    let mut space = store.get_space(space_id)?
        .ok_or_else(|| ButlerError::space_not_found(space_id))?;
    space.set_permit(permit);
    store.put_space(&space)?;
    Ok(())
}

/// Store a space received from owner via publish (Node mode)
#[instrument(skip(store, permit), fields(space_id = %space.id))]
pub fn store_published_space(store: &RedbStore, space: &Space, permit: &str) -> Result<()> {
    store_published_space_with_source(store, space, permit, None)
}

/// Store a space received from another peer with source tracking
#[instrument(skip(store, permit), fields(space_id = %space.id))]
pub fn store_published_space_with_source(
    store: &RedbStore,
    space: &Space,
    permit: &str,
    source_node_id: Option<&str>,
) -> Result<()> {
    let meta = SpaceMeta {
        id: space.id.clone(),
        name: space.name.clone(),
        parent_space_id: space.parent_space_id.clone(),
        owner_did: space.owner_did.clone(),
        is_default: space.is_default,
        description: space.description.clone(),
        created_at: space.created_at,
        updated_at: space.updated_at,
    };

    let mut data = if let Some(node_id) = source_node_id {
        SpaceData::with_source(meta, node_id.to_string())
    } else {
        SpaceData::new(meta)
    };
    data.set_permit(permit.to_string());
    store.put_space(&data)?;
    Ok(())
}

/// Mark a space as published to a specific node
#[instrument(skip(store), fields(space_id = %space_id, node_id = %node_id))]
pub fn mark_space_published(store: &RedbStore, space_id: &str, node_id: &str) -> Result<()> {
    let mut space = store.get_space(space_id)?
        .ok_or_else(|| ButlerError::space_not_found(space_id))?;

    // Add node_id to published_to list (using shares for now - could add separate field)
    // For simplicity, store as "published:{node_id}" in shares
    space.add_share(format!("published:{}", node_id));
    store.put_space(&space)?;
    Ok(())
}

/// Get all spaces a user has access to
#[instrument(skip(store), fields(user_did = %user_did))]
pub fn get_accessible_spaces(store: &RedbStore, user_did: &str) -> Result<Vec<Space>> {
    let all_spaces = store.list_spaces()?;
    Ok(all_spaces
        .into_iter()
        .filter(|s| s.meta.owner_did == user_did || s.has_share(user_did))
        .map(Space::from)
        .collect())
}

/// Check if a user has access to a space
#[instrument(skip(store), fields(space_id = %space_id, user_did = %user_did))]
pub fn has_space_access(store: &RedbStore, space_id: &str, user_did: &str) -> Result<bool> {
    if let Some(space) = store.get_space(space_id)? {
        Ok(space.meta.owner_did == user_did || space.has_share(user_did))
    } else {
        Ok(false)
    }
}
