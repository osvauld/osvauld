//! ContactService - Stateless functions for managing contacts and shares
//!
//! All functions take store as first parameter - Butler injects this.
//!
//! Used by Node to track viewers and their permits.

use crate::error::Result;
use crate::storage::RedbStore;
use crate::models::{ContactData, ShareInfo};

// ==================== CONTACT CRUD ====================

/// Get a contact by DID
pub fn get_contact(store: &RedbStore, user_did: &str) -> Result<Option<ContactData>> {
    store.get_contact(user_did)
}

/// Create or update a contact
pub fn upsert_contact(store: &RedbStore, contact: &ContactData) -> Result<()> {
    store.put_contact(contact)
}

/// List all contacts
pub fn list_contacts(store: &RedbStore) -> Result<Vec<ContactData>> {
    store.list_contacts()
}

/// Delete a contact
pub fn delete_contact(store: &RedbStore, user_did: &str) -> Result<bool> {
    store.delete_contact(user_did)
}

// ==================== SHARE OPERATIONS ====================

/// Add a share to a contact (updates shares_by_page index)
///
/// Contact must already exist. Call upsert_contact first if needed.
pub fn add_share_to_contact(
    store: &RedbStore,
    user_did: &str,
    page_id: &str,
    share: ShareInfo,
) -> Result<()> {
    store.add_share_to_contact(user_did, page_id, share)
}

/// Get all user DIDs who have a share for a given page
///
/// Used by Node to push updates to all viewers of a page.
pub fn get_users_for_page(store: &RedbStore, page_id: &str) -> Result<Vec<String>> {
    store.get_users_for_page(page_id)
}

/// Get contacts with their shares for a page (combines index lookup + contact fetch)
///
/// Returns contacts who have access to the given page.
pub fn get_contacts_for_page(store: &RedbStore, page_id: &str) -> Result<Vec<ContactData>> {
    let user_dids = store.get_users_for_page(page_id)?;
    let mut contacts = Vec::new();

    for user_did in user_dids {
        if let Some(contact) = store.get_contact(&user_did)? {
            contacts.push(contact);
        }
    }

    Ok(contacts)
}
