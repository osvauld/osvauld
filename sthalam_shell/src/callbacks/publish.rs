//! Publish callbacks
//!
//! Handles space publishing: connected nodes, publish, shareable link, copy.

use std::collections::HashSet;
use std::sync::Arc;

use butler::Butler;
use courier::CourierHandle;
use slint::ComponentHandle;
use tokio::sync::RwLock;

use crate::utils::sovereign_node_to_node_info_with_published;
use crate::Shell;

/// Register publish-related callbacks on the shell
pub fn register(
    shell: &Shell,
    butler: Arc<Butler>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    register_request_connected_nodes(shell, butler);
    register_publish_space(shell, courier_handle.clone(), tokio_handle.clone());
    register_copy_shareable_link(shell);
    register_get_shareable_link(shell, courier_handle, tokio_handle);
}

fn register_request_connected_nodes(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_request_connected_nodes(move || {
        let current_space_id = if let Some(shell) = shell_weak.upgrade() {
            shell.get_current_space_id().to_string()
        } else {
            return;
        };

        println!(
            "Requesting connected nodes for space {}...",
            current_space_id
        );

        let published_node_dids: HashSet<String> = if !current_space_id.is_empty() {
            butler
                .publish()
                .get_nodes_with_permits(&current_space_id)
                .unwrap_or_default()
                .into_iter()
                .collect()
        } else {
            HashSet::new()
        };

        println!(
            "Found {} nodes with space permits",
            published_node_dids.len()
        );

        let nodes = butler.nodes().list().unwrap_or_default();
        let connected_nodes: Vec<crate::NodeInfo> = nodes
            .iter()
            .filter(|n| n.is_connected)
            .map(|n| sovereign_node_to_node_info_with_published(n, &published_node_dids))
            .collect();

        println!("Found {} connected nodes", connected_nodes.len());

        if let Some(shell) = shell_weak.upgrade() {
            shell.set_connected_nodes(slint::ModelRc::new(slint::VecModel::from(connected_nodes)));
        }
    });
}

fn register_publish_space(
    shell: &Shell,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    let shell_weak = shell.as_weak();

    shell.on_publish_space(move |space_id, node_id| {
        println!("Publishing space {} to node {}", space_id, node_id);

        let space_id_str = space_id.to_string();
        let node_id_str = node_id.to_string();
        let courier = courier_handle.clone();
        let shell_weak = shell_weak.clone();

        if let Some(shell) = shell_weak.upgrade() {
            shell.set_publishing(true);
        }

        tokio_handle.spawn(async move {
            let courier_guard = courier.read().await;
            if let Some(handle) = courier_guard.as_ref() {
                match handle.publish_space(&space_id_str, &node_id_str).await {
                    Ok(_) => {
                        println!("Publish initiated for space {}", space_id_str);
                    }
                    Err(e) => {
                        println!("Publish failed immediately: {}", e);
                        let err_msg = format!("Publish failed: {}", e);
                        slint::invoke_from_event_loop(move || {
                            if let Some(shell) = shell_weak.upgrade() {
                                shell.set_publishing(false);
                                shell.set_toast_message(err_msg.into());
                                shell.set_toast_is_error(true);
                                shell.set_toast_visible(true);
                            }
                        })
                        .ok();
                    }
                }
            } else {
                println!("P2P not initialized");
                slint::invoke_from_event_loop(move || {
                    if let Some(shell) = shell_weak.upgrade() {
                        shell.set_publishing(false);
                        shell.set_toast_message("P2P not initialized".into());
                        shell.set_toast_is_error(true);
                        shell.set_toast_visible(true);
                    }
                })
                .ok();
            }
        });
    });
}

/// Copy text to clipboard.
///
/// On Linux, prefers wl-copy over arboard because arboard's X11/XWayland backend
/// drops the clipboard owner too quickly, causing clipboard manager handoff timeouts.
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};

        if let Ok(mut child) = Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                if stdin.write_all(text.as_bytes()).is_ok() {
                    drop(stdin);
                    if child.wait().is_ok() {
                        return Ok(());
                    }
                }
            }
        }
    }

    // Fallback to arboard (works well on macOS/Windows)
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        if clipboard.set_text(text).is_ok() {
            return Ok(());
        }
    }

    Err("No clipboard backend available".to_string())
}

fn register_copy_shareable_link(shell: &Shell) {
    let shell_weak = shell.as_weak();
    shell.on_copy_shareable_link(move || {
        if let Some(shell) = shell_weak.upgrade() {
            let link = shell.get_shareable_link().to_string();
            if !link.is_empty() {
                match copy_to_clipboard(&link) {
                    Ok(()) => {
                        shell.set_toast_message("Link copied to clipboard".into());
                        shell.set_toast_is_error(false);
                        shell.set_toast_visible(true);
                    }
                    Err(e) => {
                        println!("Clipboard error: {}", e);
                        shell.set_toast_message("Failed to copy to clipboard".into());
                        shell.set_toast_is_error(true);
                        shell.set_toast_visible(true);
                    }
                }
            }
        }
    });
}

fn register_get_shareable_link(
    shell: &Shell,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    let shell_weak = shell.as_weak();

    shell.on_get_shareable_link(move |space_id, node_id| {
        println!(
            "Requesting shareable link for space {} from node {}",
            space_id, node_id
        );

        let space_id_str = space_id.to_string();
        let node_id_str = node_id.to_string();
        let courier = courier_handle.clone();
        let shell_weak = shell_weak.clone();

        if let Some(shell) = shell_weak.upgrade() {
            shell.set_requesting_link(true);
        }

        tokio_handle.spawn(async move {
            let courier_guard = courier.read().await;
            if let Some(handle) = courier_guard.as_ref() {
                match handle.get_shareable_link(&space_id_str, &node_id_str).await {
                    Ok(connection_string) => {
                        println!("Shareable link received for space {}", space_id_str);
                        slint::invoke_from_event_loop(move || {
                            if let Some(shell) = shell_weak.upgrade() {
                                shell.set_requesting_link(false);
                                shell.set_shareable_link(connection_string.into());
                                shell.set_toast_message("Shareable link ready".into());
                                shell.set_toast_is_error(false);
                                shell.set_toast_visible(true);
                            }
                        })
                        .ok();
                    }
                    Err(e) => {
                        println!("Get shareable link failed: {}", e);
                        let err_msg = format!("Failed to get link: {}", e);
                        slint::invoke_from_event_loop(move || {
                            if let Some(shell) = shell_weak.upgrade() {
                                shell.set_requesting_link(false);
                                shell.set_toast_message(err_msg.into());
                                shell.set_toast_is_error(true);
                                shell.set_toast_visible(true);
                            }
                        })
                        .ok();
                    }
                }
            } else {
                println!("P2P not initialized");
                slint::invoke_from_event_loop(move || {
                    if let Some(shell) = shell_weak.upgrade() {
                        shell.set_requesting_link(false);
                        shell.set_toast_message("P2P not initialized".into());
                        shell.set_toast_is_error(true);
                        shell.set_toast_visible(true);
                    }
                })
                .ok();
            }
        });
    });
}
