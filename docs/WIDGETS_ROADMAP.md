# Widgets Roadmap

Living plan for the `widgets/` crate, the block-based text editor, the
infinite-canvas direction, and the broader widget/capability story. Scope here
is **what's next**, not the full end-state. Architectural pieces explicitly
deferred are listed at the bottom.

## Where we are (2026-05)

**Milestone 1 — done.** `widgets/` crate, `RichTextEdit` exposed via the
`@osvauld/widgets.slint` library, registered on the slint-interpreter
`Compiler` in `renderer_slint`. Sample app `richtext-hello` and e2e test
`test_richtext_hello.py` confirm the import path works end-to-end.

**Milestone 1.5 — done.** `scribe:bind_text(prop, layer, key)` declarative
binding pushes a string-valued map key to a Slint property. Engine-side
remote-update path fetches a fresh layer snapshot when the observer omits
`full_data` (delta-only path), so live sync reaches the bound property.

**Step 0 — done.** LoroTree as a layer type. `tree` joins `map`, `list`,
`movable_list` as a layer kind. Wire-format `TreeOp` + `LoroDelta::Tree`,
scribe operations + observer, `LuaLoroTree` userdata, `scribe:tree(name)`
accessor, `scribe:bind_tree(prop, layer)` reactive binding (with surgical
`Set` vs `Replace` diff via `BindingManager::diff_tree_rows`). Round-trip
test `test_tree_layer_round_trip` passes.

**Step A — done.** `sample_apps/text-editor/` ships a tree-backed editor.
Each tree node is a block (`{kind, text}`), rendered as a flat list of
`RichTextEdit` rows keyed by node id. Enter splits, Backspace-at-empty deletes,
toolbar (P/H1/H2/Code) toggles `kind` on the focused block via
`active_block_id`. Two-peer sync verified by `test_text_editor.py`. Fast-typing
race fixed via `external-text` echo guard (skip echo while editor focused).

**Step C primitives shipped along the way.** `RichTextEdit` now exposes
`key-pressed` / `key-released` callbacks (with `EventResult.accept`),
`cursor-position` / `anchor-position` byte offsets, and `forward-focus` for
programmatic focus delegation. Enough to drive the current editor; selection-
range and mark-aware cursor work waits for the parley-backed widget.

**LoroText platform support — done at the CRDT and binding layers.**
- `scribe::message`: `TextOp { Retain | Insert | Delete }`, `LoroDelta::Text`.
- `ScribeMessage`: `EnsureLoroText`, `TextInsert`, `TextDelete`,
  `TextSnapshot`, `TextLength`. Round-trip test `test_text_layer_round_trip`
  passes.
- `loro_observer`: `Diff::Text` → `LoroDelta::Text` (mark attributes are
  intentionally stripped at this layer for the MVP).
- `lua_runtime`: `LuaLoroText` userdata, `scribe:text(name)` accessor,
  `scribe:bind_loro_text(prop, layer)` reactive binding (cached snapshot,
  no-op suppression).

**LoroText is not yet composed into the editor.** Block text still lives as a
plain string in tree-node meta. Switching to a nested LoroText per block (so
intra-block edits merge char-level instead of LWW) is a separate step gated
on the parley-backed widget — without it, there's no UI surface that benefits
from char-level CRDT.

**Known limitations carried into next phase:**

- `RichTextEdit` is still a `TextInput` wrapper. No marks, no IME niceties.
- Per-keystroke writes against block text broadcast the whole string (LWW per
  meta key). Concurrent same-block edits clobber. Acceptable for single-author,
  needs the LoroText-per-block composition + mark rendering for true collab.

## Next phase — lists and structure

Goal: stress the tree + indent path with real per-app composition before
investing in the render layer.

### Step B — first nesting case (~2 days, no platform changes)

Add `list` and `list_item` block kinds to the editor app.

- **Behaviors:**
  - Tab on a list_item → reparent under previous sibling (indent).
  - Shift-Tab → reparent up (outdent).
  - Enter on a list_item → create sibling list_item.
- **Slint:** kind-aware row dispatch (paragraph rows vs list_item rows with a
  bullet glyph + indent margin = `block.depth * 24px`).
- **Lua:** dispatcher in `on_field_changed` gains `indent:` / `outdent:`
  actions calling `tree:move(id, new_parent, idx)`.
- **Validation:** `diff_tree_rows` may need to verify that re-parents (same
  ids, changed `depth`/`parent`) emit `Set` rather than `Replace`. If they
  fall through to `Replace`, focus dies on indent — likely fix at the binding
  layer.

