//! Game Loop - Lua integration for Raylib games
//!
//! Manages: Lua VM, game callbacks (on_init, update, draw)
//! Provides: Canvas bindings for immediate-mode drawing

use butler::{Butler, ScribeMessage};
use lua_runtime::{json_to_lua, ActorScribeHandle, ScribeBindings};
use mlua::{Function, Lua, Table};
use ractor::ActorRef;
use raylib::prelude::*;
use scribe::message::PageUpdate;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::canvas_bindings::{parse_color, DrawCommand};

/// Game loop state
pub struct GameLoop {
    lua: Lua,
    page_id: String,
    _app_name: String,
    _butler: Arc<Butler>,
    scribe_ref: ActorRef<ScribeMessage>,
    our_did: String,
    /// Shared draw commands between Lua and render phase
    draw_commands: Arc<Mutex<Vec<DrawCommand>>>,
    /// Channel for receiving page updates (ephemeral, peer join/leave)
    page_update_rx: mpsc::Receiver<PageUpdate>,
}

impl GameLoop {
    /// Create a new game loop
    pub fn new(
        lua_code: &str,
        page_id: &str,
        app_name: &str,
        butler: Arc<Butler>,
        scribe_ref: ActorRef<ScribeMessage>,
        our_did: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let lua = Lua::new();
        let draw_commands = Arc::new(Mutex::new(Vec::new()));

        // Subscribe to page updates for multiplayer sync
        let (tx, rx) = mpsc::channel(256);
        scribe_ref
            .cast(ScribeMessage::SubscribeToPageUpdates { tx })
            .map_err(|e| format!("Failed to subscribe to page updates: {}", e))?;

        // Load the Lua code
        lua.load(lua_code)
            .set_name(&format!("{}:{}", page_id, app_name))
            .exec()?;

        Ok(Self {
            lua,
            page_id: page_id.to_string(),
            _app_name: app_name.to_string(),
            _butler: butler,
            scribe_ref,
            our_did,
            draw_commands,
            page_update_rx: rx,
        })
    }

    /// Initialize the game (call on_init)
    ///
    /// **Context**: Registers canvas + input + scribe bindings, then calls Lua on_init
    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Register canvas bindings
        self.register_canvas_bindings()?;

        // Register scribe bindings for multiplayer
        let scribe_bindings = ScribeBindings::new(
            ActorScribeHandle::new(self.scribe_ref.clone()),
            self.page_id.clone(),
            self.our_did.clone(),
            None,
        );
        self.lua.globals().set("scribe", scribe_bindings)?;

        // Call on_init if it exists
        if let Ok(init_fn) = self.lua.globals().get::<Function>("on_init") {
            init_fn.call::<()>(())?;
            tracing::debug!(page_id = %self.page_id, "Called on_init");
        }

