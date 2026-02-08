//! Contacts API - Contact and device resolution operations

use crate::{Butler, Result, ContactData, ConnectionDeviceInfo};
use crate::services::{contact_service, node_service};

/// Contacts API facade
///
/// Access via `butler.contacts()`
pub struct ContactsApi<'a> {
    pub(crate) butler: &'a Butler,
}

impl<'a> ContactsApi<'a> {

    /// Get a contact by DID
    pub fn get(&self, user_did: &str) -> Result<Option<ContactData>> {
        contact_service::get_contact(self.butler.store(), user_did)
    }

    /// Create or update a contact
    pub fn upsert(&self, contact: &ContactData) -> Result<()> {
        contact_service::upsert_contact(self.butler.store(), contact)
    }

    /// List all contacts
    pub fn list(&self) -> Result<Vec<ContactData>> {
        contact_service::list_contacts(self.butler.store())
    }

    /// Add a node contact (viewer side - for tracking nodes viewer is subscribed to)
    pub fn add_node(&self, did: &str, encryption_key: &str, name: &str, node_id: &str, permit: &str) -> Result<ContactData> {
        contact_service::upsert_node_contact(self.butler.store(), did, encryption_key, name, node_id, permit)
    }

    /// List node contacts only (viewer side)
    pub fn list_nodes(&self) -> Result<Vec<ContactData>> {
        contact_service::list_node_contacts(self.butler.store())
    }

    /// Resolve user_did to device connection info.
    ///
    /// **Context**: Called by Coordinator when handling EnsureSync.
    /// **We query**: Contact info for user, then connection permit.
    /// **We return**: ConnectionDeviceInfo with node_id and permit.
    pub fn resolve_device(&self, user_did: &str) -> Result<Option<ConnectionDeviceInfo>> {
        // 1. Try contacts first (for viewer → node connections)
        if let Some(contact) = contact_service::get_contact(self.butler.store(), user_did)? {
            // Node type contacts have node_id and permit directly
            if let (Some(node_id), Some(permit)) = (contact.node_id, contact.permit) {
                return Ok(Some(ConnectionDeviceInfo { node_id, permit }));
            }
        }

        // 2. Try sovereign nodes (for owner → node connections)
        for node in node_service::list_sovereign_nodes(self.butler.store())? {
            if node.did == user_did {
                if let Some(permit) = node.permit {
                    return Ok(Some(ConnectionDeviceInfo {
                        node_id: node.node_id,
                        permit,
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Store viewer-issued consent permits
    ///
    /// **Context**: Node receives SyncConsentGrant from viewer.
    /// The permits are viewer-issued, expressing consent for receiving sync updates.
    pub async fn store_viewer_consent(
        &self,
        viewer_did: &str,
        space_id: &str,
        space_consent_permit: &str,
        page_consent_permits: &[(String, String)],
    ) -> Result<()> {
        // Store space consent (only if not empty)
        if !space_consent_permit.is_empty() {
            self.butler.store().put_viewer_space_consent(viewer_did, space_id, space_consent_permit)?;
        }

        // Store page consents
        for (page_id, permit) in page_consent_permits {
            self.butler.store().put_viewer_page_consent(viewer_did, page_id, permit)?;
        }

        Ok(())
    }

    /// Get viewer's space consent permit
    ///
    /// **Context**: Node needs consent permit to send sync updates
    pub fn get_viewer_space_consent(&self, viewer_did: &str, space_id: &str) -> Result<Option<String>> {
        self.butler.store().get_viewer_space_consent(viewer_did, space_id)
    }

    /// Get viewer's page consent permit
    ///
    /// **Context**: Node needs consent permit to send layer updates
    pub fn get_viewer_page_consent(&self, viewer_did: &str, page_id: &str) -> Result<Option<String>> {
        self.butler.store().get_viewer_page_consent(viewer_did, page_id)
    }
}
