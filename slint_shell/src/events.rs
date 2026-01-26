//! CourierEvent handling
//!
//! Spawns event listener for P2P events and updates UI accordingly.

use std::collections::HashSet;
use std::sync::Arc;

use butler::Butler;
use courier::CourierEvent;
use tokio::sync::mpsc::Receiver;

use crate::utils::sovereign_node_to_node_info_with_published;
use crate::Shell;

/// Spawn event listener for courier events
///
/// Handles: SpacePublished, PublishFailed, ShareableLinkReceived,
/// ViewerSpaceReceived, ViewerSyncComplete
pub fn spawn_event_listener(
    mut event_rx: Receiver<CourierEvent>,
    shell_weak: slint::Weak<Shell>,
    butler: Arc<Butler>,
) {
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                CourierEvent::PeerAuthenticated { node_id, username, .. } => {
                    println!("Peer authenticated: {} ({})", username, node_id);
                    let shell_weak = shell_weak.clone();
                    let butler = butler.clone();
                    let node_id_str = node_id.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting(false);
                            shell.set_show_add_modal(false);
                            shell.set_connection_string("".into());

                            // Mark node as connected in storage
                            let _ = butler.set_sovereign_node_connected(&node_id_str, true);

                            // Refresh nodes list
                            let nodes = butler.list_sovereign_nodes().unwrap_or_default();
                            let node_infos: Vec<crate::NodeInfo> = nodes
                                .iter()
                                .map(crate::utils::sovereign_node_to_node_info)
                                .collect();
                            shell.set_nodes(slint::ModelRc::new(slint::VecModel::from(node_infos)));

                            shell.set_toast_message(format!("Connected to {}", username).into());
                            shell.set_toast_is_error(false);
                            shell.set_toast_visible(true);
                        }
                    })
                    .ok();
                }
                CourierEvent::ConnectionFailed { node_id, error } => {
                    println!("Connection failed for {}: {}", node_id, error);
                    let shell_weak = shell_weak.clone();
                    let err_msg = format!("Connection failed: {}", error);
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting(false);
                            shell.set_connecting_website(false);
                            shell.set_error_message(err_msg.clone().into());
                            shell.set_toast_message(err_msg.into());
                            shell.set_toast_is_error(true);
                            shell.set_toast_visible(true);
                        }
                    })
                    .ok();
                }
                CourierEvent::PeerDisconnected { node_id } => {
                    println!("Peer disconnected: {}", node_id);
                    let shell_weak = shell_weak.clone();
                    let butler = butler.clone();
                    let node_id_str = node_id.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            // Mark node as disconnected in storage
                            let _ = butler.set_sovereign_node_connected(&node_id_str, false);

                            // Refresh nodes list
                            let nodes = butler.list_sovereign_nodes().unwrap_or_default();
                            let node_infos: Vec<crate::NodeInfo> = nodes
                                .iter()
                                .map(crate::utils::sovereign_node_to_node_info)
                                .collect();
                            shell.set_nodes(slint::ModelRc::new(slint::VecModel::from(node_infos)));
                        }
                    })
                    .ok();
                }
                CourierEvent::SpacePublished { node_id, space_id } => {
                    println!("Space {} published to node {}", space_id, node_id);
                    let shell_weak = shell_weak.clone();
                    let butler = butler.clone();
                    let published_space_id = space_id.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_publishing(false);
                            shell.set_toast_message("Space published successfully".into());
                            shell.set_toast_is_error(false);
                            shell.set_toast_visible(true);

                            // Refresh connected-nodes list with updated published state
                            let current_space_id = shell.get_current_space_id().to_string();
                            if current_space_id == published_space_id {
                                let published_node_dids: HashSet<String> = butler
                                    .get_nodes_with_space_permits(&current_space_id)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .collect();

                                let nodes = butler.list_sovereign_nodes().unwrap_or_default();
                                let connected_nodes: Vec<crate::NodeInfo> = nodes
                                    .iter()
                                    .filter(|n| n.is_connected)
                                    .map(|n| sovereign_node_to_node_info_with_published(n, &published_node_dids))
                                    .collect();
                                shell.set_connected_nodes(slint::ModelRc::new(slint::VecModel::from(connected_nodes)));
                            }
                        }
                    })
                    .ok();
                }
                CourierEvent::PublishFailed { error, .. } => {
                    println!("Publish failed: {}", error);
                    let shell_weak = shell_weak.clone();
                    let err_msg = format!("Publish failed: {}", error);
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
                CourierEvent::ShareableLinkReceived { permit, .. } => {
                    println!("Received shareable link");
                    let shell_weak = shell_weak.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_requesting_link(false);
                            shell.set_shareable_link(permit.into());
                        }
                    })
                    .ok();
                }
                CourierEvent::ViewerSpaceReceived { space, .. } => {
                    println!("Viewer received space: {}", space.name);
                    let shell_weak = shell_weak.clone();
                    let space_name = space.name.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_toast_message(format!("Receiving: {}", space_name).into());
                            shell.set_toast_is_error(false);
                            shell.set_toast_visible(true);
                        }
                    })
                    .ok();
                }
                CourierEvent::ViewerSyncComplete { pages_synced, .. } => {
                    println!("Viewer sync complete: {} pages synced", pages_synced);
                    let shell_weak = shell_weak.clone();
                    let butler = butler.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting_website(false);
                            shell.set_show_add_website_modal(false);

                            // Refresh spaces list
                            let spaces = butler.list_spaces().unwrap_or_default();
                            let space_infos: Vec<crate::SpaceInfo> = spaces
                                .iter()
                                .map(|s| crate::SpaceInfo {
                                    id: s.id.clone().into(),
                                    name: s.name.clone().into(),
                                    app_count: 0,
                                })
                                .collect();
                            shell.set_spaces(slint::ModelRc::new(slint::VecModel::from(space_infos)));

                            shell.set_toast_message(format!("Synced {} pages", pages_synced).into());
                            shell.set_toast_is_error(false);
                            shell.set_toast_visible(true);
                        }
                    })
                    .ok();
                }
                _ => {} // Ignore other events
            }
        }
    });
}
