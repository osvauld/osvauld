# UI Automation

**Status:** 🟢 active — 2026-05-10
**Owner:** Abraham + Claude
**Related:** [widgets_roadmap.md](widgets_roadmap.md) (consumes this — drag tests).

## Goal

Drive the **live, user-visible** Slint window from outside the process
(via the existing Unix-socket control server). Capture **screenshots and
GIFs** of UI events plus a **per-frame state log** so tests can assert on
mid-animation values, not just final state.

When this lands, a python e2e test can do:

```python
with peer.ui_record(gif="/tmp/drag.gif", states="/tmp/drag.jsonl"):
    peer.ui_mouse_drag(from=(grip_x, row0_y), to=(grip_x, row5_y))

# Visual: open /tmp/drag.gif
# Programmatic:
states = peer.read_record_states("/tmp/drag.jsonl")
intents = [s["state"]["hover_intent"] for s in states if s["state"]["dragging"]]
assert "above" in intents and "below" in intents
ys = [s["state"]["cursor_y"] for s in states if s["state"]["dragging"]]
assert ys[0] < ys[-1]
```

## Why now

We just landed Draggable / DropTarget / DragLayer in `widgets/`. The
e2e test only validates `tree:move` via Lua eval — it never touches the
actual drag UI. Bugs like *"drop indicator on wrong row"* and *"hover
flickers entering grip"* are invisible to the data-layer tests; the user
has to reproduce them by hand and describe what they see. A proper UI
test loop unblocks every interactive widget that comes after this:
infinite canvas, command palette, context menus, multi-select.

**Decision: NOT using `i-slint-backend-testing` + its built-in MCP server.**
That crate replaces winit, so the user doesn't see a real window. We
want simultaneous live rendering (for human inspection during dev) and
remote control (for the agent / tests). That means a from-scratch path
on top of Slint's public API.

## Design

### Mechanism

Slint exposes everything we need as **public API** on the regular winit
backend:

- `slint::Window::dispatch_event(WindowEvent::PointerPressed/Moved/Released)`
  — synthetic input flows through Slint's normal event routing, hits
  TouchAreas, triggers `moved` / `pointer-event` callbacks identically
  to a real cursor.
- `slint::Window::take_snapshot() -> SharedPixelBuffer<Rgba8Pixel>`
  — captures the current frame as RGBA pixels; we encode to PNG (`image`
  crate) or GIF (`image::codecs::gif::GifEncoder`, already a transitive
  dep).
- `slint-interpreter::ComponentInstance::get_global_property(global, prop)`
  — reads any global property, e.g. `DragController.cursor-x`. Used for
  the per-frame state log.

No new internal-API dependencies. Slint API surface stays stable.

### Threading

`dispatch_event` and `take_snapshot` must run on the slint event-loop
thread. The control server runs on tokio. We bridge via the **existing
per-app slint Timer** (`launch.rs`, ~16 ms tick) — it already has access
to `RunningSlintApp.slint_runtime.slint_instance()`. Add a new
`mpsc::Receiver<AppUiCommand>` to `RunningSlintApp`; the timer drains it
each tick and dispatches.

For commands with reply (screenshot path, window size), use
`tokio::sync::oneshot` reply channels embedded in the command.

### Drag pacing

Doing 30 dispatch_events in one timer tick collapses to a single paint —
visually the cursor teleports. We want **user-like motion**, ~500 ms
end-to-end with one move per tick (60 Hz → 30 frames). Implementation:

- `Drag` command stores its trajectory + state machine in
  `RunningSlintApp.active_drag: Option<DragState>`.
- `DragState` = `{ from, to, total_steps, current_step, button }`.
- Each timer tick advances one step, dispatches a `PointerMoved` at the
  interpolated position.
- On final step → `PointerReleased`, drop the state.
- Guard: only one active drag at a time; second `Drag` returns error.

This makes drags **temporally observable** — the GIF and state log span
the full motion.

### GIF + state recording

A *recording* is a span between `RecordStart` and `RecordStop`. While
active, the timer:

1. Snapshots the window once per `1/fps_target` (default 30 fps).
2. Reads a configurable list of properties via
   `get_global_property(...)` and stores them as a JSON object.
3. Appends both to in-memory buffers.

Hard caps: 600 frames (~20 s @ 30 fps) to bound memory; older frames
dropped FIFO with a warning. A drag is over in <1 s, so this never
trips in practice.

`RecordStop` consumes the buffers:

- **GIF:** `GifEncoder::encode_frame` per `Frame`. Output path returned
  to the caller.
- **State log:** newline-delimited JSON (`.jsonl`), one object per
  captured frame: `{ t_ms, frame_idx, state: { ... } }`.

Default state captures the **DragController globals**: `dragging`,
`cursor-x`, `cursor-y`, `payload`, `kind`, `ghost-label`, `hover-target`,
`hover-intent`. Apps can extend by exposing a Lua function (e.g.
`get_record_state()`) the renderer calls each tick — that pushes
custom state into the JSONL alongside the globals.

