# Code Quality Analysis

Date: 2026-02-12

## Overall Rating

**7.6 / 10**

The repository has strong architecture and good core engineering practices, with notable gaps in consistency (instrumentation/logging), test coverage in some runtime crates, and a few high-impact protocol/security TODOs.

## Scorecard

- **Architecture: 8.5/10** - Clear crate boundaries and layered design are well documented and mostly reflected in implementation.
- **Reliability: 7.0/10** - Good actor model and sync design, but protocol signature TODOs remain.
- **Readability: 7.2/10** - Strong doc comments in core crates; some files are too large and harder to reason about.
- **Testability: 7.4/10** - Strong integration harness; uneven unit/smoke coverage in UI/runtime/control crates.
- **Observability: 7.0/10** - Strong in core sync path; uneven adoption of `#[instrument]` and mixed logging style.
- **Developer Ergonomics: 8.2/10** - Good docs and scenario tooling, with some doc drift.

## Key Strengths

1. Well-defined crate responsibilities and boundaries.
   - Evidence: `docs/ARCHITECTURE.md`, workspace crate layout in `Cargo.toml`.
2. Transport abstraction enables testability and protocol isolation.
   - Evidence: `transport/src/traits.rs`.
3. Strong actor-model implementation in core sync path.
   - Evidence: `scribe/src/actor.rs`, `courier/src/peer_actor/handshake.rs`.
4. Good API-style service boundary in Butler.
   - Evidence: `butler/src/lib.rs` (`spaces()`, `pages()`, `nodes()`, etc.).
5. Mature integration test harness with scenario builder and message tracing.
   - Evidence: `integration_tests/src/scenario.rs`, `docs/INTEGRATION_TESTING.md`.
6. Structured error handling in Butler with category mapping.
   - Evidence: `butler/src/error.rs`.

## Top Issues and Improvements

1. **Critical**: Missing handshake signatures in protocol flow.
   - Evidence: `courier/src/peer_actor/handshake.rs:116`, `courier/src/peer_actor/handshake.rs:409`.
   - Improvement: Implement signing and verification for Hello/Welcome, with timestamp validation and rejection paths.

2. **High**: Inconsistent async instrumentation outside core crates.
   - Evidence: async functions without `#[instrument]` in `transport/src/lib.rs`, `control_server/src/server.rs`, and runtime entry crates.
   - Improvement: Add `#[instrument]` at async boundaries per policy.

3. **High**: Mixed logging approach (`tracing`, `log`, `println!`).
   - Evidence: `butler/src/lib.rs` (`log::info!`), `sthalam_shell/src/callbacks/viewer.rs` (`println!`).
   - Improvement: Standardize to `tracing` macros; reserve `println!` for explicit CLI UX output.

4. **High**: Stringly-typed error returns in Courier APIs.
   - Evidence: `courier/src/handle.rs` methods returning `Result<_, String>`.
   - Improvement: Introduce and propagate a typed `CourierError` enum.

5. **Medium**: Complexity hotspots in large files/functions.
   - Evidence: `courier/src/peer_actor/mod.rs`, `renderer_slint/src/lib.rs`, `lua_runtime/src/runtime.rs`.
   - Improvement: Refactor by flow/state modules and extract long functions.

6. **Medium**: Documentation drift in setup guide.
   - Evidence: stale paths in `docs/SETUP.md` referencing files no longer present.
   - Improvement: update docs and add CI check for path validity.

7. **Medium**: Test coverage gaps in `sthalam`, `kunki`, `control_server`, `renderer_raylib`, `sthalam_shell`.
   - Improvement: add smoke tests for startup, control RPC handling, and critical runtime paths.

8. **Medium**: Open TODOs in user-facing flows.
   - Evidence: `sthalam_shell/src/callbacks/pages.rs:209`, `sthalam_shell/src/callbacks/spaces.rs:148`.
   - Improvement: implement or hide unfinished actions.

9. **Low**: Error category mapping occasionally conflates storage/input issues.
   - Evidence: some app import failures reported as `Database` in `butler/src/services/app_service.rs`.
   - Improvement: tighten mapping to `Io`, `Serialization`, and `Invalid`.

10. **Low**: Large domain/protocol files impact discoverability.
    - Evidence: `domains/src/layer.rs`, `courier/src/message.rs`.
    - Improvement: split into focused modules and add module-level docs.

## Prioritized Roadmap

### Quick Wins (This Week)

- Implement handshake signature verification and rejection paths.
- Standardize logging macros in active runtime paths.
- Add `#[instrument]` to async entrypoints in `transport` and `control_server`.
- Fix stale paths in `docs/SETUP.md`.

### Medium-Term (This Quarter)

- Introduce typed `CourierError` across public APIs.
- Refactor complexity hotspots (`peer_actor`, `renderer_slint`, `lua_runtime`).
- Add smoke/integration tests for `kunki` and control server flows.

### Long-Term Bets

- Add CI policy checks for instrumentation and logging style consistency.
- Add docs drift checks (path/link validation).
- Add architecture conformance checks for crate dependency boundaries.
