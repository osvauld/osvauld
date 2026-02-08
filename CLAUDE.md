# Osvauld Code Policies

This file is auto-read by Claude Code.

**Note**: Logs and comments exist for Claude and the author to understand complex sync flows during debugging. This is an experimental approach - prioritize visibility and clarity.

## Logging Policy

| Level | Use Case | Production |
|-------|----------|------------|
| `trace!` | Wire-level: bytes, serialization | Filtered out |
| `debug!` | Implementation details: cache hits | Filtered out |
| `info!` | Business events: connections, sync | Visible |
| `warn!` | Recoverable issues: retry, fallback | Visible |
| `error!` | Failures requiring attention | Visible |

- Use `#[instrument]` on ALL async functions (don't skip params)
- Transport logs wire-level at trace, Courier logs business events at info
- Use `{:?}` for message logging via Debug derive

## Comment Policy

Use domain terminology (their/our, peer, permit):

```rust
/// Handles first connection from a new owner.
///
/// **Context**: When we receive Hello from someone claiming to be owner
/// **Peer sends**: Hello with first_connection permit
/// **We verify**: Permit validity, signature
/// **We store**: OwnerInfo
/// **We issue**: permit_for_owner
/// **We send**: Welcome with permit_for_owner
```

- Inline comments only for non-obvious logic
- Never comment obvious code or leave commented-out code

## Code Structure

- Separate functions for different flows (first_connection vs reconnection)
- Max 2 levels of nesting, then extract to function
- Match arms >3 lines should be functions
- Handler naming: `on_*` for incoming messages, `handle_*` for processing

## Crate Boundaries

| Crate | Purpose | Dependencies |
|-------|---------|--------------|
| `transport` | QUIC connections, message framing | iroh |
| `courier` | P2P orchestration, handshakes | transport, butler, gurkha |
| `butler` | Storage, services API | herald, gurkha |
| `gurkha` | Permit parse + validate only | herald (for crypto) |
| `herald` | Identity, encryption, signing | - |
| `scribe` | CRDT document actor (Loro) | butler |
| `lua_runtime` | Lua VM, app bindings | scribe, butler |
| `domains` | Shared domain types | - |
| `sthalam` | Desktop app browser library | lua_runtime, renderer_slint, renderer_raylib |
| `sthalam_shell` | Desktop app entry point | sthalam |
| `renderer_slint` | Slint UI renderer | lua_runtime |
| `renderer_raylib` | Raylib graphics renderer | lua_runtime |
| `kunki` | Always-on node runtime | butler, courier |
| `control_server` | Unix socket JSON-RPC API | butler, courier |

- Context doesn't leak boundaries
- Butler has services-only API (no direct store access)
- Courier has channel-based communication with apps
- Each crate wraps its dependency errors

## Rust Style

- Idiomatic: `?` operator, combinators for transforms
- Import at top, no full paths in code
- `tokio::spawn` for async event loops
- `&self` with `Arc` internals for services

## Documentation

| Topic | Location |
|-------|----------|
| **App development** | `docs/app-dev/` -- Lua apps, renderers, manifests, permits, validation, derivation, testing |
| **Architecture** | `docs/ARCHITECTURE.md` -- System crates, boundaries, data flow |
| **Protocol** | `docs/PROTOCOL.md` -- P2P wire protocol |
| **Data model** | `docs/DATA_MODEL.md` -- Space/Page/Layer hierarchy |
| **Setup** | `docs/SETUP.md` -- Build, IDE, running |

## Review Checklist

Before submitting:
- [ ] `#[instrument]` on async functions
- [ ] Doc comments on public functions with Context/Peer sends/We verify/etc
- [ ] No deep nesting (max 2 levels)
- [ ] Long match arms extracted to functions
- [ ] Appropriate log levels (trace for wire, info for business)
- [ ] Crate boundaries respected
- [ ] No commented-out code
