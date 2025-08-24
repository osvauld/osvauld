use crate::{
    EventManager, preview_generator::generate_preview_html, search_index::SearchIndexManager,
    types::ResourcePreview, user_state::UserState,
};
use log::{error, info};
use services::get_resource_by_id_direct;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;
pub mod live_edit;
pub mod system;
pub mod updates;

/// Common helper methods for P2P event handlers
impl EventManager {
    /// Generate and emit a resource preview for the given resource
    pub(super) async fn emit_resource_preview(
        &self,
        resource_id: &str,
        event_name: &str,
    ) -> Result<(), String> {
        // Get current user
        let user_state = self.app_handle.state::<UserState>();
        let current_user = match user_state.get_user().await {
            Ok(user) => user,
            Err(e) => {
                error!("Failed to get current user for {}: {}", event_name, e);
                return Err(format!("Failed to get current user: {}", e));
            }
        };

        // Get decrypted resource
        let decrypted_resource = match get_resource_by_id_direct(
            resource_id,
            &current_user.id,
            self.repo_ctx.clone(),
            &self.crypto_utils,
        )
        .await
        {
            Ok(resource) => resource,
            Err(e) => {
                error!(
                    "Failed to get resource {} for {}: {}",
                    resource_id, event_name, e
                );
                return Err(format!("Failed to get resource: {}", e));
            }
        };

        // Generate preview
        let (preview, title) = match generate_preview_html(&decrypted_resource.data, 3).await {
            Ok((preview, title)) => (preview, title),
            Err(e) => {
                error!(
                    "Failed to generate preview for resource {}: {}",
                    decrypted_resource.id, e
                );
                (String::new(), String::new())
            }
        };

        // Create resource preview
        let resource_preview = ResourcePreview {
            id: decrypted_resource.id.clone(),
            title,
            preview,
            favourite: decrypted_resource.favourite,
            last_accessed: decrypted_resource.last_accessed,
            folder_id: decrypted_resource.folder_id.clone(),
            last_modified: decrypted_resource.last_accessed,
        };

        // Emit event
        if let Err(e) = self.app_handle.emit(event_name, resource_preview) {
            error!("Failed to emit {} event: {}", event_name, e);
            return Err(format!("Failed to emit event: {}", e));
        }
        let search_manager = self.app_handle.state::<Arc<Mutex<SearchIndexManager>>>();
        let search_manager_clone = search_manager.inner().clone();
        let resource_id_clone = decrypted_resource.id.clone();
        let resource_data = decrypted_resource.data.clone();
        let folder_id = decrypted_resource.folder_id.clone();

        tokio::spawn(async move {
            info!(
                "Spawning background task to update search index for resource {}",
                resource_id_clone
            );
            if let Err(e) = search_manager_clone
                .lock()
                .await
                .update_resource(&resource_id_clone, &resource_data, &folder_id)
                .await
            {
                error!(
                    "Failed to update search index for resource {}: {}",
                    resource_id_clone, e
                );
            } else {
                info!(
                    "Successfully updated search index for resource {}",
                    resource_id_clone
                );
            }
        });

        info!(
            "Successfully emitted {} for resource: {}",
            event_name, resource_id
        );
        Ok(())
    }

    /// Emit a simple JSON payload to the frontend
    pub(super) fn emit_json(
        &self,
        event_name: &str,
        payload: serde_json::Value,
    ) -> Result<(), String> {
        if let Err(e) = self.app_handle.emit(event_name, payload) {
            error!("Failed to emit {} event: {}", event_name, e);
            Err(format!("Failed to emit {}: {}", event_name, e))
        } else {
            Ok(())
        }
    }
}
