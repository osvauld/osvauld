# App IPC

**Status:** 🔵 next — 2026-05-13
**Owner:** Abraham + Claude
**Related:** [renderer_egui.md](renderer_egui.md) (the forcing function — egui apps must run as subprocesses, so they need this), [widgets_roadmap.md](widgets_roadmap.md) (downstream consumer once shape is right).

## Goal

A capability-aware RPC + event channel between sthalam (the daemon)
and Lua app subprocesses. Lua bindings on the child side look identical
to today's in-process `ScribeBindings`; under the hood they marshal
requests over a per-child Unix `socketpair` to a daemon-side server
that calls the same `ActorScribeHandle` / butler facade we already use,
checks permits, and pushes CRDT update events back to subscribed
children.

When this lands:
- An egui app running as a child process can do `scribe:tree("doc"):walk()`
  and get correct, live, permit-checked data.
- The Python scenario harness's readiness eval (`eval("return 1")`)
  reaches the child's Lua VM, so `test_drag_list_egui.py` passes.
- `drag-list-egui` rewritten against a Loro layer syncs between two
  peers exactly like the existing Slint chat does.
- The same IPC layer is usable by future `renderer_raylib_runner` and
  `renderer_slint_runner` subprocesses with zero changes — it's
  renderer-agnostic.

## Why now

- `renderer_egui` subprocess works as of 2026-05-12 but its
  `EguiApp::new_headless` deliberately skips scribe — the child has no
  backend. That's a deliberate scope-cut waiting for this plan.
- All three renderers will eventually want subprocess + IPC (multiple
  raylib instances, crash isolation, sandboxing, sthalam-as-daemon
  endgame). Building it once, generically, beats three half-versions.
- The IPC boundary is the right place to settle the **single-source-of-
  truth** story: daemon owns canonical Loro state, each child has a
  mirror fed by push events. Defer this and we'll grow ad-hoc copies in
  every renderer.

## Design

### Mental model

```
┌─ daemon (sthalam) ──────────────────────────────────┐
│                                                     │
│   scribe (ractor)  ◄──►  butler  ◄──►  courier      │
│        ▲                                            │
│        │  in-process ActorRef.send                  │
│        │                                            │
│   ┌────┴───────────────────────────┐                │
│   │ AppIpcActor (one per child)    │                │
│   │  - decode req, check permit    │                │
│   │  - dispatch into scribe/butler │                │
│   │  - forward subscribed events   │                │
│   └────────────────────────────────┘                │
└──────────────┬──────────────────────────────────────┘
               │  Unix socketpair (fd inherited at spawn)
               ▼
┌─ child (renderer_egui_runner) ──────────────────────┐
│                                                     │
│   Lua VM  ◄──► ScribeIpcBindings (Rust)             │
│                  - serialize req                    │
│                  - await reply                      │
│                  - on event: update mirror,         │
│                    call ctx.request_repaint()       │
└─────────────────────────────────────────────────────┘
```

- **Actors stay inside the daemon.** Across the boundary it's RPC, not
  remote-actor messaging. ractor_cluster considered, rejected — wrong
  trust topology (peer-to-peer with shared trust, vs our
  client-server with capability checks). See renderer_egui.md
  "Event loop & threading" for the trade-off table that led here.
- **No replica on the child side.** Daemon owns the Loro state; that's
  the only copy. `ScribeIpcBindings` is a stateless RPC client.
  `tree:walk()`, `text:read()` etc. are RPC calls that return fresh
  snapshots; the child returns them to Lua, the app uses them for the
  current frame, they're discarded. No mirror, no cache, no
  invalidation, nothing to drift.
- **Push events are wake-ups, not data.** Daemon emits
  `Event { topic: "tree:doc" }` (no payload) on every canonical change.
  Child translates topic→repaint: for egui it's `ctx.request_repaint()`.
  Next frame `on_frame` runs, calls `tree:walk()`, gets the new state.
  Subscriptions trigger work; RPC fetches data. Clean separation.
- **Capability checks happen daemon-side, per request.** No tokens
  cached at the child. Same permit logic as today, just on
  deserialized inputs.
- **Cost is fine.** Worst-case text-editor-shaped doc: 60 fps × 1
  walk/frame × ~200 KB snapshot = ~12 MB/s on the Unix socket. Kernel
  handles that at memcpy speed; serde-json adds <1% of a 16 ms frame
  budget. For 10k+ item docs, add `tree:walk_range(from, to)` and let
  the renderer's virtualized scroll area decide what to ask for.

### Transport

