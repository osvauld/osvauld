//! Multi-peer test runner
//!
//! Shared MockScribeState with N independent LuaRuntime instances.
//! Simulates multiple users interacting with the same page.

use std::path::Path;
use std::sync::{Arc, Mutex};

use lua_runtime::{MockScribeHandle, MockScribeState};

use crate::runner::AppTestRunner;

/// Role definition for a peer
pub struct PeerRole {
    pub did: String,
    pub name: String,
    pub role: String,
}

impl PeerRole {
    pub fn new(did: &str, name: &str, role: &str) -> Self {
        Self {
            did: did.to_string(),
            name: name.to_string(),
            role: role.to_string(),
        }
    }
}

/// Multi-peer test runner
///
/// Shared MockScribeState across N runners — writes from one peer
/// are immediately visible to others (no sync delay simulation).
pub struct MultiPeerRunner {
    peers: Vec<AppTestRunner>,
    shared_state: Arc<Mutex<MockScribeState>>,
}

impl MultiPeerRunner {
    /// Create from app directory with multiple peers
    pub fn from_dir(
        app_dir: &Path,
        page_id: &str,
        roles: &[PeerRole],
    ) -> Result<Self, String> {
        // Read manifest + lua code
        let manifest_path = app_dir.join("manifest.json");
        let manifest_str = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read manifest.json: {}", e))?;
        let manifest: serde_json::Value = serde_json::from_str(&manifest_str)
            .map_err(|e| format!("Failed to parse manifest.json: {}", e))?;

        let entry_logic = manifest
            .get("entry_logic")
            .and_then(|v| v.as_str())
            .unwrap_or("app.lua");

        let lua_path = app_dir.join(entry_logic);
        let lua_code = std::fs::read_to_string(&lua_path)
            .map_err(|e| format!("Failed to read {}: {}", entry_logic, e))?;

        let init_path = app_dir.join("init.lua");
        let full_code = if init_path.exists() {
            let init_code = std::fs::read_to_string(&init_path)
                .map_err(|e| format!("Failed to read init.lua: {}", e))?;
            format!("{}\n{}", init_code, lua_code)
        } else {
            lua_code
        };

        let app_name = manifest
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("test-app");

        Self::from_code(&full_code, app_name, page_id, roles)
    }

    /// Create from raw Lua code with multiple peers
    pub fn from_code(
        lua_code: &str,
        app_name: &str,
        page_id: &str,
        roles: &[PeerRole],
    ) -> Result<Self, String> {
        let shared_state = MockScribeHandle::shared_state();

        // Set subscriber count to number of peers
        shared_state.lock().unwrap().set_subscriber_count(roles.len());

        let mut peers = Vec::with_capacity(roles.len());
        for role in roles {
            let runner = AppTestRunner::from_code_with_shared_state(
                lua_code,
                app_name,
                page_id,
                &role.role,
                &role.did,
                &role.name,
                shared_state.clone(),
            )?;
            peers.push(runner);
        }

        Ok(Self {
            peers,
            shared_state,
        })
    }

    /// Call on_init on all peers
    pub fn init_all(&mut self) {
        for peer in &mut self.peers {
            peer.call_on_init();
        }
    }

    /// Tick all peers once
    pub fn tick_all(&mut self) {
        for peer in &mut self.peers {
            peer.tick();
        }
    }

    /// Tick all peers N times
    pub fn tick_all_n(&mut self, n: usize) {
        for _ in 0..n {
            for peer in &mut self.peers {
                peer.tick();
            }
        }
    }

    /// Get peer by index
    pub fn peer(&mut self, index: usize) -> &mut AppTestRunner {
        &mut self.peers[index]
    }

    /// Get number of peers
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Get the shared mock state
    pub fn shared_state(&self) -> Arc<Mutex<MockScribeState>> {
        self.shared_state.clone()
    }
}
