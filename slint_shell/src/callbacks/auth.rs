//! Authentication callbacks
//!
//! Handles login, signup, and import key flows.

use std::sync::Arc;

use butler::Butler;
use courier::CourierHandle;
use slint::ComponentHandle;
use tokio::sync::RwLock;

use crate::events::spawn_event_listener;
use crate::setup::init_p2p;
use crate::Shell;

/// Register authentication callbacks on the shell
pub fn register(
    shell: &Shell,
    butler: Arc<Butler>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    register_import_key(shell);
    register_login(shell, butler.clone(), courier_handle, tokio_handle);
    register_signup(shell, butler);
}

fn register_import_key(shell: &Shell) {
    shell.on_import_key(|| {
        println!("Import key clicked");
        // TODO: Open file dialog for key import
    });
}

fn register_login(
    shell: &Shell,
    butler: Arc<Butler>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
) {
    let shell_weak = shell.as_weak();

    shell.on_login(move |passphrase| {
        println!("Attempting login...");

        match butler.login_sync(&passphrase) {
            Ok(identity) => {
                println!("Login successful! DID: {}", identity.did());

                let butler_for_p2p = butler.clone();
                let courier_handle_for_p2p = courier_handle.clone();
                let shell_weak_for_p2p = shell_weak.clone();

                // Update UI immediately
                if let Some(shell) = shell_weak.upgrade() {
                    shell.set_authenticated(true);
                    shell.set_current_screen("spaces".into());
                }

                tokio_handle.spawn(async move {
                    butler_for_p2p.set_identity(identity.clone()).await;

                    println!("Initializing P2P...");
                    match init_p2p(butler_for_p2p.clone()).await {
                        Ok((handle, event_rx)) => {
                            println!("P2P initialized successfully");
                            *courier_handle_for_p2p.write().await = Some(handle.clone());

                            // Auto-reconnect to stored sovereign nodes
                            spawn_auto_reconnect(butler_for_p2p.clone(), handle.clone());

                            // Spawn event listener
                            spawn_event_listener(event_rx, shell_weak_for_p2p, butler_for_p2p);
                        }
                        Err(e) => {
                            eprintln!("Failed to initialize P2P: {}", e);
                        }
                    }
                });
            }
            Err(e) => {
                println!("Login failed: {}", e);
                // TODO: Set error state on shell
            }
        }
    });
}

fn register_signup(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();

    shell.on_sign_up(move |username, password| {
        println!("Sign up: username={}", username);

        match butler.signup_sync(&username, &password) {
            Ok(result) => {
                println!("Signup successful!");
                println!("IMPORTANT: Save this seed phrase: {}", result.mnemonic);

                if let Some(shell) = shell_weak.upgrade() {
                    shell.set_initialized(true);
                    shell.set_current_screen("login".into());
                }
            }
            Err(e) => {
                println!("Signup failed: {}", e);
                // TODO: Set error state on shell
            }
        }
    });
}

/// Spawn background task to auto-reconnect to stored sovereign nodes
fn spawn_auto_reconnect(butler: Arc<Butler>, handle: CourierHandle) {
    tokio::spawn(async move {
        let nodes = butler.list_sovereign_nodes().unwrap_or_default();
        for node in nodes {
            println!("Auto-connecting to node: {} ({})", node.name, node.node_id);
            if let Some(permit) = &node.permit {
                match handle.connect_and_wait_for_auth(&node.node_id, permit).await {
                    Ok(_) => {
                        println!("Connected and authenticated with node {}", node.name);
                        let _ = butler.set_sovereign_node_connected(&node.node_id, true);
                    }
                    Err(e) => {
                        println!("Failed to connect to node {}: {}", node.name, e);
                        let _ = butler.set_sovereign_node_connected(&node.node_id, false);
                    }
                }
            } else {
                println!("No permit stored for node {}, skipping", node.name);
                let _ = butler.set_sovereign_node_connected(&node.node_id, false);
            }
        }
    });
}
