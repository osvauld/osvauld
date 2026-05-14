//! CourierEvent handling
//!
//! Spawns event listener for P2P events and updates UI accordingly.
//! Includes automatic reconnection with exponential backoff when peers disconnect.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use butler::Butler;
use courier::{CourierEvent, CourierHandle};
use sthalam_shell::Shell;
use tokio::sync::{mpsc::Receiver, watch};

/// Spawn event listener for courier events.
///
/// On PeerDisconnected spawns a background reconnect task with exponential
/// backoff; PeerAuthenticated cancels any pending reconnect for that node.
pub fn spawn_event_listener(
    mut event_rx: Receiver<CourierEvent>,
    shell_weak: slint::Weak<Shell>,
    butler: Arc<Butler>,
    handle: CourierHandle,
) {
    tokio::spawn(async move {
        // Track active reconnection tasks so we can cancel them on successful connect.
        // Sending `true` on the watch channel signals the reconnect loop to stop.
        let mut reconnect_cancellers: HashMap<String, watch::Sender<bool>> = HashMap::new();

        while let Some(event) = event_rx.recv().await {
            match event {
                CourierEvent::PeerAuthenticated {
                    node_id, username, ..
                } => {
                    tracing::info!(username = %username, node_id = %node_id, "Peer authenticated");

                    if let Some(tx) = reconnect_cancellers.remove(&node_id) {
                        tracing::info!(node_id = %node_id, "Cancelling reconnection task — peer authenticated");
                        let _ = tx.send(true);
                    }

                    let shell_weak = shell_weak.clone();
                    let butler = butler.clone();
                    let node_id_str = node_id.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting(false);
                            shell.set_show_add_modal(false);
                            shell.set_connection_string("".into());

                            let _ = butler.nodes().set_connected(&node_id_str, true);

                            let nodes = butler.nodes().list().unwrap_or_default();
                            let node_infos: Vec<sthalam_shell::NodeInfo> = nodes
                                .iter()
                                .map(sthalam_shell::sovereign_node_to_node_info)
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
                    tracing::warn!(node_id = %node_id, error = %error, "Connection failed");
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
                    tracing::info!(node_id = %node_id, "Peer disconnected");

                    let shell_weak_clone = shell_weak.clone();
                    let butler_clone = butler.clone();
                    let node_id_str = node_id.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak_clone.upgrade() {
                            let _ = butler_clone.nodes().set_connected(&node_id_str, false);

                            let nodes = butler_clone.nodes().list().unwrap_or_default();
                            let node_infos: Vec<sthalam_shell::NodeInfo> = nodes
                                .iter()
                                .map(sthalam_shell::sovereign_node_to_node_info)
                                .collect();
                            shell.set_nodes(slint::ModelRc::new(slint::VecModel::from(node_infos)));
                        }
                    })
                    .ok();

                    if let Some(old_tx) = reconnect_cancellers.remove(&node_id) {
                        let _ = old_tx.send(true);
                    }

                    let (cancel_tx, cancel_rx) = watch::channel(false);
                    reconnect_cancellers.insert(node_id.clone(), cancel_tx);

                    spawn_reconnect_task(node_id, butler.clone(), handle.clone(), cancel_rx);
                }
                CourierEvent::SpacePublished { node_id, space_id } => {
                    tracing::info!(space_id = %space_id, node_id = %node_id, "Space published");
                    let shell_weak = shell_weak.clone();
                    let butler = butler.clone();
                    let published_space_id = space_id.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_publishing(false);
                            shell.set_toast_message("Space published successfully".into());
                            shell.set_toast_is_error(false);
                            shell.set_toast_visible(true);

                            let current_space_id = shell.get_current_space_id().to_string();
                            if current_space_id == published_space_id {
                                let published_node_dids: HashSet<String> = butler
                                    .publish()
                                    .get_nodes_with_permits(&current_space_id)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .collect();

                                let nodes = butler.nodes().list().unwrap_or_default();
                                let connected_nodes: Vec<sthalam_shell::NodeInfo> = nodes
                                    .iter()
                                    .filter(|n| n.is_connected)
                                    .map(|n| {
                                        sthalam_shell::sovereign_node_to_node_info_with_published(
                                            n,
                                            &published_node_dids,
                                        )
                                    })
                                    .collect();
                                shell.set_connected_nodes(slint::ModelRc::new(
                                    slint::VecModel::from(connected_nodes),
                                ));
                            }
                        }
                    })
                    .ok();
                }
                CourierEvent::PublishFailed { error, .. } => {
                    tracing::warn!(error = %error, "Publish failed");
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
                    tracing::info!("Received shareable link");
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
                    tracing::info!(space_name = %space.name, "Viewer received space");
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
                    tracing::info!(pages_synced = pages_synced, "Viewer sync complete");
                    let shell_weak = shell_weak.clone();
                    let butler = butler.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(shell) = shell_weak.upgrade() {
                            shell.set_connecting_website(false);
                            shell.set_show_add_website_modal(false);

                            let spaces = butler.spaces().list().unwrap_or_default();
                            let space_infos: Vec<sthalam_shell::SpaceInfo> = spaces
                                .iter()
                                .map(|s| sthalam_shell::SpaceInfo {
                                    id: s.id.clone().into(),
                                    name: s.name.clone().into(),
                                    app_count: 0,
                                })
                                .collect();
                            shell.set_spaces(slint::ModelRc::new(slint::VecModel::from(
                                space_infos,
                            )));

                            shell
                                .set_toast_message(format!("Synced {} pages", pages_synced).into());
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

/// Spawn a background reconnect task for a disconnected peer.
///
/// Backoff: 2s, 4s, 8s, 16s, 30s, ... (capped at 30s, indefinite retries).
/// Cancelled via watch channel when PeerAuthenticated arrives.
fn spawn_reconnect_task(
    node_id: String,
    butler: Arc<Butler>,
    handle: CourierHandle,
    mut cancel_rx: watch::Receiver<bool>,
) {
    tokio::spawn(async move {
        let permit = find_stored_permit(&butler, &node_id);
        let permit = match permit {
            Some(p) => p,
            None => {
                tracing::info!(node_id = %node_id, "No stored permit for disconnected node, skipping reconnect");
                return;
            }
        };

        let mut delay_secs: u64 = 2;
        let max_delay_secs: u64 = 30;

        loop {
            tokio::select! {
                result = cancel_rx.changed() => {
                    if result.is_ok() && *cancel_rx.borrow() {
                        tracing::info!(node_id = %node_id, "Reconnection cancelled (peer authenticated)");
                        return;
                    }
                    if result.is_err() {
                        return;
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(delay_secs)) => {}
            }

            if *cancel_rx.borrow() {
                tracing::info!(node_id = %node_id, "Reconnection cancelled before attempt");
                return;
            }

            tracing::info!(node_id = %node_id, delay_secs, "Attempting reconnection");

            match handle.connect(&node_id, &permit) {
                Ok(()) => {
                    // connect() is fire-and-forget; PeerAuthenticated/ConnectionFailed
                    // resolve it asynchronously via the event channel. Pause before next retry.
                    tokio::select! {
                        result = cancel_rx.changed() => {
                            if result.is_ok() && *cancel_rx.borrow() {
                                tracing::info!(node_id = %node_id, "Reconnection cancelled after connect attempt");
                                return;
                            }
                            if result.is_err() {
                                return;
                            }
                        }
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                            tracing::debug!(node_id = %node_id, "Reconnection attempt timed out, will retry");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(node_id = %node_id, error = %e, "Reconnect attempt failed");
                }
            }

            delay_secs = (delay_secs * 2).min(max_delay_secs);
        }
    });
}

/// Look up stored permit for a node, checking both sovereign nodes (owner-side)
/// and node contacts (viewer-side).
fn find_stored_permit(butler: &Butler, node_id: &str) -> Option<String> {
    let nodes = butler.nodes().list().unwrap_or_default();
    for node in &nodes {
        if node.node_id == node_id {
            if let Some(ref permit) = node.permit {
                return Some(permit.clone());
            }
        }
    }

    let contacts = butler.contacts().list_nodes().unwrap_or_default();
    for contact in &contacts {
        if contact.node_id.as_deref() == Some(node_id) {
            if let Some(ref permit) = contact.permit {
                return Some(permit.clone());
            }
        }
    }

    None
}
