//! Raylib Runtime - Game loop and Lua integration
//!
//! Owns: Raylib window, AudioDevice, SharedSynth, Lua VM
//! Executes: Game loop (init → update → draw → audio fill)
//! Receives: Sync updates via channel

use butler::{Butler, ScribeMessage};
use domains::AppManifest;
use ractor::ActorRef;
use std::sync::{Arc, Mutex};

use crate::game_loop::GameLoop;
use crate::synth::{SharedSynth, Synth};

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
    ///
    /// **Context**: Called from the OS thread spawned by spawn_app
    /// **We own**: Raylib window + AudioDevice + SharedSynth
    /// **Audio**: AudioStream filled from Synth every frame when processed
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

        // MSAA for smooth spheres/cylinders
        tracing::info!(page_id = %self.page_id, "Calling raylib::init().build()");
        let w = self.width as i32;
        let h = self.height as i32;
        let title = self.app_name.clone();
        let fps = self.target_fps;
        let (mut rl, thread) = match std::panic::catch_unwind(|| {
            raylib::init().size(w, h).title(&title).msaa_4x().build()
        }) {
            Ok(result) => {
                tracing::info!("raylib::init().build() succeeded");
                result
            }
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| panic.downcast_ref::<String>().map(|s| s.as_str()))
                    .unwrap_or("unknown panic");
                tracing::error!(page_id = %self.page_id, panic = msg, "raylib::init().build() panicked");
                return Err(format!("raylib init panicked: {}", msg).into());
            }
        };
        rl.set_target_fps(fps);

        // No audio hardware on this machine: synth still exists so audio Lua bindings
        // don't crash; it just produces no sound.
        let synth: SharedSynth = Arc::new(Mutex::new(Synth::new()));

        tracing::info!(page_id = %self.page_id, "Calling GameLoop::new()");
        let lua_code = self.lua_code.clone();
        let page_id2 = self.page_id.clone();
        let app_name2 = self.app_name.clone();
        let butler2 = self.butler.clone();
        let scribe_ref2 = self.scribe_ref.clone();
        let our_did2 = our_did.clone();
        let synth2 = synth.clone();
        let mut game_loop = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            GameLoop::new(
                &lua_code,
                &page_id2,
                &app_name2,
                butler2,
                scribe_ref2,
                our_did2,
                synth2,
            )
        })) {
            Ok(Ok(gl)) => {
                tracing::info!(page_id = %self.page_id, "GameLoop::new() succeeded");
                gl
            }
            Ok(Err(e)) => {
                tracing::error!(page_id = %self.page_id, error = %e, "GameLoop::new() error");
                return Err(e);
            }
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| panic.downcast_ref::<String>().map(|s| s.as_str()))
                    .unwrap_or("unknown panic");
                tracing::error!(page_id = %self.page_id, panic = msg, "GameLoop::new() panicked");
                return Err(format!("GameLoop::new panicked: {}", msg).into());
            }
        };

        tracing::info!(page_id = %self.page_id, "Calling game_loop.init()");
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game_loop.init())) {
            Ok(Ok(())) => tracing::info!(page_id = %self.page_id, "game_loop.init() succeeded"),
            Ok(Err(e)) => {
                tracing::error!(page_id = %self.page_id, error = %e, "game_loop.init() returned error");
                return Err(e);
            }
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| panic.downcast_ref::<String>().map(|s| s.as_str()))
                    .unwrap_or("unknown panic");
                tracing::error!(page_id = %self.page_id, panic = msg, "game_loop.init() panicked");
                return Err(format!("init panicked: {}", msg).into());
            }
        }

        tracing::info!(page_id = %self.page_id, "Entering game loop, window_should_close={}", rl.window_should_close());

        let mut frame = 0u64;
        while !rl.window_should_close() {
            frame += 1;
            let dt = rl.get_frame_time();

            game_loop.process_sync_updates();
            game_loop.handle_input(&mut rl);
            if let Err(e) = game_loop.update(dt) {
                tracing::error!(page_id = %self.page_id, frame, error = %e, "update failed");
                break;
            }

            let mut d = rl.begin_drawing(&thread);
            if let Err(e) = game_loop.draw(&mut d) {
                tracing::error!(page_id = %self.page_id, frame, error = %e, "draw failed");
                break;
            }
        }

        tracing::info!(
            page_id = %self.page_id,
            app_name = %self.app_name,
            frames = frame,
            "Raylib game ended"
        );

        Ok(())
    }
}
