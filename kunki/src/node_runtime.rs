//! Node script runtime for kunki
//!
//! **Context**: Runs node.lua scripts for pages that define `entry_node` in manifest
//! **Uses**: HeadlessRuntime from butler with tick loop and ephemeral support

use butler::runtime::HeadlessRuntime;
use butler::scribe::ScribeMessage;
use butler::Butler;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Manifest structure for parsing entry_node
#[derive(Debug, serde::Deserialize)]
struct AppManifest {
    name: String,
    #[serde(default)]
    entry_node: Option<String>,
    #[serde(default)]
    tick_enabled: bool,
}

/// Running node script instance
struct NodeInstance {
    page_id: String,
    app_name: String,
    shutdown_tx: mpsc::Sender<()>,
}

/// Manages node script instances for pages
pub struct NodeRuntimeManager {
    butler: Arc<Butler>,
    /// Running node instances by page_id
    instances: RwLock<HashMap<String, NodeInstance>>,
}

impl NodeRuntimeManager {
    pub fn new(butler: Arc<Butler>) -> Self {
        Self {
            butler,
            instances: RwLock::new(HashMap::new()),
        }
    }

    /// Check if a page has a node script and start it
    ///
    /// **Context**: Called when a page is opened/synced
    /// **Flow**:
    /// 1. Open page to get Scribe reference
    /// 2. List apps in page
    /// 3. For each app, check manifest for entry_node
    /// 4. If present, load node script and start tick loop
    pub async fn start_node_for_page(&self, page_id: &str) -> Result<(), String> {
        // Check if already running
        {
            let instances = self.instances.read().await;
            if instances.contains_key(page_id) {
                debug!(page_id = %page_id, "Node script already running");
                return Ok(());
            }
        }

        // Open page to get scribe reference
        let scribe_ref = self.butler.open_page(page_id).await
            .map_err(|e| format!("Failed to open page: {}", e))?;

        // List apps in this page
        let apps = self.butler.list_apps(page_id)
            .map_err(|e| format!("Failed to list apps: {}", e))?;

        if apps.is_empty() {
            return Err("Page has no apps".to_string());
        }

        // Check each app for entry_node
        for app_name in apps {
            // Get app files from Scribe
            let files = get_app_files_from_scribe(&scribe_ref, &app_name).await
                .map_err(|e| format!("Failed to get app files: {}", e))?;

            // Parse manifest
            let manifest_str = files.get("manifest.json")
                .ok_or_else(|| "No manifest.json in app".to_string())?;

            let manifest: AppManifest = serde_json::from_str(manifest_str)
                .map_err(|e| format!("Failed to parse manifest: {}", e))?;

            // Check if entry_node is defined
            let node_script_name = match manifest.entry_node {
                Some(name) => name,
                None => {
                    debug!(page_id = %page_id, app_name = %app_name, "No entry_node in manifest");
                    continue;
                }
            };

            info!(page_id = %page_id, app_name = %app_name, node_script = %node_script_name, "Starting node script");

            // Get node script content
            let node_script = files.get(&node_script_name)
                .ok_or_else(|| format!("Node script '{}' not found in app files", node_script_name))?;

            // Get identity info for permit bindings
            let identity = self.butler.get_identity().await
                .map_err(|e| format!("Failed to get identity: {}", e))?;

            let identity_data = self.butler.identity_data()
                .ok()
                .flatten()
                .ok_or_else(|| "Identity data not found".to_string())?;

            // Create HeadlessRuntime
            let mut runtime = HeadlessRuntime::new(
                page_id,
                scribe_ref.clone(),
                identity.did(),
                &identity_data.username,
                "node",  // Node has special role
            ).await?;

            // Load node script
            runtime.load_code(node_script)?;

            // Enable tick (nodes always have tick enabled for game loops)
            runtime.enable_tick();

            // Create shutdown channel
            let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

            // Store instance
            {
                let mut instances = self.instances.write().await;
                instances.insert(page_id.to_string(), NodeInstance {
                    page_id: page_id.to_string(),
                    app_name: app_name.clone(),
                    shutdown_tx,
                });
            }

            // Spawn tick loop in background
            let page_id_owned = page_id.to_string();
            let app_name_owned = app_name.clone();
            tokio::spawn(async move {
                info!(page_id = %page_id_owned, app_name = %app_name_owned, "Node script tick loop starting");

                // Run at 30fps for node (sufficient for game logic)
                match runtime.run_tick_loop(30, shutdown_rx).await {
                    Ok(()) => info!(page_id = %page_id_owned, "Node script tick loop ended"),
                    Err(e) => error!(page_id = %page_id_owned, error = %e, "Node script tick loop error"),
                }
            });

            info!(page_id = %page_id, app_name = %app_name, "Node script started successfully");

            // Only start one node script per page
            return Ok(());
        }

        // No entry_node found in any app
        Err("No entry_node in manifest".to_string())
    }

    /// Stop node script for a page
    pub async fn stop_node_for_page(&self, page_id: &str) -> Result<(), String> {
        let instance = {
            let mut instances = self.instances.write().await;
            instances.remove(page_id)
        };

        if let Some(instance) = instance {
            instance.shutdown_tx.send(()).await
                .map_err(|_| "Failed to send shutdown signal")?;
            info!(page_id = %page_id, "Node script stopped");
        }

        Ok(())
    }

    /// Stop all running node scripts
    pub async fn stop_all(&self) {
        let instances: Vec<_> = {
            let mut instances = self.instances.write().await;
            instances.drain().collect()
        };

        for (page_id, instance) in instances {
            let _ = instance.shutdown_tx.send(()).await;
            info!(page_id = %page_id, "Node script stopped");
        }
    }

    /// Get list of running node script page IDs
    pub async fn running_pages(&self) -> Vec<String> {
        let instances = self.instances.read().await;
        instances.keys().cloned().collect()
    }
}

/// Get app files from Scribe's in-memory layer
async fn get_app_files_from_scribe(
    scribe: &ractor::ActorRef<ScribeMessage>,
    app_name: &str,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error + Send + Sync>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    scribe.cast(ScribeMessage::GetAppFiles {
        app_name: app_name.to_string(),
        reply: tx,
    })?;
    let result = rx.await??;
    Ok(result)
}
