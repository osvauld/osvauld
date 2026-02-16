//! ContactService - Stateless functions for managing contacts
//!
//! All functions take store as first parameter - Butler injects this.
//!
//! Contacts store identity info (did, encryption_key, username, devices).

use crate::error::Result;
use crate::models::{ContactData, ContactType};
use crate::storage::RedbStore;
use tracing::instrument;

/// Get a contact by DID
#[instrument(skip(store), fields(user_did = %user_did))]
pub fn get_contact(store: &RedbStore, user_did: &str) -> Result<Option<ContactData>> {
    store.get_contact(user_did)
}

/// Create or update a contact
#[instrument(skip_all)]
pub fn upsert_contact(store: &RedbStore, contact: &ContactData) -> Result<()> {
    store.put_contact(contact)
}

/// List all contacts
#[instrument(skip_all)]
pub fn list_contacts(store: &RedbStore) -> Result<Vec<ContactData>> {
    store.list_contacts()
}

/// Delete a contact
#[instrument(skip(store), fields(user_did = %user_did))]
pub fn delete_contact(store: &RedbStore, user_did: &str) -> Result<bool> {
    store.delete_contact(user_did)
}

/// Create or update a node contact (viewer → node relationship)
///
/// Used when viewer connects to a node - stores node info for future reconnection.
#[instrument(skip(store, permit), fields(did = %did, name = %name, node_id = %node_id))]
pub fn upsert_node_contact(
    store: &RedbStore,
    did: &str,
    encryption_key: &str,
    name: &str,
    node_id: &str,
    permit: &str,
) -> Result<ContactData> {
    let contact = ContactData::new_node(
        did.to_string(),
        encryption_key.to_string(),
        name.to_string(),
        node_id.to_string(),
        permit.to_string(),
    );
    store.put_contact(&contact)?;
    Ok(contact)
}

/// List node contacts only (filter by ContactType::Node)
#[instrument(skip_all)]
pub fn list_node_contacts(store: &RedbStore) -> Result<Vec<ContactData>> {
    let all = store.list_contacts()?;
    Ok(all
        .into_iter()
        .filter(|c| c.contact_type == ContactType::Node)
        .collect())
}
