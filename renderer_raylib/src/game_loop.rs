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
use crate::synth::SharedSynth;

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
    /// Shared synthesizer — Lua loads music_ir, runtime fills AudioStream
    synth: SharedSynth,
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
        synth: SharedSynth,
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
            synth,
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
            if let Err(e) = init_fn.call::<()>(()) {
                tracing::error!(page_id = %self.page_id, error = %e, "Lua on_init() error");
                return Err(Box::new(e));
            }
            tracing::debug!(page_id = %self.page_id, "Called on_init");
        }

        Ok(())
    }

    /// Register canvas bindings in Lua
    fn register_canvas_bindings(&self) -> Result<(), Box<dyn std::error::Error>> {
        let globals = self.lua.globals();

        let canvas = self.lua.create_table()?;

        // All canvas methods use colon syntax (canvas:method), so self is passed as first arg.

        let cmds = self.draw_commands.clone();
        let clear_fn =
            self.lua
                .create_function(move |_, (_self, color): (mlua::Value, String)| {
                    let color = parse_color(&color);
                    cmds.lock().unwrap().push(DrawCommand::Clear { color });
                    Ok(())
                })?;
        canvas.set("clear", clear_fn)?;

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

        let cmds = self.draw_commands.clone();
        let begin_3d_fn = self.lua.create_function(
            move |_,
                  (_self, cam_x, cam_y, cam_z, target_x, target_y, target_z, fovy): (
                mlua::Value,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
            )| {
                cmds.lock().unwrap().push(DrawCommand::BeginMode3D {
                    cam_x,
                    cam_y,
                    cam_z,
                    target_x,
                    target_y,
                    target_z,
                    fovy,
                });
                Ok(())
            },
        )?;
        canvas.set("begin_3d", begin_3d_fn)?;

        let cmds = self.draw_commands.clone();
        let end_3d_fn = self.lua.create_function(move |_, _self: mlua::Value| {
            cmds.lock().unwrap().push(DrawCommand::EndMode3D);
            Ok(())
        })?;
        canvas.set("end_3d", end_3d_fn)?;

        let cmds = self.draw_commands.clone();
        let sphere_fn = self.lua.create_function(
            move |_, (_self, x, y, z, r, color): (mlua::Value, f32, f32, f32, f32, String)| {
                let color = parse_color(&color);
                cmds.lock()
                    .unwrap()
                    .push(DrawCommand::Sphere3D { x, y, z, r, color });
                Ok(())
            },
        )?;
        canvas.set("sphere", sphere_fn)?;

        let cmds = self.draw_commands.clone();
        let line3d_fn = self.lua.create_function(
            move |_,
                  (_self, x1, y1, z1, x2, y2, z2, color): (
                mlua::Value,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                String,
            )| {
                let color = parse_color(&color);
                cmds.lock().unwrap().push(DrawCommand::Line3D {
                    x1,
                    y1,
                    z1,
                    x2,
                    y2,
                    z2,
                    color,
                });
                Ok(())
            },
        )?;
        canvas.set("line3d", line3d_fn)?;

        let cmds = self.draw_commands.clone();
        let cyl_fn = self.lua.create_function(
            move |_,
                  (_self, x1, y1, z1, x2, y2, z2, radius, color): (
                mlua::Value,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                String,
            )| {
                let color = parse_color(&color);
                cmds.lock().unwrap().push(DrawCommand::Cylinder3D {
                    x1,
                    y1,
                    z1,
                    x2,
                    y2,
                    z2,
                    radius,
                    color,
                });
                Ok(())
            },
        )?;
        canvas.set("cylinder3d", cyl_fn)?;

        globals.set("canvas", canvas)?;

        let input = self.lua.create_table()?;

        // Stub: real key_down is installed per-frame in handle_input.
        let key_down_fn = self
            .lua
            .create_function(|_, (_self, _key): (mlua::Value, String)| Ok(false))?;
        input.set("key_down", key_down_fn)?;

        globals.set("input", input)?;

        // audio: Lua calls play_music_ir / stop / is_playing.
        let audio_tbl = self.lua.create_table()?;

        let synth_play = self.synth.clone();
        let play_fn = self.lua.create_function(
            move |_, (_self, json, energy): (mlua::Value, String, f32)| {
                synth_play.lock().unwrap().load_music_ir(&json, energy);
                Ok(())
            },
        )?;
        audio_tbl.set("play_music_ir", play_fn)?;

        let synth_stop = self.synth.clone();
        let stop_fn = self.lua.create_function(move |_, _self: mlua::Value| {
            synth_stop.lock().unwrap().stop();
            Ok(())
        })?;
        audio_tbl.set("stop", stop_fn)?;

        let synth_check = self.synth.clone();
        let playing_fn = self.lua.create_function(move |_, _self: mlua::Value| {
            Ok(synth_check.lock().unwrap().is_playing())
        })?;
        audio_tbl.set("is_playing", playing_fn)?;

        globals.set("audio", audio_tbl)?;

        Ok(())
    }

    /// Handle input (called before update)
    pub fn handle_input(&mut self, rl: &mut RaylibHandle) {
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
        key_states.insert(
            "enter".to_string(),
            rl.is_key_pressed(KeyboardKey::KEY_ENTER),
        );
        key_states.insert(
            "backspace".to_string(),
            rl.is_key_pressed(KeyboardKey::KEY_BACKSPACE),
        );
        key_states.insert(
            "escape".to_string(),
            rl.is_key_down(KeyboardKey::KEY_ESCAPE),
        );
        key_states.insert("r".to_string(), rl.is_key_down(KeyboardKey::KEY_R));
        // Preset camera-view keys (graph-camera-interaction.om: key-1/2/3-ahara)
        key_states.insert("1".to_string(), rl.is_key_pressed(KeyboardKey::KEY_ONE));
        key_states.insert("2".to_string(), rl.is_key_pressed(KeyboardKey::KEY_TWO));
        key_states.insert("3".to_string(), rl.is_key_pressed(KeyboardKey::KEY_THREE));

        let mut chars_this_frame = String::new();
        while let Some(c) = rl.get_char_pressed() {
            chars_this_frame.push(c);
        }

        let mouse_pos = rl.get_mouse_position();
        let mouse_clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
        let mouse_down = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
        let mouse_right_down = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_RIGHT);
        let mouse_delta = rl.get_mouse_delta();
        let mouse_wheel = rl.get_mouse_wheel_move();

        let globals = self.lua.globals();
        if let Ok(input) = globals.get::<Table>("input") {
            let ks = key_states.clone();
            let key_down_fn =
                self.lua
                    .create_function(move |_, (_self, key): (mlua::Value, String)| {
                        let key_lower = key.to_lowercase();
                        let is_down = match key_lower.as_str() {
                            "w" | "up" => {
                                *ks.get("w").unwrap_or(&false) || *ks.get("up").unwrap_or(&false)
                            }
                            "s" | "down" => {
                                *ks.get("s").unwrap_or(&false) || *ks.get("down").unwrap_or(&false)
                            }
                            "a" | "left" => {
                                *ks.get("a").unwrap_or(&false) || *ks.get("left").unwrap_or(&false)
                            }
                            "d" | "right" => {
                                *ks.get("d").unwrap_or(&false) || *ks.get("right").unwrap_or(&false)
                            }
                            _ => *ks.get(&key_lower).unwrap_or(&false),
                        };
                        Ok(is_down)
                    });
            if let Ok(key_fn) = key_down_fn {
                let _ = input.set("key_down", key_fn);
            }

            // key_pressed: single press this frame (enter/backspace use is_key_pressed above).
            let ks2 = key_states.clone();
            let key_pressed_fn =
                self.lua
                    .create_function(move |_, (_self, key): (mlua::Value, String)| {
                        let key_lower = key.to_lowercase();
                        Ok(*ks2.get(&key_lower).unwrap_or(&false))
                    });
            if let Ok(kp_fn) = key_pressed_fn {
                let _ = input.set("key_pressed", kp_fn);
            }

            let chars = chars_this_frame.clone();
            let chars_fn = self
                .lua
                .create_function(move |_, _self: mlua::Value| Ok(chars.clone()));
            if let Ok(cf) = chars_fn {
                let _ = input.set("chars", cf);
            }

            let mx = mouse_pos.x;
            let my = mouse_pos.y;
            let mouse_pos_fn = self
                .lua
                .create_function(move |_, _self: mlua::Value| Ok((mx, my)));
            if let Ok(mpf) = mouse_pos_fn {
                let _ = input.set("mouse_pos", mpf);
            }

            let clicked = mouse_clicked;
            if let Ok(mcf) = self
                .lua
                .create_function(move |_, _self: mlua::Value| Ok(clicked))
            {
                let _ = input.set("mouse_clicked", mcf);
            }

            let down = mouse_down;
            if let Ok(mdf) = self
                .lua
                .create_function(move |_, _self: mlua::Value| Ok(down))
            {
                let _ = input.set("mouse_down", mdf);
            }

            // right button — used by orbit-kriya in 3D scenes
            let rdown = mouse_right_down;
            if let Ok(mrdf) = self
                .lua
                .create_function(move |_, _self: mlua::Value| Ok(rdown))
            {
                let _ = input.set("mouse_right_down", mrdf);
            }

            let dx = mouse_delta.x;
            let dy = mouse_delta.y;
            if let Ok(mdf2) = self
                .lua
                .create_function(move |_, _self: mlua::Value| Ok((dx, dy)))
            {
                let _ = input.set("mouse_delta", mdf2);
            }

            let wheel = mouse_wheel;
            if let Ok(mwf) = self
                .lua
                .create_function(move |_, _self: mlua::Value| Ok(wheel))
            {
                let _ = input.set("mouse_wheel", mwf);
            }
        }
    }

    /// Update game state (call update(dt))
    pub fn update(&mut self, dt: f32) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(update_fn) = self.lua.globals().get::<Function>("update") {
            if let Err(e) = update_fn.call::<()>(dt) {
                tracing::error!(page_id = %self.page_id, error = %e, "Lua update() error");
                return Err(Box::new(e));
            }
        }
        Ok(())
    }

    /// Draw frame (call draw())
    pub fn draw(&mut self, d: &mut RaylibDrawHandle) -> Result<(), Box<dyn std::error::Error>> {
        // Default clear; Lua may override via canvas:clear().
        d.clear_background(Color::BLACK);

        if let Ok(draw_fn) = self.lua.globals().get::<Function>("draw") {
            if let Err(e) = draw_fn.call::<()>(()) {
                tracing::error!(page_id = %self.page_id, error = %e, "Lua draw() error");
                return Err(Box::new(e));
            }
        }

        let commands: Vec<DrawCommand> = {
            let mut cmds = self.draw_commands.lock().unwrap();
            std::mem::take(&mut *cmds)
        };

        // Walk commands in order, entering mode3D on BeginMode3D and exiting on EndMode3D.
        let mut i = 0;
        while i < commands.len() {
            match &commands[i] {
                DrawCommand::Clear { color } => {
                    d.clear_background(*color);
                    i += 1;
                }
                DrawCommand::Rect { x, y, w, h, color } => {
                    d.draw_rectangle(*x as i32, *y as i32, *w as i32, *h as i32, *color);
                    i += 1;
                }
                DrawCommand::Circle { x, y, r, color } => {
                    d.draw_circle(*x as i32, *y as i32, *r, *color);
                    i += 1;
                }
                DrawCommand::Text {
                    x,
                    y,
                    text,
                    size,
                    color,
                } => {
                    d.draw_text(text, *x as i32, *y as i32, *size as i32, *color);
                    i += 1;
                }
                DrawCommand::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                } => {
                    d.draw_line(*x1 as i32, *y1 as i32, *x2 as i32, *y2 as i32, *color);
                    i += 1;
                }
                DrawCommand::BeginMode3D {
                    cam_x,
                    cam_y,
                    cam_z,
                    target_x,
                    target_y,
                    target_z,
                    fovy,
                } => {
                    let camera = Camera3D::perspective(
                        Vector3::new(*cam_x, *cam_y, *cam_z),
                        Vector3::new(*target_x, *target_y, *target_z),
                        Vector3::new(0.0, 1.0, 0.0),
                        *fovy,
                    );
                    let mut mode3d = d.begin_mode3D(camera);
                    i += 1;
                    while i < commands.len() {
                        match &commands[i] {
                            DrawCommand::EndMode3D => {
                                i += 1;
                                break;
                            }
                            DrawCommand::Sphere3D { x, y, z, r, color } => {
                                mode3d.draw_sphere(Vector3::new(*x, *y, *z), *r, *color);
                                i += 1;
                            }
                            DrawCommand::Line3D {
                                x1,
                                y1,
                                z1,
                                x2,
                                y2,
                                z2,
                                color,
                            } => {
                                mode3d.draw_line_3D(
                                    Vector3::new(*x1, *y1, *z1),
                                    Vector3::new(*x2, *y2, *z2),
                                    *color,
                                );
                                i += 1;
                            }
                            DrawCommand::Cylinder3D {
                                x1,
                                y1,
                                z1,
                                x2,
                                y2,
                                z2,
                                radius,
                                color,
                            } => {
                                // draw_cylinder_ex: start_pos, end_pos, start_r, end_r, slices.
                                mode3d.draw_cylinder_ex(
                                    Vector3::new(*x1, *y1, *z1),
                                    Vector3::new(*x2, *y2, *z2),
                                    *radius,
                                    *radius,
                                    8,
                                    *color,
                                );
                                i += 1;
                            }
                            // 2D commands inside a 3D block — skip gracefully
                            _ => {
                                i += 1;
                            }
                        }
                    }
                }
                DrawCommand::EndMode3D => {
                    i += 1;
                } // stray end — ignore
                DrawCommand::Sphere3D { .. }
                | DrawCommand::Line3D { .. }
                | DrawCommand::Cylinder3D { .. } => {
                    // 3D commands outside a BeginMode3D block — ignore
                    i += 1;
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
