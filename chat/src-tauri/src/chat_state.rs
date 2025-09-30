use log::{error, info};
use osvauld_core::models::document::YjsDocExt;
use persistance::database::RepositoryContext;
use services::get_shared_user_devices_for_note;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use yrs::Doc;

/// Per-resource buffer information
#[derive(Debug, Clone)]
struct ResourceBuffers {
    #[allow(dead_code)]
    resource_id: String,
    main_doc: Doc,
    image_doc: Doc,
}

impl ResourceBuffers {
    fn new(resource_id: String) -> Self {
        Self {
            resource_id,
            main_doc: Doc::new(),
            image_doc: Doc::new(),
        }
    }
}

/// Subscription cache entry
#[derive(Debug, Clone)]
struct SubscriptionCache {
    device_ids: Vec<String>,
    last_updated: std::time::Instant,
}

/// Chat state managing multiple simultaneous chat resources
#[derive(Clone)]
pub struct ChatState {
    /// Per-resource document buffers: resource_id -> ResourceBuffers
    buffers: Arc<RwLock<HashMap<String, ResourceBuffers>>>,
    
    /// Subscription cache: resource_id -> device_ids
    /// Cached to avoid repeated database queries
    subscription_cache: Arc<RwLock<HashMap<String, SubscriptionCache>>>,
    
    /// Currently open chat (for UI context only, not for routing decisions)
    current_chat_id: Arc<RwLock<Option<String>>>,
    
    /// Repository context for database access
    repo_ctx: Arc<RepositoryContext>,
    
    /// Current user and device IDs
    current_user_id: Arc<RwLock<Option<String>>>,
    current_device_id: Arc<RwLock<Option<String>>>,
}

