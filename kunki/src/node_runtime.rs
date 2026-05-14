//! Node script runtime for kunki
//!
//! **Context**: Runs node.lua scripts for pages that define `entry_node` in manifest
//! **Uses**: LuaRuntime from lua_runtime with PageUpdate bridge for CRDT events

use butler::Butler;
use butler::{PageUpdate, ScribeMessage};
use domains::AppManifest;
use lua_runtime::{ActorScribeHandle, LuaCommand, LuaRuntime, LuaRuntimeConfig};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info};

/// Running node script instance
struct NodeInstance {
    cmd_tx: mpsc::Sender<LuaCommand>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

/// Manages node script instances for pages
pub struct NodeRuntimeManager {
    butler: Arc<Butler>,
    /// Running node instances by page_id
    instances: RwLock<HashMap<String, NodeInstance>>,
    /// Clock source for runtime time control
    clock: Arc<dyn domains::ClockSource>,
}

impl NodeRuntimeManager {
    pub fn new(butler: Arc<Butler>, clock: Arc<dyn domains::ClockSource>) -> Self {
        Self {
            butler,
            instances: RwLock::new(HashMap::new()),
            clock,
        }
    }

    /// Check if a page has a node script and start it
    ///
    /// **Context**: Called when a page is opened/synced
    /// **Flow**:
    /// 1. Open page to get Scribe reference
    /// 2. List apps in page
    /// 3. For each app, check manifest for entry_node
    /// 4. If present, spawn LuaRuntime with init.lua + node.lua and PageUpdate bridge
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
        let scribe_ref = self
            .butler
            .open_page(page_id)
            .await
            .map_err(|e| format!("Failed to open page: {}", e))?;

        // List apps in this page
        let apps = self
            .butler
            .apps()
            .list(page_id)
            .map_err(|e| format!("Failed to list apps: {}", e))?;

        if apps.is_empty() {
            return Err("Page has no apps".to_string());
        }

        // Check each app for entry_node
        for app_name in apps {
            // Get app files from Scribe
            let files = get_app_files_from_scribe(&scribe_ref, &app_name)
                .await
                .map_err(|e| format!("Failed to get app files: {}", e))?;

            // Parse manifest
            let manifest_str = files
                .get("manifest.json")
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
            let node_script = files.get(&node_script_name).ok_or_else(|| {
                format!("Node script '{}' not found in app files", node_script_name)
            })?;

            // Get init.lua if present (derivation rules)
            let init_code = files.get("init.lua").cloned();

            // Bundle init.lua + node.lua into single lua_code string
            let lua_code = match &init_code {
                Some(init) => format!("{}\n{}", init, node_script),
                None => node_script.clone(),
            };

            // Get identity info for permit bindings
            let identity = self
                .butler
                .get_identity()
                .await
                .map_err(|e| format!("Failed to get identity: {}", e))?;

            let identity_data = self
                .butler
                .identity_data()
                .ok()
                .flatten()
                .ok_or_else(|| "Identity data not found".to_string())?;

            // Spawn LuaRuntime with ui_enabled: false
            let (thread, cmd_tx) = LuaRuntime::spawn(LuaRuntimeConfig {
                page_id: page_id.to_string(),
                app_name: app_name.clone(),
                scribe: ActorScribeHandle::new(scribe_ref.clone()),
                user_did: identity.did().to_string(),
                user_name: identity_data.username.clone(),
                user_role: "node".to_string(),
                lua_code,
                ui_enabled: false,
                ui_tx: None,
                query_tx: None,
                navigate_tx: None,
                clock: self.clock.clone(),
            })?;

            // Trigger derivation rebuild if init.lua was loaded
            if init_code.is_some() {
                cmd_tx
                    .send(LuaCommand::RebuildDerivation)
                    .await
                    .map_err(|_| "Failed to send RebuildDerivation command")?;
                info!(page_id = %page_id, "Sent RebuildDerivation after init.lua");
            }

            // Subscribe to page updates from Scribe
            let (page_update_tx, mut page_update_rx) = mpsc::channel(256);
            scribe_ref
                .cast(ScribeMessage::SubscribeToPageUpdates { tx: page_update_tx })
                .map_err(|e| format!("Failed to subscribe to page updates: {}", e))?;

            // Spawn async bridge: forwards PageUpdate → LuaCommand
            let bridge_cmd_tx = cmd_tx.clone();
            let bridge_page_id = page_id.to_string();
            tokio::spawn(async move {
                while let Some(update) = page_update_rx.recv().await {
                    match update {
                        PageUpdate::LayerChanged {
                            layer,
                            full_data,
                            delta,
                            created,
                            dynamic_ref,
                            ..
                        } => {
                            let _ = bridge_cmd_tx
                                .send(LuaCommand::LayerChanged {
                                    layer_name: layer,
                                    created,
                                    delta,
                                    full_data,
                                    dynamic_ref,
                                })
                                .await;
                        }
                        PageUpdate::Ephemeral {
                            user_did, payload, ..
                        } => {
                            let _ = bridge_cmd_tx
                                .send(LuaCommand::Ephemeral { user_did, payload })
                                .await;
                        }
                        PageUpdate::StructuredEphemeral {
                            from_did,
                            func,
                            args,
                        } => {
                            let _ = bridge_cmd_tx
                                .send(LuaCommand::StructuredEphemeral {
                                    from_did,
                                    func,
                                    args,
                                })
                                .await;
                        }
                        PageUpdate::PeerSubscribed { did, .. } => {
                            let _ = bridge_cmd_tx
                                .send(LuaCommand::PeerJoined { user_did: did })
                                .await;
                        }
                        PageUpdate::PeerUnsubscribed { did } => {
                            let _ = bridge_cmd_tx
                                .send(LuaCommand::PeerLeft { user_did: did })
                                .await;
                        }
                        PageUpdate::QueryUpdated { .. } => {}
                    }
                }
                debug!(page_id = %bridge_page_id, "PageUpdate bridge ended");
            });

            // Store instance
            {
                let mut instances = self.instances.write().await;
                instances.insert(
                    page_id.to_string(),
                    NodeInstance {
                        cmd_tx,
                        thread_handle: Some(thread),
                    },
                );
            }

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

        if let Some(mut instance) = instance {
            let _ = instance.cmd_tx.send(LuaCommand::Shutdown).await;
            if let Some(handle) = instance.thread_handle.take() {
                let _ = handle.join();
            }
            info!(page_id = %page_id, "Node script stopped");
        }

        Ok(())
    }

    /// Get command channel for a running node script (for test-time controls)
    pub async fn get_cmd_tx(&self, page_id: &str) -> Option<mpsc::Sender<LuaCommand>> {
        let instances = self.instances.read().await;
        instances.get(page_id).map(|inst| inst.cmd_tx.clone())
    }

    /// Stop all running node scripts
    pub async fn stop_all(&self) {
        let instances: Vec<_> = {
            let mut instances = self.instances.write().await;
            instances.drain().collect()
        };

        for (page_id, mut instance) in instances {
            let _ = instance.cmd_tx.send(LuaCommand::Shutdown).await;
            if let Some(handle) = instance.thread_handle.take() {
                let _ = handle.join();
            }
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
