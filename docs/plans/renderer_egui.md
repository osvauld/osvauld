# Renderer: egui

**Status:** 🟢 active — 2026-05-13 (pivot to in-process via GLFW; subprocess approach explored and reverted)
**Owner:** Abraham + Claude
**Related:** [widgets_roadmap.md](widgets_roadmap.md) (drag/canvas interactivity this would unblock), [ui_automation.md](ui_automation.md) (testing approach to mirror).

## Goal

Add a third renderer crate, `renderer_egui`, alongside `renderer_slint`
and `renderer_raylib`. Apps that declare `"renderer": "egui"` in their
manifest run as immediate-mode UIs driven from Lua, with built-in
support for drag/drop, pan/zoom, custom painting, and reactive repaint.

The Slint and Raylib renderers stay. Apps choose per-app:
- **Slint** for form/list-heavy apps where declarative layout pays.
- **Raylib** for canvases/games and continuous animation.
- **egui** for interactive tools, editors, and anything that wants
  drag/drop/pan/zoom without fighting the framework.

When this lands, the `sample_apps/text-editor` block editor can ship in
both Slint and egui variants — and the egui variant is shorter, drops
the `drag_engine/` + `drag.slint` + `virtual_pointer.slint` machinery,
and supports pan/zoom natively.

## Why now

- The block editor and canvas work in `widgets_roadmap.md` keeps hitting
  the limits of Slint's input model. Drag/drop, pan/zoom, and per-widget
  pointer state aren't first-class in Slint — we're patching them in via
  `drag_engine/`, `drag_glue.rs`, and a virtual-pointer Slint component.
- We already have a two-renderer architecture (manifest-keyed dispatch in
  `sthalam/src/main.rs:406/:571`), so a third renderer is an additive
  change, not a refactor.
- Lua is the upload artifact, not Rust — apps don't care that egui is
  Rust internally. They see a Lua binding surface.

## Design

### Where it fits

```
sthalam dispatches on manifest.renderer:
  "slint"  -> renderer_slint::launch_slint_app  (existing)
  "raylib" -> renderer_raylib::spawn_app        (existing)
  "egui"   -> renderer_egui::spawn_app          (new)
```

Manifest already has `renderer: String` with a `slint` default
(`domains/src/manifest.rs:8-10`). The new crate is a peer of the
existing two, not a replacement.

### Crate skeleton

```
renderer_egui/
├── Cargo.toml             # glfw, egui, egui_glow, glow, mlua, butler,
│                          # scribe, domains, lua_runtime, tracing
└── src/
    ├── lib.rs             # pub: spawn_app, prepare_page, validate
    ├── glfw_host.rs       # GlfwEguiHost — window, GL ctx, input mapping
    ├── egui_runtime.rs    # owns EguiApp; per-frame Lua dispatch
    ├── ui_bindings.rs     # LuaUi userdata: widgets
    ├── layout_bindings.rs # available_width, grid, sized, fill, scroll_area
    ├── style_bindings.rs  # color/frame/font/spacing; painter primitives
    ├── input_bindings.rs  # drag/drop/pointer/keyboard state surfaced to Lua
    ├── asset_loader.rs    # fonts + images from app dir into egui textures
    ├── page_runtime.rs    # mirrors slint's: load manifest, lua, set up scribe
    ├── validation.rs      # minimal: "lua loads, on_frame defined"
    └── types.rs           # RunningEguiApp, EguiAppCommand
```

Shape-wise closer to `renderer_raylib` (immediate mode, owns its event
loop) than `renderer_slint` (declarative interpreter, heavy machinery).
`renderer_raylib/src/lib.rs` is the working template.

### Lua API surface

This is the product. The bridge defines what apps can express.

**Lifecycle (host calls into Lua):**
```
on_init(ctx)        -- once, after scribe/butler are ready
on_frame(ui)        -- every repaint
on_key(ev)          -- key events (optional)
on_close()          -- before window closes
```

**Widgets — minimal v1 set (~10):**
```
ui:label(text, style?)
ui:button(text, style?) -> bool
ui:text_edit(value, opts?) -> (new_value, changed)
ui:text_edit_multiline(value, opts?) -> (new_value, changed)
ui:checkbox(value, label) -> new_value
ui:image(name, opts?)
ui:separator()
ui:spinner()
ui:push_id(id, fn) / ui:pop_id()
ui:same_line()
```

