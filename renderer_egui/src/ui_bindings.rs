//! Lua-facing UI handle.
//!
//! Stateless `LuaUi` userdata; the active `egui::Ui` lives in a thread-local
//! raw pointer that the runtime sets before `on_frame` and clears after.
//! Layout containers (`horizontal`, `vertical`, etc.) swap the pointer for
//! their closure.
//!
//! **Safety**: pointer valid for the on_frame call and inside child-Ui
//! closures. egui scopes child Uis to their show closure; no parent/child
//! aliasing while a child is live.

use egui;
use mlua::{Function, Table, UserData, UserDataMethods};
use std::cell::Cell;
use std::sync::Arc;

/// Parse a hex color string like `#rrggbb` or `#rrggbbaa` into an egui Color32.
/// Returns `None` for malformed input — caller decides what to do.
fn parse_color(s: &str) -> Option<egui::Color32> {
    let s = s.strip_prefix('#').unwrap_or(s);
    let bytes = match s.len() {
        6 | 8 => s,
        _ => return None,
    };
    let r = u8::from_str_radix(&bytes[0..2], 16).ok()?;
    let g = u8::from_str_radix(&bytes[2..4], 16).ok()?;
    let b = u8::from_str_radix(&bytes[4..6], 16).ok()?;
    let a = if bytes.len() == 8 {
        u8::from_str_radix(&bytes[6..8], 16).ok()?
    } else {
        255
    };
    Some(egui::Color32::from_rgba_unmultiplied(r, g, b, a))
}

thread_local! {
    /// Pointer to the currently-active `egui::Ui` for the in-flight Lua call.
    pub(crate) static UI_PTR: Cell<*mut egui::Ui> = const { Cell::new(std::ptr::null_mut()) };
    /// One-shot flag set by `ui:focus_next()`; consumed by the next
    /// `text_edit` call, which then calls `request_focus()` on its response.
    pub(crate) static FOCUS_NEXT: Cell<bool> = const { Cell::new(false) };
    /// Most recent `text_edit` response; `popup_below_last` anchors against it.
    pub(crate) static LAST_TEXT_EDIT_RESPONSE: std::cell::RefCell<Option<egui::Response>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) fn with_ui<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let p = UI_PTR.with(|c| c.get());
    debug_assert!(!p.is_null(), "UI_PTR not set — Lua called a ui method outside on_frame");
    // Safety: runtime guarantees the pointer is valid; no aliasing because
    // egui scopes child Uis to their show closure.
    unsafe { f(&mut *p) }
}

/// Run `f` against a child `egui::Ui` by temporarily swapping the thread-local
/// pointer. Restores the previous pointer regardless of how `f` exits.
fn with_child_ui<R>(child: &mut egui::Ui, f: impl FnOnce() -> R) -> R {
    let prev = UI_PTR.with(|c| c.replace(child as *mut _));
    let result = f();
    UI_PTR.with(|c| c.set(prev));
    result
}

/// Stateless Lua-facing UI handle. One instance per Lua VM; methods read the
/// active `egui::Ui` from the thread-local.
pub struct LuaUi;

impl UserData for LuaUi {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        // ── Widgets ──────────────────────────────────────────────────────

        // label(text, opts?) — opts: { size, color = "#rrggbb", monospace,
        // weight = "bold"|"italic" }. Missing keys keep egui's defaults.
        m.add_method("label", |_, _, (text, opts): (String, Option<Table>)| {
            with_ui(|ui| {
                let mut rt = egui::RichText::new(text);
                if let Some(opts) = opts {
                    if let Ok(Some(size)) = opts.get::<Option<f32>>("size") {
                        rt = rt.size(size);
                    }
                    if let Ok(Some(color)) = opts.get::<Option<String>>("color") {
                        if let Some(c) = parse_color(&color) {
                            rt = rt.color(c);
                        }
                    }
                    if let Ok(Some(true)) = opts.get::<Option<bool>>("monospace") {
                        rt = rt.monospace();
                    }
                    if let Ok(Some(weight)) = opts.get::<Option<String>>("weight") {
                        match weight.as_str() {
                            "bold" => rt = rt.strong(),
                            "italic" => rt = rt.italics(),
                            _ => {}
                        }
                    }
                }
                ui.label(rt);
            });
            Ok(())
        });

