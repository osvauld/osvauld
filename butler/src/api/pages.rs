//! Pages API - Page CRUD and lifecycle operations

use ractor::ActorRef;
use std::collections::HashMap;

use crate::services::page_service;
use crate::{Butler, DecryptedPage, Page, PageData, Result, ScribeMessage};

/// Pages API facade
///
/// Access via `butler.pages()`
pub struct PagesApi<'a> {
    pub(crate) butler: &'a Butler,
}

impl<'a> PagesApi<'a> {
    /// Create a new page in a space
    pub async fn create(
        &self,
        space_id: &str,
        name: &str,
        layer_names: Vec<String>,
        permit_template: &str,
    ) -> Result<Page> {
        let identity = self.butler.get_identity().await?;
        let owner_did = identity.did().to_string();
        let owner_public_key = identity.public_encryption_key();
        let signing_key = identity.secret_signing_key();
        page_service::create_page(
            self.butler.store(),
            space_id.to_string(),
            name.to_string(),
            owner_did,
            &owner_public_key,
            &signing_key,
            layer_names,
            permit_template,
        )
        .await
    }

    /// Get a page by ID
    pub fn get(&self, id: &str) -> Result<Option<PageData>> {
        page_service::find_page_by_id(self.butler.store(), id)
    }

    /// Get decrypted page content
    pub async fn get_decrypted(&self, id: &str) -> Result<(DecryptedPage, [u8; 32])> {
        let identity = self.butler.get_identity().await?;
        let user_did = identity.did().to_string();
        let secret_key = identity.secret_encryption_key();
        page_service::get_decrypted_page(self.butler.store(), id, &user_did, &secret_key).await
    }

    /// List pages in a space
    pub fn list(&self, space_id: &str) -> Result<Vec<Page>> {
        page_service::list_pages(self.butler.store(), space_id)
    }

    /// List all pages
    pub fn list_all(&self) -> Result<Vec<Page>> {
        page_service::get_pages(self.butler.store())
    }

    /// Delete a page
    pub fn delete(&self, space_id: &str, page_id: &str) -> Result<bool> {
        page_service::delete_page(self.butler.store(), space_id, page_id)
    }

    /// List page IDs for a space
    pub fn list_ids(&self, space_id: &str) -> Result<Vec<String>> {
        let pages = page_service::list_pages(self.butler.store(), space_id)?;
        Ok(pages.into_iter().map(|p| p.id).collect())
    }

    /// Open a page's Scribe actor
    pub async fn open(&self, id: &str) -> Result<ActorRef<ScribeMessage>> {
        self.butler.open_page(id).await
    }

    /// Close a page's Scribe actor
    pub async fn close(&self, id: &str) -> Result<()> {
        self.butler.close_page(id).await
    }

    /// Set the permit for a page (stores as "our" permit on PageData)
    ///
    /// **Context**: Viewer receives PermitUpdate from node after SpaceDataAck.
    /// This stores the permit on PageData.permit so build_scribe_args can
    /// find it as our_permit for Scribe authorization.
    pub fn set_permit(&self, page_id: &str, permit: String) -> Result<()> {
        let mut page = self
            .butler
            .store()
            .find_page_by_id(page_id)?
            .ok_or_else(|| crate::error::ButlerError::page_not_found(page_id))?;
        page.set_permit(permit);
        self.butler.store().put_page(&page)?;
        Ok(())
    }

    /// Get source node for a page (for lazy sync)
    ///
    /// Returns the node_id that gave us this page's space, if any.
    pub fn get_source_node(&self, page_id: &str) -> Result<Option<String>> {
        page_service::get_source_node_for_page(self.butler.store(), page_id)
    }

    /// Update page layers with explicit secret key
    pub fn update_layers(
        &self,
        page_id: &str,
        secret_key: &[u8; 32],
        layer_updates: &HashMap<String, serde_json::Value>,
    ) -> Result<Page> {
        page_service::update_page_layers(self.butler.store(), page_id, secret_key, layer_updates)
    }

    /// Update page layers (auto-fetches secret key from identity)
    pub async fn update_layers_auto(
        &self,
        page_id: &str,
        layer_updates: &HashMap<String, serde_json::Value>,
    ) -> Result<Page> {
        let identity = self.butler.get_identity().await?;
        let secret_key = identity.secret_encryption_key();
        page_service::update_page_layers(self.butler.store(), page_id, &secret_key, layer_updates)
    }

    /// Extract layer names from a permit template JSON
    ///
    /// Looks for `owner_template.layers` field (matching gurkha's expected structure).
    pub fn extract_layer_names_from_template(permit_template_json: &str) -> Vec<String> {
        let Ok(template) = serde_json::from_str::<serde_json::Value>(permit_template_json) else {
            return Vec::new();
        };

        let Some(layers) = template
            .get("owner_template")
            .and_then(|ot| ot.get("layers"))
            .and_then(|l| l.as_object())
        else {
            return Vec::new();
        };

        layers.keys().cloned().collect()
    }
}