**Layout (~10):**
```
ui:horizontal(fn) / ui:vertical(fn)
ui:horizontal_wrapped(fn)
ui:sized(w, h, fn) / ui:fill(fn)
ui:scroll_area(fn)
ui:scroll_area_rows(count, row_fn)        -- virtualized; required for large docs
ui:grid({ cols=n | min_item_width=px, gap }, fn)
ui:columns(n, fn)
ui:indent(px, fn)
ui:available_width() / ui:available_height()
```

**Styling (apps own it; no host theme lock-in):**
```
ui:frame({ bg, border, rounding, padding, margin }, fn)
ui:scope({ ...style overrides... }, fn)
ui:painter() -> { rect_filled, circle, line, text, image, allocate }
```
Style descriptors are Lua tables for v1 (friendly, allocates on each
call). If the FFI/GC cost shows up in benchmarks, swap to userdata.

**Interactivity — the reason this renderer exists:**
```
ui:drag_source(label, { payload }) -> handle
ui:drop_zone(fn) -> dropped_payload | nil
ui:pointer() -> { pos, delta, primary_down, scroll }
ui:hovered() / ui:clicked() / ui:dragged()   -- widget-return-value style
```

First cut: ~40 binding methods. Apps that fit Just Work; anything custom
drops to `ui:painter()`.

### Asset pipeline

Apps declare fonts and images in `manifest.json`:
```json
{
  "renderer": "egui",
  "fonts": [{ "name": "Inter", "file": "Inter.ttf" }],
  "images": ["logo.png", "icons/*.png"]
}
```

At launch, `asset_loader.rs`:
1. Reads font files → `egui::FontDefinitions` → `ctx.set_fonts(...)`.
2. Reads images → `egui::ColorImage` → `ctx.load_texture(name, ...)`,
   stored in a per-app `TextureHandle` registry.
3. Lua refers to assets by name: `ui:image("logo.png")`,
   `{ font = "Inter" }` in style tables.

### Event loop & threading

The original plan was to run egui via `eframe` in a spawned thread, the
way `renderer_raylib` runs raylib in a spawned thread. That collided
with reality on 2026-05-12:

```
ERROR renderer_egui eframe::run_native error,
  error=winit EventLoopError: EventLoop can't be recreated
```

`eframe` is built on winit. Sthalam's Slint shell already owns a winit
`EventLoop` in the process, and winit refuses a second one — even from
a different thread. `renderer_raylib` avoids this because raylib uses
GLFW, not winit; GLFW has no global event-loop singleton.

**Two paths were considered out of this:**

1. **Subprocess.** Launch egui apps as a child process of sthalam, with
   the daemon and child communicating over a Unix-socket RPC. Reuses no
   in-process bindings; every scribe call round-trips through IPC. See
   `app_ipc.md` for the protocol design we wrote down before pulling
   back.
2. **In-process via a non-winit backend.** Replace `eframe` with a thin
   GLFW + `egui_glow` host inside `renderer_egui`. Same escape hatch
   raylib already uses. Apps stay in the sthalam process, scribe
   bindings are direct Rust calls, multi-window is a thread per app.

**Decision (2026-05-13): in-process via GLFW.**

Reasoning:

- **The collision is a windowing problem, not a trust problem.** Using
  a process boundary to dodge a winit constraint is a sledgehammer for
  a nail. The right tool is a different windowing primitive.
- **No new trust gained by going subprocess today.** Every app on the
  system is one we wrote or trust. The kernel-enforced isolation that
  subprocess buys is real, but the gap to a well-audited in-process
  binding layer is not currently exploitable.
- **Engineering surface is much smaller.** Subprocess requires the
  whole `app_ipc` crate — every scribe method serialised on a wire,
  protocol versioning, framing, error variants, lifecycle. In-process
  is a ~500-1000 line GLFW input-mapping shim, then everything else
  already works.
- **Path to subprocess is preserved.** If/when we open the door to
  running peer-shipped untrusted apps on our node, subprocess becomes
  the right architecture for *security* reasons, not windowing. The
  `app_ipc.md` plan is parked, not discarded.

**Threading model.**

GLFW has its own quirks: on macOS, `glfwInit` and event polling must
happen on the main thread (Cocoa requirement). On Linux/X11 and
Wayland, this is looser — `renderer_raylib` runs in spawned threads
today and works. We are Linux-first; macOS support is an open question
to validate when the platform matters.