        m.add_method("button", |_, _, text: String| {
            let clicked = with_ui(|ui| ui.button(text).clicked());
            Ok(clicked)
        });

        m.add_method("separator", |_, _, ()| {
            with_ui(|ui| {
                ui.separator();
            });
            Ok(())
        });

        // ── Layout primitives ────────────────────────────────────────────
        // Thin pass-throughs to egui::Ui — apps compose their own helpers in Lua.

        m.add_method("add_space", |_, _, n: f32| {
            with_ui(|ui| ui.add_space(n));
            Ok(())
        });

        m.add_method("set_max_width", |_, _, w: f32| {
            with_ui(|ui| ui.set_max_width(w));
            Ok(())
        });

        m.add_method("set_min_height", |_, _, h: f32| {
            with_ui(|ui| ui.set_min_height(h));
            Ok(())
        });

        m.add_method("available_width", |_, _, ()| {
            Ok(with_ui(|ui| ui.available_width()))
        });

        // text_edit(value, opts?) -> (new_value, changed, has_focus)
        //
        // opts: { size, monospace, multiline, frame, width, code, rows }.
        // Width defaults to (avail - 100 px) because egui's stock 280 px
        // singleline looks tiny on a wide row.
        m.add_method("text_edit", |_, _, (value, opts): (String, Option<Table>)| {
            let mut buf = value.clone();

            let mut size: Option<f32> = None;
            let mut monospace = false;
            let mut multiline = false;
            let mut code = false;
            let mut frame = true;
            let mut explicit_width: Option<f32> = None;
            let mut rows: Option<usize> = None;
            if let Some(opts) = opts {
                size = opts.get::<Option<f32>>("size").ok().flatten();
                monospace = opts.get::<Option<bool>>("monospace").ok().flatten().unwrap_or(false);
                multiline = opts.get::<Option<bool>>("multiline").ok().flatten().unwrap_or(false);
                code = opts.get::<Option<bool>>("code").ok().flatten().unwrap_or(false);
                // `frame` defaults to true (egui default); explicit false = borderless.
                frame = opts.get::<Option<bool>>("frame").ok().flatten().unwrap_or(true);
                explicit_width = opts.get::<Option<f32>>("width").ok().flatten();
                rows = opts.get::<Option<usize>>("rows").ok().flatten();
            }

            let (changed, has_focus) = with_ui(|ui| {
                let avail = ui.available_width();
                let default_w = (avail - 100.0).max(120.0);
                let width = explicit_width.unwrap_or(default_w);

                let mut edit = if multiline {
                    egui::TextEdit::multiline(&mut buf)
                        .desired_rows(rows.unwrap_or(1))
                } else {
                    egui::TextEdit::singleline(&mut buf)
                };
                edit = edit.desired_width(width);
                if let Some(s) = size {
                    edit = edit.font(egui::FontId::proportional(s));
                } else if monospace {
                    edit = edit.font(egui::TextStyle::Monospace);
                }
                if code {
                    edit = edit.code_editor();
                }
                if !frame {
                    edit = edit.frame(egui::Frame::NONE);
                }
                let resp = ui.add(edit);
                // Consume one-shot focus request from `ui:focus_next()`.
                if FOCUS_NEXT.with(|c| c.replace(false)) {
                    resp.request_focus();
                }
                // Stash for `popup_below_last` anchoring. Response is Clone (Arc internals).
                LAST_TEXT_EDIT_RESPONSE.with(|c| *c.borrow_mut() = Some(resp.clone()));
                (resp.changed(), resp.has_focus())
            });
            Ok((buf, changed, has_focus))
        });