### Single-window MVP

`ShellHandler` tracks **the latest launched app's** ui-tx. Multi-window
is a v2 (key by `page_id`). The editor test only needs one window.

## File-level changes

| File | Change |
|---|---|
| `renderer_slint/src/types.rs` | New `AppUiCommand` enum, `DragState` struct, `RecordingState` struct. Extend `RunningSlintApp` with ui_command_rx, active_drag, recording. Extend `LaunchedApp` with `app_ui_tx`. |
| `renderer_slint/src/launch.rs` | In the per-app timer closure: drain `ui_command_rx`, advance active drag, capture frame + state if recording. |
| `renderer_slint/src/lib.rs` | Re-export `AppUiCommand` for sthalam to use. |
| `sthalam/src/control_server.rs` | `ShellHandler::set_app_ui_channel(tx)`. New methods: `ui_window_size`, `ui_mouse_move`, `ui_mouse_press`, `ui_mouse_release`, `ui_mouse_drag`, `ui_screenshot`, `ui_record_start`, `ui_record_stop`. Each forwards via the channel. |
| `sthalam/src/main.rs` | In the collector_timer where eval is wired, also `server.set_app_ui_channel(launched.app_ui_tx)`. |
| `scripts/osvauld/scenario.py` | `PeerHandle.ui_*` helpers + `ui_record` context manager. |
| `e2e_tests/test_text_editor_drag.py` | New: 12 blocks, drag grip from row 0 to row 5, assert tree-order changes, assert intent transitions in JSONL, GIF for human review. |

## Sequencing

### Phase 1 — Core dispatch (PR 1)

- `AppUiCommand` enum with variants: MouseMove, MousePress, MouseRelease,
  Drag, Screenshot, WindowSize.
- Channel + timer-loop drain in `RunningSlintApp`.
- `ShellHandler` methods for all of the above.
- Python `PeerHandle.ui_mouse_move`, `ui_mouse_press`, `ui_mouse_release`,
  `ui_mouse_drag`, `ui_screenshot`, `ui_window_size`.
- Smoke test: drag the editor's grip end-to-end, assert tree order
  changes. No GIF yet.

**Deliverable:** I can drive a real drag remotely and verify the
post-drag state. Drag pacing is user-like (one step per tick).

### Phase 2 — GIF + state recording (PR 2)

- `RecordStart` / `RecordStop` commands.
- Frame buffer + GIF encoder.
- Per-frame state capture via `get_global_property` (default
  DragController set; configurable list in RecordStart).
- Python `peer.ui_record(...)` context manager.
- `e2e_tests/test_text_editor_drag.py`: 12 blocks, drag from row 0 to
  row 5, GIF, JSONL, assertions on intent transitions.

**Deliverable:** First UI test that asserts on mid-animation state.
GIF artefact for visual review on failure (or always).

### Phase 3 — Custom Lua state hook (PR 3)

- Renderer calls `get_record_state()` (Lua function) each tick if
  defined; merges its return value into the state JSON.
- App-defined per-row state: source row opacity, indicator visibility,
  ghost position, etc.
- Update editor test to assert these.

**Deliverable:** Apps can expose any state they want for assertion
without renderer-side changes. Tests become *deeply* introspective.

### Phase 4 — Polish (later)

- Multi-window: key the channel by `page_id`.
- Element queries (find by id / type) — would need to walk Slint
  component tree.
- Slow-motion playback option for GIFs (configurable fps).
- Test framework integration: auto-record around any test that calls
  a `ui_*` method, dump on failure.

## Open questions

1. **GIF size at 800×600 × 30 fps × 1 s.** RGBA frames = ~58 MB raw,
   GIF should be ~2–5 MB after palette quantisation. Acceptable. If
   tests start dumping many GIFs we may want optional downsample.
2. **Lua hook timing.** `get_record_state()` runs on the lua worker
   thread, async to the slint timer. We need a synchronous bridge per
   tick OR accept that state is captured on a slight delay relative to
   the frame. Latter is fine for assertions; the timestamp lands where
   the *snapshot* was taken.
3. **Recording cap behaviour.** When the 600-frame cap is hit, do we
   stop recording (silently truncate) or evict FIFO? FIFO is friendlier
   but the timestamps may be misleading. Lean toward "stop + warn."

## Out of scope

- Replacing the live winit backend (`i-slint-backend-testing`'s built-in
  MCP server does this; rejected — see "Why now").
- Element-by-id queries (Phase 4).
- Cross-process coordination of multiple sthalam instances during a
  single recording.
- Video formats other than GIF.

## Definition of done

Phase 1: editor drag test passes via UI automation (no manual reproduction).
Phase 2: same test produces a GIF + JSONL, both checked into a known
output directory; a deliberately-broken drag (e.g. wrong target id) is
caught by the JSONL assertions before the post-drop tree assertion.
Phase 3: editor test asserts source-row dim opacity transitioned 1.0 → 0.25
during drag, sourced from a Lua-defined `get_record_state()`.