Each egui app gets its own OS thread inside sthalam, owning its GLFW
window, GL context, egui `Context`, Lua VM, and scribe `ActorRef`. Lua
runs **inline** on the egui thread — `on_frame` work should be
lightweight by design; a channel hop adds latency that defeats the
point of immediate-mode UI.

**Crash isolation.** None for native crashes; an app's segfault takes
the whole sthalam process. Lua panics are caught around `on_frame` so a
buggy Lua script doesn't crash the host. We accept this — it's the
same trust posture as raylib apps today.

### Repaint model

Default to **reactive repaint** (egui's normal mode): only redraw on
input events or animation. Apps that want continuous animation call
`ctx:request_repaint()` explicitly. This is what keeps idle CPU at zero
and matches Slint's "static unless something changed" feel.

### Performance — what we know, what to measure

| Cost | Mitigation |
|---|---|
| Per-frame Lua FFI calls (200 widgets × 60fps = 12k mlua calls/sec) | Phase 0 benchmark before committing the binding shape. If bad, switch to batched widget descriptor table. |
| Loro tree walk every frame on large docs | Cache walk in Rust, invalidate on CRDT-change events. Out of scope for v1; design the binding so this is addable later. |
| Large lists | Expose `ui:scroll_area_rows(count, row_fn)` → `egui::ScrollArea::show_rows` from v1. |
| GC churn from style tables | Pool common descriptors host-side later if needed. |

Phase 0 is non-negotiable.

### Phase 0 results (2026-05-12, release build)

Bench: `bench_egui_lua/` — eframe 0.34 + mlua 0.10 (lua54), thread-local
`egui::Ui` pointer pattern, ~5 binding methods (label, button, separator,
horizontal, vertical). Stress mode adds a nested `horizontal` per row.

| Rows | Mode | Lua ms | FFI/frame | Wall ms (FPS) | Per-FFI cost |
|---:|---|---:|---:|---:|---:|
| 200 | stress | 1.53 | 1,810 | 16.67 (60) | ~845 ns |
| 1000 | stress | 6.50 | 9,042 | 16.66 (60) | ~720 ns |

**Interpretation.**
- Per-FFI cost is sub-microsecond *including the egui call it triggers*. The
  mlua bridge itself is not a bottleneck at the widget densities apps will
  use.
- Both runs are vsync-bound (16.66 ms wall = 60 Hz). Lua consumes ~9% of
  frame budget at 200 rows, ~39% at 1000 rows.
- Linear scaling between the two data points confirms no superlinear cost
  (no quadratic anywhere in the bridge).

**What this does and doesn't prove.**
- ✅ Thread-local `Ui*` + `UserData` method dispatch is viable.
- ✅ Table-based binding (vs batched descriptors) is fast enough for v1.
- ✅ Apps with hundreds of widgets per frame are comfortably real-time.
- ⚠️ Release build only — debug builds will be much slower; never bench
  egui in debug.
- ⚠️ Bench uses only `label/button/horizontal/separator`. Real apps will
  hit `text_edit` (expensive in egui itself) and `painter` (small per-call
  but allocates display-list entries).
- ⚠️ Doesn't measure: GC pressure under sustained run, Loro tree walks
  per frame, asset loads.

**Decision.** Binding shape locked for phase 1. Reopen if real apps in
phase 5 show > 4 ms Lua time at typical widget counts.

### Validation

`renderer_slint/src/validation.rs` is heavy because the Slint DSL has a
parser to invoke. egui validation is minimal: load the Lua, check
`on_frame` is defined, check declared assets exist. Fast and shallow.

## Sequencing

| Phase | Scope | Deliverable | Gate |
|---|---|---|---|
| 0 ✅ | Bench Lua-FFI overhead on a 200-widget egui frame from mlua | `bench_egui_lua` crate; numbers below | Cleared 2026-05-12 |
| 1 ✅ | Crate skeleton + Lua entry + 6 widgets (label, button, horizontal, vertical, text_edit, separator) | Hello-world egui app renders, sthalam dispatch wired | Cleared 2026-05-12 (build green; end-to-end run deferred) |
| A ⚫ | Subprocess pivot (eframe in a child process, app_ipc shim) | Subprocess plan + runner binary | Explored; reverted 2026-05-13 in favour of phase G (in-process via GLFW) |
| G1 | Standalone GLFW + `egui_glow` proof — open a window, run a frame loop, paint, no sthalam, no Lua | `bench_egui_glfw` crate runs; deps pinned; threading-on-Linux validated | Bench renders egui demo at 60 Hz from a spawned thread |
| G2 | Extract `GlfwEguiHost` wrapper inside `renderer_egui` | `host.run(\|ctx\| ...)` API takes a frame closure | Wrapper unit-testable; no eframe dep in `renderer_egui` |
| G3 | Rewire `spawn_app` to thread (drop subprocess path); restore `EguiApp::new` full butler+scribe wiring | `renderer_egui::spawn_app` launches in-process; `hello-egui` renders inside sthalam | `hello-egui` runs end-to-end with no winit error |
| G4 | Wire scribe wake-ups → `ctx.request_repaint()` | Reactive repaint on remote CRDT changes | Two-peer `drag-list-egui` reorders sync visually |
| G5 | Delete subprocess scaffolding (`bin/renderer_egui_runner.rs`, env var, `[[bin]]`, eframe dep); update `app_ipc.md` to "deferred" in plans README | Clean tree | `cargo build -p sthalam --features egui` green; no dead code |
| G6 | End-to-end validation: hello-egui, drag-list-egui, two-peer drag sync, multiple egui apps concurrently | Working multi-app demo | Manual smoke + screenshot in plan |
| 2 | Layout + styling (frame, scope, color/font tables) + scroll_area | Real-looking app | Manual smoke |
| 3 | Drag/drop + pointer + painter (extend beyond the v1 binding set) | Drag-reordering list demo with custom painter | Manual smoke |
| 4 | Asset pipeline (fonts + images) + manifest validation | Production-ready surface | `validate_egui_app` passes for a real app |
| 5 | Port `sample_apps/text-editor` to egui (alongside the Slint version) | Apples-to-apples comparison | Both versions feature-parity |
| 6 | Headless test integration — mirror `MockScribeHandle` story for egui via `app_test` crate | Egui apps testable without a window | `cargo test -p app_test` covers egui apps |

Phases G1-G6 are the in-process migration; they unblock everything
downstream of phase 1. Phase A is kept in the table as a record of the
direction we explored — useful when someone later asks "did we
consider subprocess?"

Phase 5 is the real validation: if the egui text-editor is meaningfully
worse than the Slint one, the binding API is wrong and we iterate
before going wider.

## Open questions

1. **Single-window vs multi-window?** Recommend single for v1. Confirm
   no near-term need for multiple concurrent egui apps.
2. **Style descriptors: tables or userdata?** Tables for v1; revisit if
   benchmarks justify.
3. **Where does `tree:walk()` invalidation live?** A per-app cache in
   `lua_runtime` or in the renderer? Probably `lua_runtime` so it
   benefits all renderers, but out of scope for v1.
4. **Custom widget extension story.** Apps that want a node-graph or a
   timeline widget — do they compose from `ui:painter()` each frame, or
   do we eventually let apps register reusable widgets host-side?
   Defer; `painter()` is enough for v1.
5. **How much of `drag_engine/` and `drag_glue.rs` is reusable?** Worth
   reading once before phase 3 to lift any input math (hit-testing,
   intent zones) instead of redoing it.
6. **App-level routing.** If an app declares `"renderer": "egui"` but
   the host build excluded the feature, fail loudly at validation time,
   not at launch.

## Out of scope

- Replacing Slint or Raylib. Both stay; egui is additive.
- A host-imposed design system. Apps style themselves freely; we ship a
  reasonable default `egui::Style` but apps can override anything.
- Multi-window egui apps in v1.
- Hot-reload of egui apps (Slint's hot-reload story is its own beast;
  egui apps reload by relaunch for v1).
- Cross-renderer shared widgets (e.g. one widget that renders in both
  Slint and egui). Apps pick a renderer; widgets are renderer-specific.

## Related

- `widgets_roadmap.md` — the interactivity work that egui makes
  tractable (drag/drop, canvas, block editor).
- `ui_automation.md` — once egui apps exist, automation needs to drive
  egui windows too. Likely reuses the same control-socket approach with
  an egui-specific event injection path.
- `sample_apps/text-editor/` — the phase-5 port target.
- `renderer_raylib/` — the shape template for the new crate.
- `renderer_slint/` — the heavier template for parts like
  `prepare_page`, asset handling, and scribe wiring.