        // checkbox(checked) -> new_checked
        m.add_method("checkbox", |_, _, checked: bool| {
            let mut state = checked;
            with_ui(|ui| {
                ui.checkbox(&mut state, "");
            });
            Ok(state)
        });

        // ── Layout containers ────────────────────────────────────────────

        m.add_method("horizontal", |_, _, f: Function| {
            let inner_err = with_ui(|ui| {
                let mut err: Option<mlua::Error> = None;
                ui.horizontal(|child| {
                    with_child_ui(child, || {
                        if let Err(e) = f.call::<()>(()) {
                            err = Some(e);
                        }
                    });
                });
                err
            });
            if let Some(e) = inner_err {
                return Err(e);
            }
            Ok(())
        });

        m.add_method("vertical", |_, _, f: Function| {
            let inner_err = with_ui(|ui| {
                let mut err: Option<mlua::Error> = None;
                ui.vertical(|child| {
                    with_child_ui(child, || {
                        if let Err(e) = f.call::<()>(()) {
                            err = Some(e);
                        }
                    });
                });
                err
            });
            if let Some(e) = inner_err {
                return Err(e);
            }
            Ok(())
        });

        // scroll_area(fn) — vertical scrolling region. Closure draws contents.
        m.add_method("scroll_area", |_, _, f: Function| {
            let inner_err = with_ui(|ui| {
                let mut err: Option<mlua::Error> = None;
                egui::ScrollArea::vertical().show(ui, |child| {
                    with_child_ui(child, || {
                        if let Err(e) = f.call::<()>(()) {
                            err = Some(e);
                        }
                    });
                });
                err
            });
            if let Some(e) = inner_err {
                return Err(e);
            }
            Ok(())
        });

        // frame(opts, fn) — bg/rounding/padding wrapper.
        // opts: { bg = "#rrggbb", rounding, padding }. All optional.
        m.add_method("frame", |_, _, (opts, f): (Table, Function)| {
            let bg = opts
                .get::<Option<String>>("bg")
                .ok()
                .flatten()
                .and_then(|s| parse_color(&s));
            let rounding = opts.get::<Option<f32>>("rounding").ok().flatten();
            let padding = opts.get::<Option<f32>>("padding").ok().flatten();

            let mut frame = egui::Frame::default();
            if let Some(c) = bg {
                frame = frame.fill(c);
            }
            if let Some(r) = rounding {
                frame = frame.corner_radius(r);
            }
            if let Some(p) = padding {
                frame = frame.inner_margin(p);
            }

            let inner_err = with_ui(|ui| {
                let mut err: Option<mlua::Error> = None;
                frame.show(ui, |child| {
                    with_child_ui(child, || {
                        if let Err(e) = f.call::<()>(()) {
                            err = Some(e);
                        }
                    });
                });
                err
            });
            if let Some(e) = inner_err {
                return Err(e);
            }
            Ok(())
        });