impl ChatState {
    pub fn new(repo_ctx: Arc<RepositoryContext>) -> Self {
        Self {
            buffers: Arc::new(RwLock::new(HashMap::new())),
            subscription_cache: Arc::new(RwLock::new(HashMap::new())),
            current_chat_id: Arc::new(RwLock::new(None)),
            repo_ctx,
            current_user_id: Arc::new(RwLock::new(None)),
            current_device_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Set current user and device (called on initialization)
    pub async fn set_current_user(&self, user_id: String, device_id: String) {
        *self.current_user_id.write().await = Some(user_id.clone());
        *self.current_device_id.write().await = Some(device_id.clone());
        info!("Set current user: {} and device: {}", user_id, device_id);
    }

    /// Set the currently open chat (for UI context)
    pub async fn set_current_chat(&self, chat_id: Option<String>) {
        *self.current_chat_id.write().await = chat_id.clone();
        info!("Current chat set to: {:?}", chat_id);
    }

    /// Get the currently open chat
    pub async fn get_current_chat(&self) -> Option<String> {
        self.current_chat_id.read().await.clone()
    }

    /// Load a chat resource into buffers
    /// This will create new docs and populate them with the chat's state
    pub async fn load_chat_resource(
        &self,
        resource_id: String,
        main_doc_state: Option<Vec<u8>>,
        image_state: Option<Vec<u8>>,
    ) {
        let mut buffers = ResourceBuffers::new(resource_id.clone());

        // Load main document state if provided
        if let Some(main_state) = main_doc_state {
            if !main_state.is_empty() {
                if let Err(e) = buffers.main_doc.apply_update_v2(&main_state).await {
                    error!("Failed to load main document state: {}", e);
                }
            }
        }

        // Load image state if provided
        if let Some(img_state) = image_state {
            if !img_state.is_empty() {
                if let Err(e) = buffers.image_doc.apply_update_v2(&img_state).await {
                    error!("Failed to load image state: {}", e);
                }
            }
        }

        // Store in buffers map
        self.buffers.write().await.insert(resource_id.clone(), buffers);
        info!("Loaded chat resource into buffers: {}", resource_id);
    }

    /// Apply updates to a specific chat resource
    pub async fn apply_update(&self, resource_id: &str, new_updates: Vec<u8>, doc_type: &str) {
        if new_updates.is_empty() {
            return;
        }

        // Determine which doc to update based on doc_type
        let is_image_doc = matches!(doc_type, "images" | "image_state");

        // Get or create buffers for this resource
        let mut buffers_map = self.buffers.write().await;
        let resource_buffers = buffers_map
            .entry(resource_id.to_string())
            .or_insert_with(|| ResourceBuffers::new(resource_id.to_string()));

        // Clone the doc to apply updates outside the lock
        let mut temp_doc = if is_image_doc {
            resource_buffers.image_doc.clone()
        } else {
            resource_buffers.main_doc.clone()
        };

        // Release the lock before the async operation
        drop(buffers_map);

        // Apply updates to the temporary doc (no lock held here)
        if let Err(e) = temp_doc.apply_update_v2(&new_updates).await {
            error!("Failed to apply updates to {} document: {}", doc_type, e);
            return;
        }

        // Quick swap with minimal lock time
        let mut buffers_map = self.buffers.write().await;
        if let Some(resource_buffers) = buffers_map.get_mut(resource_id) {
            if is_image_doc {
                resource_buffers.image_doc = temp_doc;
            } else {
                resource_buffers.main_doc = temp_doc;
            }
        }
        
        info!("Applied {} bytes to {} document for resource: {}", 
              new_updates.len(), doc_type, resource_id);
    }

    /// Get state vectors for a specific chat resource
    pub async fn get_state_vectors(&self, resource_id: &str) -> Result<String, String> {
        let buffers = self.buffers.read().await;
        
        let resource_buffers = buffers
            .get(resource_id)
            .ok_or_else(|| format!("No buffers found for resource: {}", resource_id))?;

        let mut result = serde_json::Map::new();

        // Get state vector for main doc
        let main_sv = resource_buffers.main_doc.get_state_vector_v2().await;
        if !main_sv.is_empty() {
            result.insert(
                "chat".to_string(),  // Chat resource type uses "chat" key
                serde_json::Value::Object(Self::create_doc_result(vec![], main_sv)),
            );
        }

        // Get state vector for image doc
        let image_sv = resource_buffers.image_doc.get_state_vector_v2().await;
        if !image_sv.is_empty() {
            result.insert(
                "image_state".to_string(),
                serde_json::Value::Object(Self::create_doc_result(vec![], image_sv)),
            );
        }

        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize state vectors: {}", e))
    }

    /// Export full document state as bytes (for preview generation)
    /// Returns the complete state of the main_doc for a resource
    pub async fn export_document_state(&self, resource_id: &str) -> Result<Vec<u8>, String> {
        let buffers = self.buffers.read().await;
        
        let resource_buffers = buffers
            .get(resource_id)
            .ok_or_else(|| format!("No buffers found for resource: {}", resource_id))?;

        // Export the full state of the main document
        use yrs::{StateVector, Transact, ReadTxn};
        let txn = resource_buffers.main_doc.transact();
        let empty_state_vector = StateVector::default();
        let full_state = txn.encode_state_as_update_v2(&empty_state_vector);
        
        Ok(full_state.to_vec())
    }

    /// Generate updates for a peer based on their state vectors
    pub async fn generate_updates_for_peer(
        &self,
        resource_id: &str,
        peer_state_vectors_json: &str,
    ) -> Result<String, String> {
        let peer_data: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(peer_state_vectors_json)
                .map_err(|e| format!("Failed to parse peer state vectors: {}", e))?;

        let buffers = self.buffers.read().await;
        let resource_buffers = buffers
            .get(resource_id)
            .ok_or_else(|| format!("No buffers found for resource: {}", resource_id))?;

        let mut result = serde_json::Map::new();

        // Process main_doc (called "chat" for Chat resource type)
        Self::process_doc_state_vectors(
            &resource_buffers.main_doc,
            peer_data.get("chat"),
            "chat",
            &mut result,
        )
        .await?;

        // Process image_state
        Self::process_doc_state_vectors(
            &resource_buffers.image_doc,
            peer_data.get("image_state"),
            "image_state",
            &mut result,
        )
        .await?;

        serde_json::to_string(&result).map_err(|e| format!("Failed to serialize updates: {}", e))
    }

    /// Apply updates from peer and generate diff
    pub async fn apply_updates_and_generate_diff(
        &self,
        resource_id: &str,
        peer_updates_json: &str,
    ) -> Result<String, String> {
        let peer_data: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(peer_updates_json)
                .map_err(|e| format!("Failed to parse peer updates: {}", e))?;

        let mut result = serde_json::Map::new();

        // Get buffers
        let buffers = self.buffers.read().await;
        let resource_buffers = buffers
            .get(resource_id)
            .ok_or_else(|| format!("No buffers found for resource: {}", resource_id))?;

        // Process main_doc (called "chat" for Chat resource type)
        let main_doc = resource_buffers.main_doc.clone();
        drop(buffers);

        if let Some(updated_doc) = Self::process_doc_updates_and_diff(
            main_doc,
            peer_data.get("chat"),
            "chat",
            &mut result,
        )
        .await?
        {
            let mut buffers = self.buffers.write().await;
            if let Some(rb) = buffers.get_mut(resource_id) {
                rb.main_doc = updated_doc;
            }
        }

        // Process image_doc
        let buffers = self.buffers.read().await;
        let image_doc = buffers
            .get(resource_id)
            .map(|rb| rb.image_doc.clone())
            .ok_or_else(|| format!("No buffers found for resource: {}", resource_id))?;
        drop(buffers);

        if let Some(updated_doc) = Self::process_doc_updates_and_diff(
            image_doc,
            peer_data.get("image_state"),
            "image_state",
            &mut result,
        )
        .await?
        {
            let mut buffers = self.buffers.write().await;
            if let Some(rb) = buffers.get_mut(resource_id) {
                rb.image_doc = updated_doc;
            }
        }

        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize diff updates: {}", e))
    }

    /// Apply peer updates without generating a response
    pub async fn apply_peer_updates(
        &self,
        resource_id: &str,
        peer_updates_json: &str,
    ) -> Result<(), String> {
        let peer_data: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(peer_updates_json)
                .map_err(|e| format!("Failed to parse peer updates: {}", e))?;

        // Process main_doc updates (called "chat" for Chat resource type)
        if let Some(main_data) = peer_data.get("chat") {
            let peer_updates = Self::parse_byte_array(main_data.get("updates"));

            if !peer_updates.is_empty() {
                self.apply_update(resource_id, peer_updates, "chat").await;
            }
        }

        // Process image_state updates
        if let Some(image_data) = peer_data.get("image_state") {
            let peer_updates = Self::parse_byte_array(image_data.get("updates"));

            if !peer_updates.is_empty() {
                self.apply_update(resource_id, peer_updates, "image_state").await;
            }
        }

        Ok(())
    }

    /// Get or fetch subscribers for a chat resource (with caching)
    pub async fn get_subscribers(&self, resource_id: &str) -> Result<Vec<String>, String> {
        // Check cache first
        let cache = self.subscription_cache.read().await;
        if let Some(cached) = cache.get(resource_id) {
            // Cache valid for 5 minutes
            if cached.last_updated.elapsed().as_secs() < 300 {
                info!("Using cached subscribers for resource: {}", resource_id);
                return Ok(cached.device_ids.clone());
            }
        }
        drop(cache);

        // Cache miss or expired, fetch from database
        info!("Fetching subscribers from database for resource: {}", resource_id);
        
        let user_id = self.current_user_id.read().await.clone()
            .ok_or_else(|| "Current user ID not set".to_string())?;
        let device_id = self.current_device_id.read().await.clone()
            .ok_or_else(|| "Current device ID not set".to_string())?;

        let (device_ids, _user_ids) = get_shared_user_devices_for_note(
            resource_id,
            &user_id,
            &device_id,
            true,
            self.repo_ctx.clone(),
        )
        .await
        .map_err(|e| format!("Failed to get shared devices: {}", e))?;

        // Update cache
        let cache_entry = SubscriptionCache {
            device_ids: device_ids.clone(),
            last_updated: std::time::Instant::now(),
        };
        self.subscription_cache.write().await.insert(resource_id.to_string(), cache_entry);

        info!("Cached {} subscribers for resource: {}", device_ids.len(), resource_id);
        Ok(device_ids)
    }

    /// Invalidate subscription cache for a resource (call when sharing changes)
    pub async fn invalidate_subscription_cache(&self, resource_id: &str) {
        self.subscription_cache.write().await.remove(resource_id);
        info!("Invalidated subscription cache for resource: {}", resource_id);
    }

    /// Clear all subscription caches
    pub async fn clear_subscription_cache(&self) {
        self.subscription_cache.write().await.clear();
        info!("Cleared all subscription caches");
    }

    // Helper functions (similar to CurrentNoteState)

    fn parse_byte_array(value: Option<&serde_json::Value>) -> Vec<u8> {
        value
            .and_then(|v| v.as_array())
            .map(|array| {
                array
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn bytes_to_json_array(bytes: &[u8]) -> Vec<serde_json::Value> {
        bytes
            .iter()
            .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
            .collect()
    }

    fn create_doc_result(
        updates: Vec<u8>,
        state_vector: Vec<u8>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut doc_result = serde_json::Map::new();
        doc_result.insert(
            "updates".to_string(),
            serde_json::Value::Array(Self::bytes_to_json_array(&updates)),
        );
        doc_result.insert(
            "state_vector".to_string(),
            serde_json::Value::Array(Self::bytes_to_json_array(&state_vector)),
        );
        doc_result
    }

    async fn process_doc_state_vectors(
        doc: &Doc,
        doc_data: Option<&serde_json::Value>,
        doc_name: &str,
        result: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(data) = doc_data {
            let peer_state_vector = Self::parse_byte_array(data.get("state_vector"));

            if !peer_state_vector.is_empty() {
                let updates_for_peer = doc
                    .get_diff_update_v2(&peer_state_vector)
                    .await
                    .map_err(|e| format!("Failed to generate {} diff: {}", doc_name, e))?;

                let our_state_vector = doc.get_state_vector_v2().await;

                result.insert(
                    doc_name.to_string(),
                    serde_json::Value::Object(Self::create_doc_result(
                        updates_for_peer,
                        our_state_vector,
                    )),
                );
            }
        }
        Ok(())
    }

    async fn process_doc_updates_and_diff(
        doc: Doc,
        doc_data: Option<&serde_json::Value>,
        doc_name: &str,
        result: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<Option<Doc>, String> {
        if let Some(data) = doc_data {
            let peer_updates = Self::parse_byte_array(data.get("updates"));
            let peer_state_vector = Self::parse_byte_array(data.get("state_vector"));

            let mut doc = doc;

            // Apply peer updates if any
            if !peer_updates.is_empty() {
                doc.apply_update_v2(&peer_updates)
                    .await
                    .map_err(|e| format!("Failed to apply {} updates: {}", doc_name, e))?;
            }

            // Generate diff based on peer's state vector
            if !peer_state_vector.is_empty() {
                let updates_for_peer = doc
                    .get_diff_update_v2(&peer_state_vector)
                    .await
                    .map_err(|e| format!("Failed to generate {} diff: {}", doc_name, e))?;

                let our_state_vector = doc.get_state_vector_v2().await;

                result.insert(
                    doc_name.to_string(),
                    serde_json::Value::Object(Self::create_doc_result(
                        updates_for_peer,
                        our_state_vector,
                    )),
                );

                return Ok(Some(doc));
            }
        }
        Ok(None)
    }
}

