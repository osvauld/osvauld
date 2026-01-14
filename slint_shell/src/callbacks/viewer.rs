//! Viewer connection callbacks
//!
//! Handles viewer flow: add website using shareable link.

use std::sync::Arc;

use butler::Butler;
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
            let conn = match butler.parse_connection_string(&conn_str) {
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

            // 4. Connect and authenticate
            match courier_handle
                .connect_and_wait_for_auth(&node_id, &permit)
                .await
            {
                Ok(_) => println!("Connected and authenticated with node"),
                Err(e) => {
                    println!("Failed to connect: {}", e);
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
            }

            // 5. Store node as Contact for future reconnection
            if let Ok(node_did) = conn.node_did() {
                if let Err(e) = butler.add_node_contact(
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

            // 6. Request space as viewer (async - sync happens via events)
            match courier_handle
                .request_space_as_viewer(&space_id, &node_id, &permit)
                .await
            {
                Ok(_) => {
                    println!("Space request sent, waiting for sync...");
                }
                Err(e) => {
                    println!("Failed to request space: {}", e);
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
                }
            }
        });
    });
}
