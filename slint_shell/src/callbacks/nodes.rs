//! Node management callbacks
//!
//! Handles sovereign node operations: request, add, delete, connect, navigate.

use std::sync::Arc;

use butler::Butler;
use courier::CourierHandle;
use slint::ComponentHandle;
use tokio::sync::RwLock;

use crate::utils::sovereign_node_to_node_info;
use crate::Shell;

/// Register node management callbacks on the shell
pub fn register(
    shell: &Shell,
    butler: Arc<Butler>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    register_request_nodes(shell, butler.clone());
    register_add_node(shell, butler.clone(), courier_handle.clone(), tokio_handle.clone());
    register_delete_node(shell, butler.clone());
    register_connect_node(shell, butler.clone(), courier_handle, tokio_handle);
    register_navigate_to_nodes(shell);
}

fn register_request_nodes(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_request_nodes(move || {
        println!("Requesting sovereign nodes...");

        let nodes = butler.list_sovereign_nodes().unwrap_or_default();
        println!("Found {} sovereign nodes", nodes.len());

        if let Some(shell) = shell_weak.upgrade() {
            let node_infos: Vec<crate::NodeInfo> = nodes
                .iter()
                .map(sovereign_node_to_node_info)
                .collect();
            shell.set_nodes(slint::ModelRc::new(slint::VecModel::from(node_infos)));
        }
    });
}

fn register_add_node(
    shell: &Shell,
    butler: Arc<Butler>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    let shell_weak = shell.as_weak();

    shell.on_add_node(move |connection_string| {
        println!("Adding sovereign node...");
        let conn_str = connection_string.to_string();
        let butler = butler.clone();
        let courier = courier_handle.clone();
        let shell_weak = shell_weak.clone();

        // Mark as connecting
        if let Some(shell) = shell_weak.upgrade() {
            shell.set_connecting(true);
            shell.set_error_message("".into());
        }

        tokio_handle.spawn(async move {
            // 1. Parse and store node via Butler
            let node = match butler.add_sovereign_node(&conn_str) {
                Ok(n) => {
                    println!("Node stored: {} ({})", n.name, n.node_id);
                    n
                }
                Err(e) => {
                    println!("Failed to add node: {}", e);
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting(false);
                            shell.set_error_message(format!("Failed to add node: {}", e).into());
                        }
                    })
                    .ok();
                    return;
                }
            };

            // 2. Extract permit from node
            let permit = match &node.permit {
                Some(p) => p.clone(),
                None => {
                    println!("No permit in node");
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting(false);
                            shell.set_error_message("No permit in connection string".into());
                        }
                    })
                    .ok();
                    return;
                }
            };

            // 3. Connect and authenticate via Courier
            let courier_guard = courier.read().await;
            let courier_handle = match courier_guard.as_ref() {
                Some(h) => h.clone(),
                None => {
                    println!("P2P not initialized yet");
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting(false);
                            shell.set_error_message("P2P not initialized. Please login first.".into());
                        }
                    })
                    .ok();
                    return;
                }
            };
            drop(courier_guard);

            match courier_handle
                .connect_and_wait_for_auth(&node.node_id, &permit)
                .await
            {
                Ok(_) => {
                    println!("Successfully connected to node: {}", node.name);
                    let _ = butler.set_sovereign_node_connected(&node.node_id, true);

                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting(false);
                            shell.set_show_add_modal(false);
                            shell.set_connection_string("".into());

                            let nodes = butler.list_sovereign_nodes().unwrap_or_default();
                            let node_infos: Vec<crate::NodeInfo> = nodes
                                .iter()
                                .map(sovereign_node_to_node_info)
                                .collect();
                            shell.set_nodes(slint::ModelRc::new(slint::VecModel::from(node_infos)));
                        }
                    })
                    .ok();
                }
                Err(e) => {
                    println!("Failed to connect: {}", e);
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting(false);
                            shell.set_error_message(format!("Connection failed: {}", e).into());
                        }
                    })
                    .ok();
                }
            }
        });
    });
}

fn register_delete_node(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_delete_node(move |node_id| {
        println!("Deleting node: {}", node_id);

        match butler.delete_sovereign_node(&node_id) {
            Ok(true) => {
                println!("Node deleted successfully");

                if let Some(shell) = shell_weak.upgrade() {
                    let nodes = butler.list_sovereign_nodes().unwrap_or_default();
                    let node_infos: Vec<crate::NodeInfo> = nodes
                        .iter()
                        .map(sovereign_node_to_node_info)
                        .collect();
                    shell.set_nodes(slint::ModelRc::new(slint::VecModel::from(node_infos)));
                }
            }
            Ok(false) => println!("Node not found"),
            Err(e) => println!("Error deleting node: {}", e),
        }
    });
}

fn register_connect_node(
    shell: &Shell,
    butler: Arc<Butler>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    shell.on_connect_node(move |node_id| {
        println!("Reconnecting to node: {}", node_id);
        let node_id_str = node_id.to_string();
        let butler = butler.clone();
        let courier = courier_handle.clone();

        tokio_handle.spawn(async move {
            let node = match butler.get_sovereign_node(&node_id_str) {
                Ok(Some(n)) => n,
                Ok(None) => {
                    println!("Node not found: {}", node_id_str);
                    return;
                }
                Err(e) => {
                    println!("Error getting node: {}", e);
                    return;
                }
            };

            let courier_guard = courier.read().await;
            let courier_handle = match courier_guard.as_ref() {
                Some(h) => h.clone(),
                None => {
                    println!("P2P not initialized yet");
                    return;
                }
            };
            drop(courier_guard);

            match courier_handle.connect(&node.node_id).await {
                Ok(_) => {
                    println!("Reconnected successfully");
                    let _ = butler.set_sovereign_node_connected(&node_id_str, true);
                }
                Err(e) => {
                    println!("Reconnection failed: {}", e);
                }
            }
        });
    });
}

fn register_navigate_to_nodes(shell: &Shell) {
    let shell_weak = shell.as_weak();
    shell.on_navigate_to_nodes(move || {
        if let Some(shell) = shell_weak.upgrade() {
            shell.set_current_screen("nodes".into());
        }
    });
}