        Ok(())
    }

    /// Register canvas bindings in Lua
    fn register_canvas_bindings(&self) -> Result<(), Box<dyn std::error::Error>> {
        let globals = self.lua.globals();

        // Create canvas table
        let canvas = self.lua.create_table()?;

        // Note: All canvas methods use colon syntax (canvas:method), so self is passed as first arg

        // canvas:clear(color)
        let cmds = self.draw_commands.clone();
        let clear_fn =
            self.lua
                .create_function(move |_, (_self, color): (mlua::Value, String)| {
                    let color = parse_color(&color);
                    cmds.lock().unwrap().push(DrawCommand::Clear { color });
                    Ok(())
                })?;
        canvas.set("clear", clear_fn)?;

        // canvas:rect(x, y, w, h, color)
        let cmds = self.draw_commands.clone();
        let rect_fn = self.lua.create_function(
            move |_, (_self, x, y, w, h, color): (mlua::Value, f32, f32, f32, f32, String)| {
                let color = parse_color(&color);
                cmds.lock()
                    .unwrap()
                    .push(DrawCommand::Rect { x, y, w, h, color });
                Ok(())
            },
        )?;
        canvas.set("rect", rect_fn)?;

        // canvas:circle(x, y, r, color)
        let cmds = self.draw_commands.clone();
        let circle_fn = self.lua.create_function(
            move |_, (_self, x, y, r, color): (mlua::Value, f32, f32, f32, String)| {
                let color = parse_color(&color);
                cmds.lock()
                    .unwrap()
                    .push(DrawCommand::Circle { x, y, r, color });
                Ok(())
            },
        )?;
        canvas.set("circle", circle_fn)?;

        // canvas:text(x, y, text, size, color)
        let cmds = self.draw_commands.clone();
        let text_fn = self.lua.create_function(
            move |_,
                  (_self, x, y, text, size, color): (
                mlua::Value,
                f32,
                f32,
                String,
                f32,
                String,
            )| {
                let color = parse_color(&color);
                cmds.lock().unwrap().push(DrawCommand::Text {
                    x,
                    y,
                    text,
                    size,
                    color,
                });
                Ok(())
            },
        )?;
        canvas.set("text", text_fn)?;

        // canvas:line(x1, y1, x2, y2, color)
        let cmds = self.draw_commands.clone();
        let line_fn = self.lua.create_function(
            move |_, (_self, x1, y1, x2, y2, color): (mlua::Value, f32, f32, f32, f32, String)| {
                let color = parse_color(&color);
                cmds.lock().unwrap().push(DrawCommand::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                });
                Ok(())
            },
        )?;
        canvas.set("line", line_fn)?;

        globals.set("canvas", canvas)?;

        // Create input table
        let input = self.lua.create_table()?;

        // input:key_down(key) - will be populated in handle_input
        // Note: Using colon syntax (input:key_down) passes self as first arg
        let key_down_fn = self
            .lua
            .create_function(|_, (_self, _key): (mlua::Value, String)| Ok(false))?;
        input.set("key_down", key_down_fn)?;

        globals.set("input", input)?;

        Ok(())
    }

    /// Handle input (called before update)
    pub fn handle_input(&mut self, rl: &RaylibHandle) {
        // Poll key states into a HashMap (owned data)
        let mut key_states: HashMap<String, bool> = HashMap::new();
        key_states.insert("w".to_string(), rl.is_key_down(KeyboardKey::KEY_W));
        key_states.insert("up".to_string(), rl.is_key_down(KeyboardKey::KEY_UP));
        key_states.insert("s".to_string(), rl.is_key_down(KeyboardKey::KEY_S));
        key_states.insert("down".to_string(), rl.is_key_down(KeyboardKey::KEY_DOWN));
        key_states.insert("a".to_string(), rl.is_key_down(KeyboardKey::KEY_A));
        key_states.insert("left".to_string(), rl.is_key_down(KeyboardKey::KEY_LEFT));
        key_states.insert("d".to_string(), rl.is_key_down(KeyboardKey::KEY_D));
        key_states.insert("right".to_string(), rl.is_key_down(KeyboardKey::KEY_RIGHT));
        key_states.insert("space".to_string(), rl.is_key_down(KeyboardKey::KEY_SPACE));
        key_states.insert("enter".to_string(), rl.is_key_down(KeyboardKey::KEY_ENTER));
        key_states.insert(
            "escape".to_string(),
            rl.is_key_down(KeyboardKey::KEY_ESCAPE),
        );
        key_states.insert("r".to_string(), rl.is_key_down(KeyboardKey::KEY_R));

        // Update input bindings based on current key state
        let globals = self.lua.globals();
        if let Ok(input) = globals.get::<Table>("input") {
            // Create closure that captures the polled key states
            // Note: Using colon syntax (input:key_down) passes self as first arg
            let key_down_fn =
                self.lua
                    .create_function(move |_, (_self, key): (mlua::Value, String)| {
                        let key_lower = key.to_lowercase();
                        // Check for combined keys (w/up, s/down, etc.)
                        let is_down = match key_lower.as_str() {
                            "w" | "up" => {
                                *key_states.get("w").unwrap_or(&false)
                                    || *key_states.get("up").unwrap_or(&false)
                            }
                            "s" | "down" => {
                                *key_states.get("s").unwrap_or(&false)
                                    || *key_states.get("down").unwrap_or(&false)
                            }
                            "a" | "left" => {
                                *key_states.get("a").unwrap_or(&false)
                                    || *key_states.get("left").unwrap_or(&false)
                            }
                            "d" | "right" => {
                                *key_states.get("d").unwrap_or(&false)
                                    || *key_states.get("right").unwrap_or(&false)
                            }
                            _ => *key_states.get(&key_lower).unwrap_or(&false),
                        };
                        Ok(is_down)
                    });
            if let Ok(key_fn) = key_down_fn {
                let _ = input.set("key_down", key_fn);
            }
        }
    }

    /// Update game state (call update(dt))
    pub fn update(&mut self, dt: f32) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(update_fn) = self.lua.globals().get::<Function>("update") {
            update_fn.call::<()>(dt)?;
        }
        Ok(())
    }

    /// Draw frame (call draw())
    pub fn draw(&mut self, d: &mut RaylibDrawHandle) -> Result<(), Box<dyn std::error::Error>> {
        // Clear with default color (may be overridden by Lua)
        d.clear_background(Color::BLACK);

        // Call Lua draw function to populate draw_commands
        if let Ok(draw_fn) = self.lua.globals().get::<Function>("draw") {
            draw_fn.call::<()>(())?;
        }

        // Execute draw commands collected from Lua
        let commands: Vec<DrawCommand> = {
            let mut cmds = self.draw_commands.lock().unwrap();
            std::mem::take(&mut *cmds)
        };

        for command in commands {
            match command {
                DrawCommand::Clear { color } => {
                    d.clear_background(color);
                }
                DrawCommand::Rect { x, y, w, h, color } => {
                    d.draw_rectangle(x as i32, y as i32, w as i32, h as i32, color);
                }
                DrawCommand::Circle { x, y, r, color } => {
                    d.draw_circle(x as i32, y as i32, r, color);
                }
                DrawCommand::Text {
                    x,
                    y,
                    text,
                    size,
                    color,
                } => {
                    d.draw_text(&text, x as i32, y as i32, size as i32, color);
                }
                DrawCommand::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                } => {
                    d.draw_line(x1 as i32, y1 as i32, x2 as i32, y2 as i32, color);
                }
            }
        }

        Ok(())
    }

    /// Process sync updates from page update channel
    ///
    /// **Context**: Drains all pending PageUpdates and dispatches to Lua callbacks
    /// **We call**: on_ephemeral(from_did, func, args) for StructuredEphemeral
    /// **We call**: on_peer_joined(did) for PeerSubscribed
    /// **We call**: on_peer_left(did) for PeerUnsubscribed
    pub fn process_sync_updates(&mut self) {
        while let Ok(update) = self.page_update_rx.try_recv() {
            match update {
                PageUpdate::StructuredEphemeral {
                    from_did,
                    func,
                    args,
                } => {
                    if let Ok(on_ephemeral) = self.lua.globals().get::<Function>("on_ephemeral") {
                        match json_to_lua(&self.lua, &args) {
                            Ok(args_lua) => {
                                if let Err(e) = on_ephemeral.call::<()>((
                                    from_did.clone(),
                                    func.clone(),
                                    args_lua,
                                )) {
                                    tracing::warn!(error = %e, func = %func, "Error calling on_ephemeral");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, func = %func, "Failed to convert ephemeral args to Lua");
                            }
                        }
                    }
                }
                PageUpdate::PeerSubscribed { did, .. } => {
                    if let Ok(on_peer_joined) = self.lua.globals().get::<Function>("on_peer_joined")
                    {
                        if let Err(e) = on_peer_joined.call::<()>(did.clone()) {
                            tracing::warn!(error = %e, did = %did, "Error calling on_peer_joined");
                        }
                    }
                }
                PageUpdate::PeerUnsubscribed { did } => {
                    if let Ok(on_peer_left) = self.lua.globals().get::<Function>("on_peer_left") {
                        if let Err(e) = on_peer_left.call::<()>(did.clone()) {
                            tracing::warn!(error = %e, did = %did, "Error calling on_peer_left");
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
