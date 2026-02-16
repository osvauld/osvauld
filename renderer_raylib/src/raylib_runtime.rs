//! Raylib Runtime - Game loop and Lua integration
//!
//! Owns: Raylib window, Lua VM
//! Executes: Game loop (init → update → draw)
//! Receives: Sync updates via channel

use butler::{Butler, ScribeMessage};
use domains::AppManifest;
use ractor::ActorRef;
use std::sync::Arc;

use crate::game_loop::GameLoop;

/// Raylib runtime state
pub struct RaylibRuntime {
    page_id: String,
    app_name: String,
    butler: Arc<Butler>,
    scribe_ref: ActorRef<ScribeMessage>,
    lua_code: String,
    width: u32,
    height: u32,
    target_fps: u32,
}

impl RaylibRuntime {
    /// Create a new Raylib runtime
    pub fn new(
        page_id: &str,
        app_name: &str,
        butler: Arc<Butler>,
        scribe_ref: ActorRef<ScribeMessage>,
        manifest: AppManifest,
        lua_code: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            page_id: page_id.to_string(),
            app_name: app_name.to_string(),
            butler,
            scribe_ref,
            lua_code,
            width: manifest.width,
            height: manifest.height,
            target_fps: manifest.target_fps,
        })
    }

    /// Run the game loop
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Get identity for multiplayer
        let app_ctx = self.butler.app_context(&self.page_id);
        let our_did = app_ctx.user_did;

        tracing::info!(
            page_id = %self.page_id,
            app_name = %self.app_name,
            our_did = %our_did,
            width = self.width,
            height = self.height,
            target_fps = self.target_fps,
            "Starting Raylib game"
        );

        // Initialize Raylib
        let (mut rl, thread) = raylib::init()
            .size(self.width as i32, self.height as i32)
            .title(&self.app_name)
            .build();

        rl.set_target_fps(self.target_fps);

        // Create game loop
        let mut game_loop = GameLoop::new(
            &self.lua_code,
            &self.page_id,
            &self.app_name,
            self.butler.clone(),
            self.scribe_ref.clone(),
            our_did,
        )?;

        // Initialize game
        game_loop.init()?;

        // Main game loop
        while !rl.window_should_close() {
            let dt = rl.get_frame_time();

            // Process sync updates
            game_loop.process_sync_updates();

            // Handle input
            game_loop.handle_input(&rl);

            // Update game state
            game_loop.update(dt)?;

            // Render
            let mut d = rl.begin_drawing(&thread);
            game_loop.draw(&mut d)?;
        }

        tracing::info!(
            page_id = %self.page_id,
            app_name = %self.app_name,
            "Raylib game ended"
        );

        Ok(())
    }
}
