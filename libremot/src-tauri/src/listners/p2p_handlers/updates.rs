use crate::EventManager;
use crate::chat_preview_generator::ChatPreviewGenerator;
use crate::user_state::UserState;
use log::{error, info};
use osvauld_core::models::Folder;
use serde_json::json;
use tauri::Manager;

/// Real-time document update and resource handlers
impl EventManager {
    /// Handle editing event (Yjs sync updates)
    /// For chat: Always apply updates regardless of which chat is currently open
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

        // Doc type is already "chat" or "image_state" from backend
        // No mapping needed - backend Resource model uses "chat" for Chat type

        // Apply to chat state buffer (always, regardless of which chat is open)
        self.chat_state
            .apply_update(&resource_id, updates.clone(), &doc_type)
            .await;

        // Create payload for frontend
        let payload = serde_json::json!({
            "resource_id": resource_id,
            "updates": updates,
            "client_id": client_id.to_string(),
            "doc_type": doc_type,  // Already "chat" or "image_state"
        });

        // Emit to the frontend for direct application
        // Frontend will decide whether to:
        // - Apply to open editor (if this chat is currently open)
        // - Update unread count (if this chat is not open)
        if let Err(e) = self.emit_json("live-updates", payload) {
            error!("Failed to emit live-updates event: {}", e);
        } else {
            info!(
                "Successfully emitted live-updates event for resource: {}",
                resource_id
            );
        }

        // Generate and emit chat preview for sidebar updates
        // This updates last message, unread count, timestamp, etc.
        self.emit_chat_preview_update(&resource_id).await;

        // CRITICAL: Save to database so messages persist across sessions
        // Without this, messages only exist in memory and disappear on restart/reload
        if doc_type == "chat" {
            self.save_chat_to_database(&resource_id).await;
        }
    }

    /// Generate and emit chat preview update
    /// This is used to update the chat list in the sidebar with latest message info
    async fn emit_chat_preview_update(&self, resource_id: &str) {
        // Get current user ID
        let user_state = self.app_handle.state::<UserState>();
        let current_user_id = match user_state.get_user().await {
            Ok(user) => user.id,
            Err(e) => {
                error!("Failed to get current user for preview generation: {}", e);
                return;
            }
        };

        // Get the updated chat state from ChatState buffers
        let chat_state_bytes = match self.get_chat_state_bytes(resource_id).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to get chat state for preview: {}", e);
                return;
            }
        };

        // Generate preview
        let preview_generator = ChatPreviewGenerator::new();
        match preview_generator.generate_chat_preview(&chat_state_bytes, &current_user_id).await {
            Ok(preview) => {
                // Emit preview update to frontend
                let preview_payload = serde_json::json!({
                    "resource_id": resource_id,
                    "last_message": preview.last_message,
                    "last_message_time": preview.last_message_time,
                    "unread_count": preview.unread_count,
                    "participants": preview.participants,
                    "participant_ids": preview.participant_ids,
                });

                if let Err(e) = self.emit_json("chat-preview-update", preview_payload) {
                    error!("Failed to emit chat preview update: {}", e);
                } else {
                    info!(
                        "Emitted chat preview update for resource: {} (unread: {})",
                        resource_id, preview.unread_count
                    );
                }
            }
            Err(e) => {
                error!("Failed to generate chat preview: {}", e);
            }
        }
    }

    /// Get chat state bytes from ChatState buffers (for main_doc)
    async fn get_chat_state_bytes(&self, resource_id: &str) -> Result<Vec<u8>, String> {
        // Export the full document state from ChatState
        self.chat_state.export_document_state(resource_id).await
    }

    /// Save chat state to database
    /// This is critical for persistence - without it, messages only exist in memory
    async fn save_chat_to_database(&self, resource_id: &str) {
        use services::update_resource;
        use base64::{Engine, engine::general_purpose};

        // Get current user and device from state
        let user_state = self.app_handle.state::<UserState>();
        
        let user = match user_state.get_user().await {
            Ok(u) => u,
            Err(e) => {
                error!("Failed to get user for saving chat: {}", e);
                return;
            }
        };

        let device = match user_state.get_device().await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to get device for saving chat: {}", e);
                return;
            }
        };

        // Export full chat state from buffer
        let chat_state_bytes = match self.chat_state.export_document_state(resource_id).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to export chat state for resource {}: {}", resource_id, e);
                return;
            }
        };

        // Convert to base64 string for database storage
        let chat_state_b64 = general_purpose::STANDARD.encode(&chat_state_bytes);

        // Create resource data in the expected format
        // For Chat resource type, the key is "chat" not "main_doc"
        let resource_data = serde_json::json!({
            "chat": chat_state_b64,
            "image_state": ""  // We're only saving chat doc here
        });

        // Save to database
        match update_resource(
            resource_id,
            resource_data.to_string(),
            &user.id,
            &device.id,
            self.repo_ctx.clone(),
            &self.crypto_utils,
        ).await {
            Ok(_) => {
                info!("Successfully saved chat {} to database", resource_id);
            }
            Err(e) => {
                error!("Failed to save chat {} to database: {}", resource_id, e);
            }
        }
    }

    /// Handle awareness event (cursor/selection updates)
    /// For chat: Always emit awareness regardless of which chat is currently open
    /// Frontend will decide whether to show it based on which chat is visible
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

        // Note: No is_current_note check - always emit for chat
        // Frontend will handle filtering based on which chat is visible

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
        // Map folders to the same format as handle_get_folders for frontend consistency
        let folder_responses: Vec<serde_json::Value> = folders
            .into_iter()
            .map(|folder| {
                json!({
                    "id": folder.id,
                    "name": folder.name,
                    "description": folder.description.unwrap_or_default(),
                    "default": folder.default_folder,
                })
            })
            .collect();

        if let Err(e) = self.emit_json(
            "folders-added-notification",
            json!({ "folders": folder_responses }),
        ) {
            error!("Failed to emit added notification: {}", e);
        }
    }
}
