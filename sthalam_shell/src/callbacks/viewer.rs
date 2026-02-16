//! Viewer connection callbacks
//!
//! Handles viewer flow: add website using shareable link.

use std::sync::Arc;

use butler::{Butler, ConnectionStringExt};
use courier::CourierHandle;
use slint::ComponentHandle;
use tokio::sync::RwLock;

use crate::Shell;

/// Register viewer connection callbacks on the shell
pub fn register(
    shell: &Shell,
    butler: Arc<Butler>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    register_add_website(shell, butler, courier_handle, tokio_handle);
}

fn register_add_website(
    shell: &Shell,
    butler: Arc<Butler>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    let shell_weak = shell.as_weak();

    shell.on_add_website(move |connection_string| {
        println!("Adding website with connection string...");

        let conn_str = connection_string.to_string();
        let butler = butler.clone();
        let courier = courier_handle.clone();
        let shell_weak = shell_weak.clone();

        if let Some(shell) = shell_weak.upgrade() {
            shell.set_connecting_website(true);
        }

        tokio_handle.spawn(async move {
            // 1. Parse connection string
            let conn = match butler.nodes().parse_connection_string(&conn_str) {
                Ok(c) => c,
                Err(e) => {
                    println!("Failed to parse connection string: {}", e);
                    let err_msg = format!("Invalid connection string: {}", e);
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting_website(false);
                            shell.set_toast_message(err_msg.into());
                            shell.set_toast_is_error(true);
                            shell.set_toast_visible(true);
                        }
                    })
                    .ok();
                    return;
                }
            };

            let node_id = conn.node_id();
            let permit = conn.permit.clone();

            println!("Parsed connection: node_id={}, name={}", node_id, conn.name);

            // 2. Extract space_id from permit
            let space_id = match conn.space_id() {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get space_id: {}", e);
                    let err_msg = format!("Invalid permit: {}", e);
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting_website(false);
                            shell.set_toast_message(err_msg.into());
                            shell.set_toast_is_error(true);
                            shell.set_toast_visible(true);
                        }
                    })
                    .ok();
                    return;
                }
            };

            println!("Space ID from permit: {}", space_id);

            // 3. Get courier handle
            let courier_guard = courier.read().await;
            let courier_handle = match courier_guard.as_ref() {
                Some(h) => h.clone(),
                None => {
                    println!("P2P not initialized");
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting_website(false);
                            shell.set_toast_message("P2P not initialized".into());
                            shell.set_toast_is_error(true);
                            shell.set_toast_visible(true);
                        }
                    })
                    .ok();
                    return;
                }
            };
            drop(courier_guard);

            // 4. Store node as Contact (before connect - we have all the info)
            if let Ok(node_did) = conn.node_did() {
                if let Err(e) = butler.contacts().add_node(
                    &node_did,
                    &conn.node_encryption_key,
                    &conn.name,
                    &node_id,
                    &permit,
                ) {
                    println!("Warning: Failed to store node contact: {}", e);
                }
            } else {
                println!("Warning: Could not derive node DID, skipping contact storage");
            }

            // 5. Initiate connection (fire-and-forget)
            if let Err(e) = courier_handle.connect(&node_id, &permit) {
                println!("Failed to initiate connection: {}", e);
                let err_msg = format!("Connection failed: {}", e);
                slint::invoke_from_event_loop(move || {
                    if let Some(shell) = shell_weak.upgrade() {
                        shell.set_connecting_website(false);
                        shell.set_toast_message(err_msg.into());
                        shell.set_toast_is_error(true);
                        shell.set_toast_visible(true);
                    }
                })
                .ok();
                return;
            }

            // 6. Retry request_space_as_viewer until accepted or timeout
            // PeerActor queues SpaceRequest until handshake completes, so we don't need
            // to wait for authentication - just retry until the request is accepted.
            let request_timeout = std::time::Duration::from_secs(15);
            let retry_interval = std::time::Duration::from_millis(200);
            let start = std::time::Instant::now();
            
            loop {
                match courier_handle
                    .request_space_as_viewer(&space_id, &node_id, &permit)
                    .await
                {
                    Ok(_) => {
                        println!("Space request sent, waiting for sync...");
                        // UI update happens via ViewerSyncComplete event in events.rs
                        break;
                    }
                    Err(e) if start.elapsed() < request_timeout => {
                        // Retry - PeerActor might not be spawned yet or handshake in progress
                        println!("Space request failed ({}), retrying...", e);
                        tokio::time::sleep(retry_interval).await;
                    }
                    Err(e) => {
                        println!("Failed to request space after retries: {}", e);
                        let err_msg = format!("Failed to request space: {}", e);
                        slint::invoke_from_event_loop(move || {
                            if let Some(shell) = shell_weak.upgrade() {
                                shell.set_connecting_website(false);
                                shell.set_toast_message(err_msg.into());
                                shell.set_toast_is_error(true);
                                shell.set_toast_visible(true);
                            }
                        })
                        .ok();
                        return;
                    }
                }
            }
        });
    });
}