This is *pure app-level composition* over Step 0 — no scribe / binding /
widget changes. It's the load-bearing test of LoroTree as a backbone.

### Step B' — marks at the CRDT layer (~half-day, opportunistic)

Cheap and unblocks future work. Stop discarding mark attributes in
`text_delta_to_op`; carry them through `TextOp::Insert { content, attrs }` and
`TextOp::Retain { count, attrs }`. Add `TextMark` / `TextUnmark` scribe
messages and Lua bindings. Snapshot remains a flat `String` for now —
attributes flow as deltas only until a renderer needs spans.

This commits the data side without committing the renderer, so we can
experiment with formatting in tests / scripts before paying for the widget.

## Render layer investment (when ready)

The custom-render M2 plan is the gating risk for both rich-text formatting
*and* the infinite canvas. The two share most of their foundation:

| Layer | Editor M2 | Canvas |
|-------|-----------|--------|
| LoroTree + LoroText (CRDT) | ✓ (already in place) | ✓ |
| parley text layout | ✓ | ✓ |
| Custom paint (vello / painter cb) | ✓ | ✓ |
| Hit-testing primitives | ✓ | ✓ |
| Selection state machine | ✓ | ✓ |
| Viewport transform (pan/zoom) | – | ✓ |
| Item drag / resize | – | ✓ |

**Decision flagged for that point: which consumer drives v1 of the render
stack, editor or canvas.**

- **Editor-first** lights up bold/italic in the existing app, but the caret-
  into-marks + selection-spanning-marks + IME work is the hardest variant of
  custom text editing. High risk for the first consumer.
- **Canvas-first** treats sticky notes / labels as the v1 consumer — text is
  styled and laid out via parley, but no caret-spanning-marks, no IME inside
  the canvas item. Validates parley + paint + hit-testing on the easier shape;
  editor M2 then reuses a proven foundation for the harder caret problem.
  Side benefit: an actual infinite canvas demo earlier.

Current lean: **canvas-first**. Editor stays usable as-is in the meantime;
formatting is a polish item, not a blocker. We re-evaluate when we get here.

## Deferred (intentionally out of scope here)

Real items, but not blocking the next phase. Listed so we don't lose them.

- **LoroText composed into block meta.** Tree node's meta map holds a nested
  `LoroText` for `text` instead of a plain string. Char-level concurrent typing
  converges. Gated on having a renderer that visibly benefits from it
  (today's TextInput-wrapper would still LWW-overwrite via `bind_text`).
- **Parley-backed `RichTextEdit`.** Replace `TextInput` body with parley
  shaping + custom paint inside a Slint `ComponentContainer`. Unblocks marks
  rendering, IME, real selection. See render layer section.
- **Loro Awareness 3-tier presence.** Awareness state for cursor/selection/
  typing per peer (LWW with TTL/heartbeat), separate from the existing
  CRDT-subscription presence. Bigger refactor: scribe + lua_runtime +
  permits + sample apps. Defer until a feature actually needs live remote
  cursors.
- **Capability-first policy refactor.** Roles → capabilities as the primary
  permit unit. UCAN-cap as the lingua franca; roles compile down to cap
  bundles. Touches `policy_model`, `osv_decl`, `gurkha`. ~1.5 weeks.
- **Widget composition / `widget.osv` packaging.** Widgets as portable
  vertical slices (UI + data sync + identity within osvauld) consumed via
  `use widget X as Y { config = ... }` in `app.osv`. Hinges on the capability
  refactor for clean cap vocabulary. ~1 week after caps land.
- **Lazy layer materialization tightening.** Layers stay on disk until widget
  visibility demands them. Mostly true today; needs explicit hooks.
- **Generic AppAPI callback registration.** Renderer currently hardcodes a
  whitelist of callback names auto-wired to Lua (`on_field_changed`, etc.).
  App-defined callbacks fire in Slint but never reach Lua. Replace with a
  data-driven registration so apps can declare custom callbacks without a
  renderer change.
- **Unified mutation message per container kind.** Today's typed
  `TextInsert` / `TextDelete` / `TreeMove` etc. work fine but proliferate
  variants. Could collapse to one `<Container>Mutate { op }` per kind.
  Schema cleanup, not a fundamental shift.

## Sequencing rationale

We're building bottom-up: foundation (LoroTree, LoroText) → consumers (editor,
canvas) → polish (marks, IME, capability refactor). Each layer has a real
consumer before the layer below it is hardened, so design pressure comes from
actual use rather than imagined requirements. The render-layer investment is
the next big rock; staging it behind canvas keeps the first consumer simple
enough to validate the foundation, leaving editor M2 as the harder second
consumer that gets a proven base.