- **Per-child Unix `socketpair`.** Daemon creates it, keeps one fd,
  passes the other to the child via fd inheritance at spawn (set
  `FD_CLOEXEC=false` on the child's end, use `Command::pre_exec` or the
  `command-fds` crate). No filesystem path, no other process can
  connect — OS-enforced isolation.
- Linux only initially. macOS has the same primitive; Windows uses
  named pipes — defer until needed.
- Child discovers its IPC fd from env var `OSVAULD_IPC_FD=<n>`.

### Framing

- **Length-prefix + JSON.** Matches what `control_server` already does;
  reuse the framing code if reasonable. 4-byte little-endian length
  prefix, then a JSON envelope.
- JSON for v1: human-readable in tcpdumps, trivial serde wiring, fast
  enough for our message rates. Postcard/msgpack are obvious perf
  upgrades if profiling demands; framing stays the same.

### Message envelopes

Three message kinds on the wire, one direction each is well-defined:

```rust
// child → daemon
enum Request {
    Call { id: u64, method: String, params: serde_json::Value },
    Cancel { id: u64 },
}

// daemon → child
enum Reply {
    Response { id: u64, result: serde_json::Value },
    Error    { id: u64, error: IpcError },
    /// Push notification.
    /// - `payload = None`: CRDT canonical-state change. Child repaints
    ///   and re-queries via RPC. Topic identifies the layer.
    /// - `payload = Some(_)`: ephemeral message (scribe:send). No stored
    ///   state exists to re-query — the payload IS the message. Child
    ///   delivers to the app's subscriber callback.
    Event    { topic: String, payload: Option<serde_json::Value> },
}

struct IpcError {
    code: ErrorCode,      // PermitDenied, NotFound, BadRequest, Internal, ...
    message: String,
    details: Option<serde_json::Value>,
}
```

- `id` is a monotonic per-child counter the child mints. Daemon echoes
  it on responses. Events have no id — they're server-pushed.
- `Cancel` is for long-running calls; v1 doesn't need it but the
  envelope leaves room.
- `Event` rides the same socket — no second channel. `topic` is a
  string like `tree:doc` or `page_update`; subscriptions filter.

### Initial method surface

