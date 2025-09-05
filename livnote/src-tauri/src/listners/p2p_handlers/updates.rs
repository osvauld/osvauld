use crate::EventManager;
use log::{error, info};
use osvauld_core::models::Folder;
use serde_json::json;

/// Real-time document update and resource handlers
impl EventManager {
    /// Handle editing event (Yjs sync updates)
    pub(crate) async fn handle_editing_event(
        &self,
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
        doc_type: String,
    ) {
        info!(
            "Received editing event for resource {}, from client {}, with {} bytes",
            resource_id,
            client_id,
            updates.len()
        );

        // Check if this is for the current document
        if !EventManager::is_current_note(&self.current_note_state, &resource_id).await {
            info!(
                "Received editing event for non-active document: {}",
                resource_id
            );
            return;
        }
        self.current_note_state
            .apply_update(updates.clone(), &doc_type)
            .await;
        // Create payload for frontend
        let payload = serde_json::json!({
            "resource_id": resource_id,
            "updates": updates,
            "client_id": client_id.to_string(),
            "doc_type": doc_type,
        });

        // Emit to the frontend for direct application to the ProseMirror document
        if let Err(e) = self.emit_json("live-updates", payload) {
            error!("Failed to emit live-updates event: {}", e);
        } else {
            info!(
                "Successfully emitted live-updates event for resource: {}",
                resource_id
            );
        }
    }

    /// Handle awareness event (cursor/selection updates)
    pub(crate) async fn handle_awareness_event(
        &self,
        resource_id: String,
        client_id: u32,
        awareness_data: Vec<u8>,
    ) {
        info!(
            "Received awareness event for resource {}, from client {}, with {} bytes",
            resource_id,
            client_id,
            awareness_data.len()
        );

        // Check if this is for the current document
        if !EventManager::is_current_note(&self.current_note_state, &resource_id).await {
            info!(
                "Received awareness event for non-active document: {}",
                resource_id
            );
            return;
        }

        // Create payload for frontend
        let payload = serde_json::json!({
            "resource_id": resource_id,
            "updates": awareness_data,
            "client_id": client_id.to_string()
        });

        // Emit to the frontend for application to awareness
        if let Err(e) = self.emit_json("awareness-updates", payload) {
            error!("Failed to emit awareness-updates event: {}", e);
        } else {
            info!(
                "Successfully emitted awareness-updates event for resource: {}",
                resource_id
            );
        }
    }

    /// Handle general updates event
    pub(crate) async fn handle_update_event(
        &self,
        resource_id: String,
        updates: String,
        client_id: u32,
    ) {
        info!(
            "Received updates event for resource {}, with {} bytes",
            resource_id,
            updates.len()
        );

        // Check if we have a current note
        if let Some(current_resource_id) = self.current_note_state.get_current_note().await {
            // Emit document updates for the current note
            let payload = serde_json::json!({
                "resource_id": current_resource_id,
                "updates": updates,
                "client_id": client_id
            });

            if let Err(e) = self.emit_json("document-updates", payload) {
                error!("Failed to emit document-updates event: {}", e);
            } else {
                info!(
                    "Successfully emitted document-updates event for resource: {}",
                    current_resource_id
                );
            }
        } else {
            // No current note, emit as resource update preview
            if let Err(e) = self
                .emit_resource_preview(&resource_id, "resource-update")
                .await
            {
                error!("Failed to emit resource update preview: {}", e);
            }
        }
    }

    /// Handle resource added event
    pub(crate) async fn handle_resource_added(&self, resource_id: String, username: String) {
        info!("Handling resource added event for: {}", resource_id);

        if let Err(e) = self
            .emit_resource_preview(&resource_id, "resource-added")
            .await
        {
            error!("Failed to emit resource added preview: {}", e);
        }

        if let Err(e) = self.emit_json(
            "resource-added-notification",
            json!({ "username": username, "resource_id": resource_id }),
        ) {
            error!("Failed to emit added notification: {}", e);
        }
    }
    pub(crate) async fn handle_folders_added(&self, folders: Vec<Folder>) {
        if let Err(e) = self.emit_json(
            "folders-added-notification",
            json!({ "folders": folders.clone()}),
        ) {
            error!("Failed to emit added notification: {}", e);
        }
    }
}
