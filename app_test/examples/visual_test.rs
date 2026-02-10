//! Generic visual test runner — launches N Slint windows sharing a reactive MockScribeHandle
//!
//! Usage:
//!   cargo run --example visual_test -p app_test -- <app_dir> [Name:role ...]
//!
//! Launched by scripts/visual_test.py which handles reload lifecycle.
//! No per-app Rust code needed. No recompilation for app changes.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use lua_runtime::mock_scribe::{MockScribeHandle, MockScribeState};
use renderer_slint::{launch_test_slint_app, LaunchedApp};

struct Peer {
    name: String,
    role: String,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: visual_test <app_dir> [Name:role ...]");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  visual_test sample_apps/osvauld-demos/group-chat Alice:owner Bob:viewer");
        eprintln!("  visual_test sample_apps/my-shop/shop-owner ShopOwner:owner Customer:viewer");
        std::process::exit(1);
    }

    let app_dir = PathBuf::from(&args[0]);
    if !app_dir.join("manifest.json").exists() {
        eprintln!("Error: {} does not contain manifest.json", app_dir.display());
        std::process::exit(1);
    }

    let peers: Vec<Peer> = if args.len() > 1 {
        args[1..]
            .iter()
            .map(|arg| {
                let parts: Vec<&str> = arg.splitn(2, ':').collect();
                Peer {
                    name: parts[0].to_string(),
                    role: parts.get(1).unwrap_or(&"viewer").to_string(),
                }
            })
            .collect()
    } else {
        vec![
            Peer { name: "Alice".to_string(), role: "owner".to_string() },
            Peer { name: "Bob".to_string(), role: "viewer".to_string() },
        ]
    };

    let page_id = "test-page-1";
    let shared_state: Arc<Mutex<MockScribeState>> = MockScribeHandle::shared_state();
    shared_state.lock().unwrap().set_page_id(page_id);

    let mut apps: Vec<LaunchedApp> = Vec::new();

    for peer in &peers {
        let did = format!("did:key:{}", peer.name.to_lowercase());
        let scribe = MockScribeHandle::with_shared_state(shared_state.clone());

        eprintln!("Launching {} ({})", peer.name, peer.role);

        let app = launch_test_slint_app(
            &app_dir,
            page_id,
            &did,
            &peer.name,
            &peer.role,
            scribe,
        )
        .unwrap_or_else(|| {
            eprintln!("Failed to launch window for {}", peer.name);
            std::process::exit(1);
        });

        shared_state
            .lock()
            .unwrap()
            .register_subscriber(app.lua_tx.clone());

        apps.push(app);
    }

    eprintln!("{} window(s) launched. Close all windows to exit.", apps.len());

    slint::run_event_loop().unwrap();
}