        // was_hovered(id) -> bool. Reads `hover_row`'s prior-frame state,
        // letting code outside the row react without re-doing pointer-rect math.
        m.add_method("was_hovered", |_, _, row_id: String| {
            Ok(with_ui(|ui| {
                let id = egui::Id::new(("hover_row", row_id));
                ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false))
            }))
        });

        // hover_row(opts, fn) -> current_hovered
        //
        // Paints a row whose bg uses the CURRENT frame's hover state (no
        // one-frame lag). Trick: reserve a shape slot before content draws
        // so z-order is correct, then back-fill it once we know the rect
        // and this frame's hover. The closure still gets `prev_hovered` so
        // siblings (grip glyph etc.) can react without redoing pointer math.
        //
        // opts: { id, bg = "#rrggbb", rounding, padding, min_height }
        m.add_method("hover_row", |_, _, (opts, f): (Table, Function)| {
            let row_id: String = opts.get::<Option<String>>("id").ok().flatten().unwrap_or_default();
            let bg = opts
                .get::<Option<String>>("bg")
                .ok()
                .flatten()
                .and_then(|s| parse_color(&s))
                .unwrap_or(egui::Color32::from_gray(38));
            let rounding = opts.get::<Option<f32>>("rounding").ok().flatten().unwrap_or(4.0);
            let padding = opts.get::<Option<f32>>("padding").ok().flatten().unwrap_or(4.0);
            let min_height = opts.get::<Option<f32>>("min_height").ok().flatten();

            let (inner_err, now_hovered) = with_ui(|ui| {
                let id = egui::Id::new(("hover_row", row_id.clone()));
                let prev_hovered: bool = ui.data(|d| d.get_temp(id).unwrap_or(false));

                // Reserve slot for the bg; back-filled after we know the rect.
                let bg_shape_idx = ui.painter().add(egui::Shape::Noop);

                // No fill on the Frame — we paint the bg via the reserved slot.
                let frame = egui::Frame::default()
                    .corner_radius(rounding)
                    .inner_margin(padding);

                let mut err: Option<mlua::Error> = None;
                let resp = frame
                    .show(ui, |child| {
                        if let Some(h) = min_height {
                            child.set_min_height(h);
                        }
                        with_child_ui(child, || {
                            if let Err(e) = f.call::<()>(prev_hovered) {
                                err = Some(e);
                            }
                        });
                    })
                    .response;
                let now = resp.contains_pointer();
                // Back-fill slot: paint bg only if hovered; else leave as Noop.
                if now {
                    ui.painter().set(
                        bg_shape_idx,
                        egui::Shape::rect_filled(resp.rect, rounding, bg),
                    );
                }
                ui.data_mut(|d| d.insert_temp(id, now));
                (err, now)
            });
            if let Some(e) = inner_err {
                return Err(e);
            }
            Ok(now_hovered)
        });

        // drag_source(id, payload, handle_fn, preview_fn?) — wraps
        // egui's dnd_drag_source. String payloads avoid typed-userdata
        // round-trips. While dragging, `preview_fn` (if any) runs in place
        // of `handle_fn`, giving a richer drag ghost than the bare handle —
        // egui captures the closure's shapes once per frame for the ghost.
        m.add_method(
            "drag_source",
            |_, _, (id, payload, handle_fn, preview_fn): (String, String, Function, Option<Function>)| {
                let inner_err = with_ui(|ui| {
                    let mut err: Option<mlua::Error> = None;
                    let eid = egui::Id::new(id);
                    ui.dnd_drag_source(eid, payload, |child| {
                        // Check dragged_id INSIDE the closure — egui sets the
                        // drag state at the top of dnd_drag_source; checking
                        // before the call saw stale prior-frame state.
                        let dragging = child.ctx().dragged_id() == Some(eid);
                        with_child_ui(child, || {
                            let f = if dragging {
                                preview_fn.as_ref().unwrap_or(&handle_fn)
                            } else {
                                &handle_fn
                            };
                            if let Err(e) = f.call::<()>(()) {
                                err = Some(e);
                            }
                        });
                    });
                    err
                });
                if let Some(e) = inner_err {
                    return Err(e);
                }
                Ok(())
            },
        );

        // set_visuals(opts) — write egui theme tokens for the session.
        // App-driven theming; runtime ships no palette. opts:
        //   { panel_fill, window_fill, fg, extreme_bg, hyperlink,
        //     widget_inactive_fill, widget_inactive_stroke,
        //     widget_hovered_fill, widget_active_fill,
        //     widget_noninteractive_fill }  // all "#rrggbb" or "#rrggbbaa"
        m.add_method("set_visuals", |_, _, opts: Table| {
            with_ui(|ui| {
                let ctx = ui.ctx().clone();
                let mut v = ctx.global_style().visuals.clone();
                if let Some(c) = opts.get::<Option<String>>("panel_fill").ok().flatten()
                    .and_then(|s| parse_color(&s)) { v.panel_fill = c; }
                if let Some(c) = opts.get::<Option<String>>("window_fill").ok().flatten()
                    .and_then(|s| parse_color(&s)) { v.window_fill = c; }
                if let Some(c) = opts.get::<Option<String>>("fg").ok().flatten()
                    .and_then(|s| parse_color(&s)) { v.override_text_color = Some(c); }
                if let Some(c) = opts.get::<Option<String>>("extreme_bg").ok().flatten()
                    .and_then(|s| parse_color(&s)) { v.extreme_bg_color = c; }
                if let Some(c) = opts.get::<Option<String>>("hyperlink").ok().flatten()
                    .and_then(|s| parse_color(&s)) { v.hyperlink_color = c; }
                // Widget surface fills — group widgets (incl. dnd_drop_zone)
                // paint `widgets.inactive.bg_fill` even with Frame::NONE.
                // Exposing these lets apps zero them so rows blend at rest.
                if let Some(c) = opts.get::<Option<String>>("widget_inactive_fill").ok().flatten()
                    .and_then(|s| parse_color(&s)) { v.widgets.inactive.bg_fill = c; v.widgets.inactive.weak_bg_fill = c; }
                if let Some(c) = opts.get::<Option<String>>("widget_inactive_stroke").ok().flatten()
                    .and_then(|s| parse_color(&s)) { v.widgets.inactive.bg_stroke.color = c; }
                if let Some(c) = opts.get::<Option<String>>("widget_hovered_fill").ok().flatten()
                    .and_then(|s| parse_color(&s)) { v.widgets.hovered.bg_fill = c; v.widgets.hovered.weak_bg_fill = c; }
                if let Some(c) = opts.get::<Option<String>>("widget_active_fill").ok().flatten()
                    .and_then(|s| parse_color(&s)) { v.widgets.active.bg_fill = c; v.widgets.active.weak_bg_fill = c; }
                if let Some(c) = opts.get::<Option<String>>("widget_noninteractive_fill").ok().flatten()
                    .and_then(|s| parse_color(&s)) { v.widgets.noninteractive.bg_fill = c; v.widgets.noninteractive.weak_bg_fill = c; }
                ctx.set_visuals(v);
            });
            Ok(())
        });

        // ── Input primitives ─────────────────────────────────────────────
        // Thin wrappers; apps compose modifier combos themselves.

        m.add_method("key_pressed", |_, _, key_name: String| {
            let key = match key_name.to_lowercase().as_str() {
                "enter" => Some(egui::Key::Enter),
                "escape" | "esc" => Some(egui::Key::Escape),
                "tab" => Some(egui::Key::Tab),
                "backspace" => Some(egui::Key::Backspace),
                "delete" | "del" => Some(egui::Key::Delete),
                "space" => Some(egui::Key::Space),
                "up" | "arrowup" => Some(egui::Key::ArrowUp),
                "down" | "arrowdown" => Some(egui::Key::ArrowDown),
                "left" | "arrowleft" => Some(egui::Key::ArrowLeft),
                "right" | "arrowright" => Some(egui::Key::ArrowRight),
                "home" => Some(egui::Key::Home),
                "end" => Some(egui::Key::End),
                _ => None,
            };
            Ok(with_ui(|ui| match key {
                Some(k) => ui.input(|i| i.key_pressed(k)),
                None => false,
            }))
        });

        // focus_next() — one-shot; the next `text_edit` takes focus.
        m.add_method("focus_next", |_, _, ()| {
            FOCUS_NEXT.with(|c| c.set(true));
            Ok(())
        });

        // set_zoom(factor) — global text+widget zoom. egui defaults to
        // desktop-tool density (Body=13pt); doc-style apps want ~1.15–1.30.
        m.add_method("set_zoom", |_, _, factor: f32| {
            with_ui(|ui| ui.ctx().set_zoom_factor(factor));
            Ok(())
        });

        // ── Popups ──────────────────────────────────────────────────────
        // Thin wrappers around egui's popup memory. `popup_below_last`
        // anchors against the most recent `text_edit`'s response — enough
        // for slash-menu suggestions below the currently-editing block.

        m.add_method("open_popup", |_, _, popup_id: String| {
            with_ui(|ui| {
                let id = egui::Id::new(("popup", popup_id));
                egui::Popup::open_id(ui.ctx(), id);
            });
            Ok(())
        });

        m.add_method("close_popup", |_, _, popup_id: String| {
            with_ui(|ui| {
                let id = egui::Id::new(("popup", popup_id));
                egui::Popup::close_id(ui.ctx(), id);
            });
            Ok(())
        });

        m.add_method("is_popup_open", |_, _, popup_id: String| {
            Ok(with_ui(|ui| {
                let id = egui::Id::new(("popup", popup_id));
                egui::Popup::is_id_open(ui.ctx(), id)
            }))
        });

        // popup_below_last(popup_id, body_fn) -> bool
        //
        // Renders body inside a popup anchored below the most recent
        // `text_edit`. Returns whether the popup is open. No-op (false) if
        // no text_edit ran yet this frame.
        #[allow(deprecated)]
        m.add_method("popup_below_last", |_, _, (popup_id, body): (String, Function)| {
            let resp_opt = LAST_TEXT_EDIT_RESPONSE.with(|c| c.borrow().clone());
            let Some(resp) = resp_opt else {
                return Ok(false);
            };
            let (inner_err, was_open) = with_ui(|ui| {
                let id = egui::Id::new(("popup", popup_id));
                let mut err: Option<mlua::Error> = None;
                let inner = egui::popup_below_widget(
                    ui,
                    id,
                    &resp,
                    egui::PopupCloseBehavior::CloseOnClickOutside,
                    |inner_ui| {
                        with_child_ui(inner_ui, || {
                            if let Err(e) = body.call::<()>(()) {
                                err = Some(e);
                            }
                        });
                    },
                );
                (err, inner.is_some())
            });
            if let Some(e) = inner_err {
                return Err(e);
            }
            Ok(was_open)
        });

        m.add_method("modifiers", |lua, _, ()| {
            let (shift, ctrl, alt, mac_cmd) = with_ui(|ui| {
                let m = ui.input(|i| i.modifiers);
                (m.shift, m.ctrl, m.alt, m.mac_cmd)
            });
            let tbl = lua.create_table()?;
            tbl.set("shift", shift)?;
            tbl.set("ctrl", ctrl)?;
            tbl.set("alt", alt)?;
            tbl.set("mac_cmd", mac_cmd)?;
            Ok(tbl)
        });

        // drop_zone(fn) -> string | nil
        //
        // Wraps egui's dnd_drop_zone. Returns the dropped payload if a
        // drag_source was released over the zone this frame, else nil.
        m.add_method("drop_zone", |_, _, f: Function| {
            let (inner_err, dropped): (Option<mlua::Error>, Option<String>) = with_ui(|ui| {
                let mut err: Option<mlua::Error> = None;
                // Frame::NONE — default group bg would show at rest; egui
                // still tints on hover during a drag.
                let (_, payload): (_, Option<Arc<String>>) =
                    ui.dnd_drop_zone(egui::Frame::NONE, |child| {
                        with_child_ui(child, || {
                            if let Err(e) = f.call::<()>(()) {
                                err = Some(e);
                            }
                        });
                    });
                let dropped = payload.map(|p| (*p).clone());
                (err, dropped)
            });
            if let Some(e) = inner_err {
                return Err(e);
            }
            Ok(dropped)
        });
    }
}