Mirrors today's in-process `scribe` Lua API (see `lua_runtime/src/bindings/
scribe.rs`). Apps target four CRDT data types plus layer/identity/ephemeral
ops. Each item below is one wire method; the envelope is unchanged.

**Identity / page context:**
- `page.context {}` → `{ page_id, my_did, my_name }`  (one shot at init)

**Tree (`scribe:tree(name):...`):**
- `tree.walk { layer }` → `[{ id, parent, depth, index, props }]`
- `tree.create { layer, parent, props }` → `{ id }`
- `tree.delete { layer, node_id }` → `{}`
- `tree.set_prop { layer, node_id, key, value }` → `{}`
- `tree.move { layer, node_id, new_parent, new_index }` → `{}`
- `tree.text_edit { layer, node_id, pos, del, ins }` → `{}`

**Text (`scribe:text(name):...`):**
- `text.read { layer }` → `string`
- `text.edit { layer, pos, del, ins }` → `{}`

**List (`scribe:list(name):...`):**
- `list.read { layer }` → `[value, ...]`
- `list.len { layer }` → `number`
- `list.push { layer, value }` → `{}`
- `list.insert { layer, index, value }` → `{}`
- `list.delete { layer, index }` → `{}`
- `list.set { layer, index, value }` → `{}`

**Map (`scribe:map(name):...`):**
- `map.read { layer }` → `{ key: value, ... }`
- `map.get { layer, key }` → `value | null`
- `map.set { layer, key, value }` → `{}`
- `map.delete { layer, key }` → `{}`
- `map.keys { layer }` → `[key, ...]`

**Layer / permit management:**
- `layer.list { pattern }` → `[layer_name, ...]`
- `layer.create { template, args, authorized? }` → `{ layer_name }`
- `layer.grant { layer, dids }` → `{}`

**Ephemeral pub/sub (`scribe:send`):**
- `ephemeral.send { topic, payload }` → `{}`
  - Daemon fans this out to other peers' app processes (via courier).
- `ephemeral.subscribe { topic }` → `{ topic }`  (subscribe to wake-ups
  carrying payload data; see Event variant)
- `ephemeral.unsubscribe { topic }` → `{}`

**Subscriptions (`scribe:bind / rebind / unbind`):**
- `bind { name, layer, kind }` → `{ topic }`
  - `kind` is one of `"tree" | "text" | "list" | "map"`.
  - Daemon emits `Event { topic, payload: None }` on every subsequent
    canonical change to `layer`.
  - Child translates the event into `ctx.request_repaint()` (egui) or
    the renderer's equivalent.
  - App's next frame calls `tree:walk()` / `list:read()` / etc., reads
    fresh canonical data.
- `rebind { name, layer }` → `{ topic }`
- `unbind { name }` → `{}`

Note: `bind` is purely a subscription to wake-ups — it does NOT return
data. Data flows via RPC, never via events for stored state. Events for
ephemeral state are the only events that carry payload.

**Dev / control:**
- `app.eval { code }` → `{ result }`  (control-socket eval; gated by
  a `dev_eval` feature flag in production builds)

### Handle model: stateless on the wire

Today `scribe:tree("doc")` returns a Lua userdata that caches the layer
reference. Across IPC we keep that Lua handle for the *app author's*
convenience but make it **stateless on the wire**: the handle is a thin
Lua object that holds the layer name as a string. Each method call sends
an RPC with `params: { layer: "doc", ... }`; the daemon resolves the
layer fresh each call.

Why stateless:
- No handle lifecycle on the daemon. Children come and go; nothing to
  clean up. Layer-name lookup is cheap.
- No "handle leaked / dangling reference" bug class.
- Apps see no difference: `scribe:tree("doc"):walk()` works the same.

The alternative — stateful handles allocated by `handle.create` with ids —
buys you a constant-factor lookup speedup that doesn't matter at our
call rates. Revisit only if profiling demands it.

**Daemon → child events. Two flavors:**

*Canonical-state wake-ups* — `payload: None`. Child repaints; app re-
queries via RPC.
- `tree:<layer>`, `text:<layer>`, `list:<layer>`, `map:<layer>`
- `peer.joined`, `peer.left` (presence; app re-queries via RPC)
- `permit.changed` (app re-queries its accessible layers)

*Ephemeral broadcasts* — `payload: Some(value)`. The payload IS the
message; there's nothing to re-query.
- `ephemeral:<topic>` — fanned out from other peers' `ephemeral.send`

### Permit & error semantics

- Every `Request::Call` runs through the same permit check the
  in-process bindings do today. The check sits inside the
  `AppIpcActor::handle_call` path, calling into gurkha/butler — no
  new policy code.
- Errors are typed: `PermitDenied`, `NotFound`, `BadRequest` (schema
  / params), `Internal` (bug / panic). Children map these into Lua
  errors with sensible messages.
- A child that floods the socket gets a per-child rate limit on the
  daemon side (token bucket, dropped messages logged). v1 limit can be
  generous; we just want a knob in place.

### Single source of truth — no replica

- **Daemon owns the only copy of canonical state.** Child holds nothing
  long-lived. There is no mirror, no cache, no replicated table.
- `ScribeIpcBindings` is a stateless RPC client. `tree:walk()` is a
  blocking RPC call that returns a fresh snapshot. Lua receives a fresh
  table each call. When the frame ends, the table is GC'd. Next frame
  asks again.
- **No replica = nothing to invalidate, nothing to drift, no consistency
  bugs.** The cost is per-frame round-trip latency for whatever the
  current frame reads; on a Unix socketpair on localhost that's a 50–
  200 µs hit per RPC, well within the 16 ms frame budget for an app
  that calls `tree:walk()` once at the top of `on_frame`.
- **Apps that call `walk()` once per frame are doing it right.** Apps
  that call it many times per frame should refactor to read once into a
  frame-local local, same as good practice today.
- **For very large documents**, the `tree:walk_range(from, to)` variant
  lets a virtualized scroll area fetch only the visible window. v1 can
  ship without it; add when a real app needs it.
- **Writes (`tree:create`, `text:edit` etc.) are synchronous RPCs.** Send
  request, await reply, return to Lua. If the write succeeded, the next
  frame's `walk()` already reflects it (the daemon applied it before
  acknowledging). If it failed, the RPC returns an error and Lua sees
  it. No optimism, no rollback machinery, no two-phase anything.

### Where the code lives

| Crate | Adds |
|---|---|
| `app_ipc` (new) | Protocol types (Request/Reply/Event/IpcError), framing (length-prefix JSON over `tokio::net::UnixStream`), server-side accept loop, client-side `IpcClient`. Renderer-agnostic. |
| `lua_runtime` | `ScribeIpcBindings`: drop-in replacement for `ScribeBindings`, same Lua surface, uses `app_ipc::IpcClient`. Stateless RPC — no mirror, no cache. |
| `sthalam` | `spawn_app`: create socketpair, fork child with fd, spawn `AppIpcActor` keyed by child PID. `AppIpcActor` lives in the existing ractor system. |
| `renderer_egui` | Runner uses `ScribeIpcBindings` instead of `EguiApp::new_headless`. Reader loop calls `ctx.request_repaint()` on every event. |

### Sequencing

| Phase | Scope | Deliverable | Gate |
|---|---|---|---|
| B0 | Protocol types + framing in `app_ipc` crate, no integration. Unit tests round-trip envelopes. | Compiling crate, types exported. | `cargo test -p app_ipc` |
| B1 | Daemon-side server: socketpair creation in `spawn_app`, `AppIpcActor` skeleton, `page.context` and `app.eval` round-trip. | A child can read its `page_id` from the daemon and the scenario readiness check passes. | `test_drag_list_egui.py` clears readiness |
| B2 | Tree surface end-to-end (walk, create, delete, set_prop, move, text_edit). Pure RPC, no subscriptions yet. | drag-list-egui (rewritten to use a Loro tree) works for a single peer. | Manual smoke |
| B3 | Subscriptions: `bind/rebind/unbind` for trees, wake-up `Event { topic, payload: None }`, child translates to `ctx.request_repaint()`. Plus ephemeral pub/sub (`ephemeral.send/subscribe`) with payload-bearing events — needed for typing indicators and presence. | drag-list-egui edits sync between two peers — alice drags, bob's window repaints, bob's `walk()` returns the new order. | Scenario test with `alice + bob` |
| B4 | `list`, `map`, `text` method surfaces (read/write + bind support). | Enough to port group-chat (map) and text-editor (tree+text) in `renderer_egui.md` phase 5. | Manual smoke against both apps |
| B5 | Layer management: `layer.list`, `layer.create`, `layer.grant`. Hardening: rate limits, error coverage, OS-sandbox hooks (Landlock/seccomp stubs), child-exit detection. | Ready for production-shaped apps including dynamic layer creation (DMs, channels). | Stress test (1000 events/s, kill -9 child mid-call) |

Phase B0 alone is meaningful — it's the "would I be happy living with
this protocol for 6 months?" gate. If the answer is no after writing
the unit tests, redesign before B1.

## Open questions

1. **JSON vs postcard.** JSON for v1 (debuggability). Switch when a
   profile shows serde-json as a hot spot. The protocol types don't
   change.
2. **Where does `AppIpcActor` live in the actor tree?** Probably under a
   supervisor that owns per-child actors, mirroring how page-scoped
   actors are organized today. Sketch this before B1.
3. **Eval security.** `app.eval` is dangerous — it runs arbitrary Lua
   in the child. Today the control socket has the same exposure. v1:
   gate behind a feature flag (`dev_eval`) so production builds reject
   it by default.
4. **Fd inheritance on macOS.** Same primitive, slightly different
   syscalls. Probably no work; verify before claiming cross-platform.
5. **What happens when the child crashes mid-call?** Daemon sees socket
   close, reaps the child, drops pending replies (caller may already be
   gone). The Lua-app side disappears with the process — no cleanup
   needed there. Sthalam UI shows "app closed."
6. **What about read coalescing?** If 5 widgets in one frame each call
   `tree:walk()`, we make 5 RPCs. Good app authors avoid this (read once
   at top of frame), but should the bindings transparently coalesce
   identical reads inside a single `on_frame`? Probably not for v1 —
   adds state, hides cost, and "read once per frame" is a fine
   discipline. Reconsider if real apps make the mistake often.

## Out of scope

- **Remote actors / `ractor_cluster`.** Considered; rejected. Wrong
  trust topology. We keep ractor inside the daemon and put thin RPC
  at the boundary.
- **Shared memory for Loro state.** Considered; rejected. Loro
  internals aren't laid out for concurrent access, you lose crash
  isolation, and "one canonical + one mirror per client via events"
  is the standard solved pattern.
- **Browser-style same-origin policy.** UCAN permits already cover this
  and are richer; no need to invent a parallel system.
- **Wire format flexibility.** Pick JSON, ship, move on. Premature
  optimization to make it pluggable.
- **Slint subprocess conversion.** Same architecture applies eventually,
  but Slint apps work in-process today — no forcing function until we
  want to drop Slint from sthalam's process entirely.

## Related

- `renderer_egui.md` — the immediate consumer; "Event loop & threading"
  section is the decision context for going subprocess in the first
  place.
- `widgets_roadmap.md` — eventually consumes this for drag/canvas apps
  that want to sync.
- `sample_apps/drag-list-egui/` — first app rewritten against this
  for the B2 gate.
- `sample_apps/text-editor/` — phase B4 target; the test of "richest
  surface works end-to-end."
- VS Code's main↔extension-host RPC and Wayland's compositor↔client
  protocol are the closest mainstream architectural analogs.
- Tauri's `invoke` / `emit` API is the closest analog for the message
  shapes; worth pulling their actual code as reference at B0.
