# Performance Testing Guide

This guide covers tools and techniques for profiling and benchmarking osvauld's P2P sync system.

## Quick Start

```bash
# Build release binaries
cargo build --release -p kunki -p sthalam

# Run performance test
python e2e_tests/test_chat_perf.py --release --messages 100 --output results/baseline.json
```

## Build Profiles

| Profile | Command | Use Case |
|---------|---------|----------|
| Debug | `cargo build` | Development (slow, not for perf testing) |
| Release | `cargo build --release` | Performance testing, production |
| Profiling | `cargo build --profile profiling --features profiling` | Flamegraphs + tokio-console |

### Build Commands

```bash
# For performance testing (fast, no symbols)
cargo build --release -p kunki -p sthalam

# For flamegraph profiling (fast + debug symbols + tracing instrumentation)
RUSTFLAGS="--cfg tokio_unstable" cargo build --profile profiling --features profiling -p kunki -p sthalam

# Same thing, flame-only (no tokio-console needed, but same binary)
RUSTFLAGS="--cfg tokio_unstable" cargo build --profile profiling --features profiling -p kunki -p sthalam
```

### Build Gotchas

Profiling builds require **three things** to work correctly:

| Requirement | What it does | What breaks without it |
|-------------|-------------|----------------------|
| `--profile profiling` | Enables debug symbols in optimized build | Flamegraph shows no function names |
| `--features profiling` | Enables `console-subscriber` + `tracing-flame` crates | Binary has no profiling code at all |
| `RUSTFLAGS="--cfg tokio_unstable"` | Enables tokio's internal instrumentation | tokio-console sees no tasks |

`--profile profiling` alone gives you debug symbols but no profiling code.
`--features profiling` alone gives you profiling code but no debug symbols in flamegraphs.
Both together without `RUSTFLAGS` means tokio-console won't see any tasks (flamegraphs still work).

## Tools

### 1. Test Script Performance Report

Built-in timing and resource monitoring:

```bash
python e2e_tests/test_chat_perf.py --release --messages 100
```

**Metrics collected:**
- Timeline of test phases (setup, connect, sync, messages, idle)
- Message throughput (msg/sec)
- Memory usage per instance (peak MB, growth rate MB/msg)
- CPU usage per instance (avg %)
- Idle memory and CPU (10s measurement after messages complete)
- Flamegraph CPU hotspots (when `--flame-only` or `--profile` is used)
- Heaptrack heap analysis (when `--heaptrack` is used)

**Flags:**
- `--release` / `-r`: Use release builds
- `--messages N` / `-m N`: Number of messages (default: 50)
- `--delay D` / `-d D`: Delay between messages in seconds (default: 0.1)
- `--profile`: Enable tokio-console + flame profiling
- `--flame-only`: Enable flame graphs only (no tokio-console overhead)
- `--heaptrack`: Wrap binaries with heaptrack for heap profiling
- `--output FILE` / `-o FILE`: Save results to JSON

**Note**: `--heaptrack` and `--flame-only` are independent and can be used separately. Don't combine `--heaptrack` with `-o` — heaptrack wraps the binary so PID-based memory sampling measures the wrong process.

### 2. tokio-console (Async Task Visualization)

See real-time task states, blocked tasks, channel saturation.

**What it shows:** Tokio runtime internals - task states, poll times, waker counts, channel capacity. This is the tokio runtime's view, NOT your `#[instrument]` spans.

**What it does NOT show:** Your `#[instrument]` spans, function-level CPU time, or call stacks. For those, use flamegraphs.

```bash
# Build with all three requirements
RUSTFLAGS="--cfg tokio_unstable" cargo build --profile profiling --features profiling -p kunki -p sthalam

# Run with console port
TOKIO_CONSOLE_PORT=6669 cargo run --profile profiling --features profiling -p kunki -- start -p test

# In another terminal
cargo install tokio-console
tokio-console http://localhost:6669
```

**Environment variables:**
- `TOKIO_CONSOLE_PORT`: Port for tokio-console (presence enables the console layer)

**What to look for:**
- Tasks stuck in BLOCKED state (deadlock?)
- High poll count with low busy time (spinning?)
- Channel resources near capacity (backpressure)

**Version compatibility:** console-subscriber (library) and tokio-console (CLI) must use the same tonic version. If you see gRPC connection errors, check that both are on compatible versions.

### 3. Flamegraph (CPU Profiling)

Visualize where CPU time is spent across `#[instrument]`-annotated functions.

**What it shows:** Function-level CPU time from tracing spans. Every `#[instrument]` annotation becomes a stack frame in the flamegraph.

**What it does NOT show:** Tokio runtime internals (use tokio-console for that).

#### Flame-only mode (recommended for CPU profiling)

The `--flame-only` flag generates `.folded` files without the overhead of the tokio-console subscriber:

```bash
# Build profiling binaries
RUSTFLAGS="--cfg tokio_unstable" cargo build --profile profiling --features profiling -p kunki -p sthalam

# Run test with flame-only (no console ports opened)
python e2e_tests/test_chat_perf.py --flame-only --messages 50

# .folded files are in each instance's data dir:
# /tmp/chat_perf/node/node.folded
# /tmp/chat_perf/alice/alice.folded
# etc.
```

#### Converting .folded files to SVG

```bash
cargo install inferno

# Single instance
inferno-flamegraph /tmp/chat_perf/node/node.folded > node_flame.svg

# Open in browser
xdg-open node_flame.svg
```

#### Automated analysis script

```bash
# Analyze all .folded files from a test run
python scripts/analyze_flamegraph.py /tmp/chat_perf/

# With SVG generation and JSON export
python scripts/analyze_flamegraph.py /tmp/chat_perf/ --svg -o results/flame_analysis.json

# Compare two flame analyses
python scripts/analyze_flamegraph.py --compare results/flame_before.json results/flame_after.json
```

The script produces:
- **Category breakdown**: CPU time by crate (iroh, scribe, courier, butler, loro, gurkha)
- **Application hotspots**: top functions excluding networking idle loops
- **Scribe actor drill-down**: what the CRDT actor spends time on
- **handle_flush breakdown**: persistence cost (loro export vs redb write vs vector save)
- **Loro CRDT breakdown**: which Loro operations cost the most

#### Interpreting results

- **Wide bars** = more CPU time in that function
- Look for unexpected hotspots: serialization, locking, encryption
- Compare CRDT merge time vs network I/O vs encryption
- The `.folded` format is one stack trace per line with a count - you can grep it directly:
  ```bash
  # Find the hottest functions
  sort -t' ' -k2 -rn node.folded | head -20

  # Find all scribe-related stacks
  grep scribe node.folded | sort -t' ' -k2 -rn | head -10
  ```

#### Using Linux perf (alternative, requires root)

```bash
cargo install flamegraph
./scripts/perf/flamegraph_sync.sh
./scripts/perf/flamegraph_node.sh 30
```

### 4. Heaptrack (Heap Profiling)

Tracks every allocation/deallocation to find peak memory consumers and leaks.

```bash
# Run test with heaptrack (wraps each binary)
python e2e_tests/test_chat_perf.py --release --heaptrack -m 100

# .zst files are in each instance's data dir:
# /tmp/chat_perf/alice/alice.zst
# /tmp/chat_perf/node/node.zst
```

#### Automated analysis

```bash
# Analyze all instances
python scripts/analyze_heaptrack.py /tmp/chat_perf/

# Save to JSON for comparison
python scripts/analyze_heaptrack.py /tmp/chat_perf/ -o results/heap_v1.json

# Compare two runs
python scripts/analyze_heaptrack.py --compare results/heap_v1.json results/heap_v2.json
```

The script reports:
- **Peak heap** per instance (actual heap, not RSS)
- **Leaked memory** per instance (not freed at exit — mostly process-lifetime state)
- **Subsystem breakdown**: peak and leaked by crate (argon2, slint, loro, redb, iroh, etc.)

#### Interactive analysis

```bash
# GUI viewer (if installed)
heaptrack_gui /tmp/chat_perf/alice/alice.zst

# Text mode — peak consumers
heaptrack_print -f /tmp/chat_perf/alice/alice.zst -p -n 10

# Filter by subsystem
heaptrack_print -f /tmp/chat_perf/alice/alice.zst --filter-bt-function argon2 -p

# Leak analysis
heaptrack_print -f /tmp/chat_perf/alice/alice.zst -l -n 10
```

#### Interpreting results

- **Peak heap** is dominated by argon2 (67 MB) — temporary, freed after login
- **"Leaked" memory** is mostly process-lifetime state (Slint GL context, QUIC connections, database cache), not actual leaks
- **Loro** should be small (<1 MB for 100 messages) — if large, delta optimizations may have regressed
- Compare across runs to catch allocation regressions

### 5. Criterion Benchmarks (Regression Testing)

Statistical benchmarks with confidence intervals:

```bash
# Run all benchmarks
cargo bench --package integration_tests

# Specific benchmark
cargo bench --package integration_tests -- handshake

# Compare against baseline
cargo bench -- --save-baseline main
# After changes:
cargo bench -- --baseline main
```

### 6. JSON Results Comparison

Track performance across commits:

```bash
# Save baseline
python e2e_tests/test_chat_perf.py --release -m 100 -o results/v1.json

# After changes
python e2e_tests/test_chat_perf.py --release -m 100 -o results/v2.json

# Compare
python scripts/compare_perf.py results/v1.json results/v2.json
```

**Output shows:**
- Throughput changes with BETTER/WORSE/SAME verdict
- Memory peak changes per process
- Memory growth rate changes (MB/msg)
- Latency changes
- Overall regression detection

## tokio-console vs Flamegraph

| | tokio-console | Flamegraph |
|---|---|---|
| **Shows** | Tokio tasks, channels, resources | Function-level CPU time |
| **Source** | Tokio runtime instrumentation | `#[instrument]` tracing spans |
| **Mode** | Live only (no export) | Post-hoc analysis (.folded -> .svg) |
| **Overhead** | Moderate (console-subscriber polling) | Low (just span entry/exit) |
| **Use when** | Debugging blocked tasks, channel backpressure | Finding CPU hotspots |
| **Needs RUSTFLAGS** | Yes (`--cfg tokio_unstable`) | No (but doesn't hurt) |
| **Flag** | `--profile` | `--flame-only` |

**Rule of thumb:** Start with `--flame-only` for CPU profiling. Only add tokio-console (`--profile`) when you suspect async scheduling issues.

## What to Test

### Sync Latency

| Metric | Description | Target |
|--------|-------------|--------|
| Handshake | Hello -> Welcome round-trip | <200ms (real network) |
| Sync round-trip | SyncOffer -> SyncAck | <50ms (mock), <300ms (real) |
| Message propagation | Send -> All peers receive | <500ms (3 peers) |

### Throughput

| Metric | Description | Target |
|--------|-------------|--------|
| Messages/sec | Sustained message rate | >20 msg/sec |
| Sync bandwidth | Data synced per second | >1 MB/sec |
| Concurrent peers | Max peers before degradation | >10 peers |

### Resource Usage

| Metric | Description | Target |
|--------|-------------|--------|
| Shell memory (idle) | RSS after 500 messages, idle | <200 MB |
| Shell memory growth | Per-message growth rate | <0.1 MB/msg |
| Node memory | RSS after 500 messages | <60 MB |
| Shell CPU idle | CPU when no activity | <10% |
| Node CPU idle | CPU when no activity | <2% |
| CPU under load | CPU during burst sync | <80% |

## Profiling Workflow

### Finding a Performance Issue

1. **Reproduce with metrics:**
   ```bash
   python e2e_tests/test_chat_perf.py --release -m 100 -o results/slow.json
   ```

2. **Generate flamegraph (where is CPU time going?):**
   ```bash
   python e2e_tests/test_chat_perf.py --flame-only -m 100
   inferno-flamegraph /tmp/chat_perf/node/node.folded > node.svg
   ```

3. **If you suspect async issues, check with tokio-console:**
   ```bash
   python e2e_tests/test_chat_perf.py --profile -m 100
   # In another terminal:
   tokio-console http://localhost:6669
   ```

4. **Fix and compare:**
   ```bash
   python e2e_tests/test_chat_perf.py --release -m 100 -o results/fixed.json
   python scripts/compare_perf.py results/slow.json results/fixed.json
   ```

### Before a Release

1. Run full benchmark suite:
   ```bash
   cargo bench --package integration_tests
   ```

2. Run stress tests:
   ```bash
   python e2e_tests/test_chat_perf.py --release --messages 500 -o results/release.json
   ```

3. Compare against previous release:
   ```bash
   python scripts/compare_perf.py results/v0.9.json results/v1.0.json
   ```

## Troubleshooting

### Flamegraph shows no symbols
- Use `--profile profiling` (includes debug symbols)
- Ensure `--features profiling` is also set (enables tracing-flame)
- On macOS, may need `dsymutil` on the binary

### tokio-console shows no tasks
- Ensure **all three** requirements: `--profile profiling`, `--features profiling`, and `RUSTFLAGS="--cfg tokio_unstable"`
- Check firewall isn't blocking the port
- Verify `TOKIO_CONSOLE_PORT` env var is set
- Check version compatibility: console-subscriber and tokio-console must use the same tonic version (e.g., both on tonic 0.12 or both on 0.14)

### tokio-console connection errors (gRPC)
- Usually a tonic version mismatch between console-subscriber (in Cargo.toml) and tokio-console (CLI)
- Check: `cargo install tokio-console` may install a newer version than your console-subscriber expects
- Fix: pin console-subscriber version to match tokio-console, or vice versa

### Benchmarks are inconsistent
- Close other applications
- Use `--warm-up-time 5` for longer warmup
- Run multiple times and check confidence intervals

### compare_perf.py shows unexpected results
- Ensure both runs used the same build profile
- Check message counts match
- Verify network conditions were similar

## Automated Benchmarking

The `benchmark_full.sh` script runs the full pipeline: release build, perf test, heaptrack capture, profiling build, flamegraph capture, analysis, and consolidated JSON report.

```bash
# Run full benchmark (500 messages default)
./scripts/benchmark_full.sh 500

# Compare against baseline
./scripts/benchmark_full.sh 500 results/baseline_cff2502a

# Run and set result as new baseline
./scripts/benchmark_full.sh 500 "" --set-baseline
```

Output: `results/benchmark_<timestamp>/report.json` — single JSON with perf metrics, heaptrack subsystem breakdown, and flamegraph hotspots. Suitable for AI analysis.

Baseline directory: `results/baseline_cff2502a/`

## Baseline Findings (Feb 2026, commit cff2502a)

Post-refactor baseline after scribe actor refactor, permit flow simplification, and lua runtime cleanup.

### Resource Profile (500 messages, release build, 3 shells + 1 node)

| Metric | Value | Notes |
|---|---|---|
| Throughput | 8.7 msg/sec | ~2.4x faster than pre-refactor (3.7) |
| Total duration | ~70s | For 500 messages |
| Node memory (peak RSS) | ~68 MB | Steady growth 55 -> 68 MB over test |
| Shell memory (peak RSS) | 177-206 MB | alice higher (owner), viewers ~177 MB |
| Memory growth (shell) | 0.01-0.02 MB/msg | Healthy, O(1) per message |
| Memory growth (node) | 0.02 MB/msg | Healthy |
| Node CPU (avg) | ~9% | Under sustained load |
| Shell CPU (avg) | 11-12% | Under sustained load |

### Heap Profile (heaptrack, 500 messages)

**Peak heap** is dominated by argon2 (~67 MB) which is temporary working memory for key derivation, freed after login. Actual application heap is ~23 MB per shell and ~0.4 MB for node beyond argon2.

| Instance | Peak Heap | Peak RSS | Leaked | Allocs |
|---|---|---|---|---|
| alice (owner) | 90 MB | 293 MB | 59 MB | 20.6M |
| bob (viewer) | 90 MB | 249 MB | 67 MB | 22.3M |
| carol (viewer) | 90 MB | 245 MB | 58 MB | 20.0M |
| node | 67 MB | 91 MB | 14 MB | 18.3M |

**Shell subsystem breakdown (leaked = process-lifetime allocations):**

| Subsystem | Peak | Leaked | Notes |
|---|---|---|---|
| argon2 | 67 MB | 0 | Temporary, freed after login |
| slint/femtovg/wayland | — | ~20 MB each | GL context, renderer, compositor — overlapping stacks, single ~20 MB allocation |
| glutin | 1.7 MB | 3.2 MB | EGL/OpenGL context |
| iroh/tokio/quinn | — | ~3 MB each | QUIC transport — overlapping stacks, single ~3 MB allocation |
| redb | — | 3.1 MB | Database file + cache |
| loro | — | 1.0-1.2 MB | CRDT document history for 500 messages |
| ractor | — | 0.8-1.1 MB | Actor framework state |
| mlua | — | 0.3-0.4 MB | Lua VM state |
| fontique | 0.3 MB | 0.3 MB | Font discovery |
| serde_json | — | 0.01-0.3 MB | JSON serialization buffers |

**Node subsystem breakdown:**

| Subsystem | Peak | Leaked | Notes |
|---|---|---|---|
| argon2 | 67 MB | 0 | Temporary, freed after login |
| iroh/tokio/quinn | — | ~3 MB | QUIC transport (overlapping stacks) |
| loro | — | 1.0 MB | CRDT document history |
| ractor | — | 0.9 MB | Actor state (more actors than shells: 3 peer actors) |
| mlua | — | 0.2 MB | Lua VM (node apps) |
| redb | — | 0.2 MB | Smaller than shells (no UI layers) |

**Key observations:**

- **RSS vs heap gap**: Shells show ~90 MB heap but ~250-293 MB RSS. The ~160-200 MB gap is GPU-mapped memory (Slint/femtovg GL textures, framebuffers) which isn't tracked by the allocator.
- **argon2 dominates peak heap**: 67 MB temporary allocation during signup/login, properly freed (0 leaked). The `malloc_trim` optimization is working.
- **Loro is efficient**: Only 1.0-1.2 MB leaked for 500 messages of CRDT history. Delta-first bindings are working.
- **"Leaked" is mostly process-lifetime state**: The ~58-67 MB "leaked" per shell is dominated by the GL context (~20 MB), QUIC connections (~3 MB), database (~3 MB), and the rest is long-lived application state that lives until process exit.
- **Node is lean**: 14 MB total leaked, mostly QUIC transport + Loro + ractor actor state.

### CPU Profile (flamegraph, 500 messages)

Application code (non-idle) represents a small fraction of total CPU time. Most time is spent in iroh networking idle loops (expected).

**Category breakdown (% of total CPU including idle):**

| Category | Alice (owner) | Bob (viewer) | Carol (viewer) | Node |
|---|---|---|---|---|
| iroh | 82.6% | 71.4% | 72.4% | 85.7% |
| portmapper | 8.7% | 15.2% | 14.2% | 4.4% |
| ractor | 6.6% | 12.0% | 12.1% | 9.4% |
| courier | 2.7% | 9.4% | 9.8% | 8.9% |
| scribe | 1.5% | 8.9% | 9.2% | 6.5% |
| butler | 2.1% | 2.0% | 2.0% | 3.5% |
| loro | 1.2% | 0.9% | 0.9% | 1.2% |
| gurkha | 0.0% | 0.3% | 0.7% | 2.0% |

**Node application hotspots (excluding idle loops):**

| Function | % of app CPU | Notes |
|---|---|---|
| `gurkha::service::issue_layer_permit` | 15.4% | Permit issuance for each subscriber layer |
| `iroh::protocol::router.accept` | 13.6% | QUIC connection acceptance |
| `iroh::socket::transports::poll_send` | 7.4% | QUIC packet sending |
| `courier::peer_actor::subscribe::on_layer_subscribe` | 7.2% | Processing layer subscription requests |
| `scribe::sync::apply::get_peer_role` | 6.8% | Role lookup during update application |
| `loro_internal::loro::export` | 4.7% | CRDT state export for sync/persistence |
| `butler::auth_service::login` | 4.6% | Argon2 key derivation (by design) |
| `courier::handle_broadcast_received` | 4.3% | Processing sync broadcasts from scribe |
| `courier::handle_protocol_message` | 3.6% | Protocol message dispatch |
| `courier::on_sync_offer` | 3.5% | Sync offer processing |

**Viewer hotspots — notable difference from owner/node:**

Viewers (bob, carol) show a distinctive pattern where `handle_layer_permit_ack` dominates at 35-37% of app CPU, and `on_layer_subscribe_ack` takes 5-6%. This is the initial subscription flow where viewers receive permits and set up layer sync for all layers. This is a one-time cost during connection, not per-message.

**handle_flush breakdown (persistence cost, consistent across instances):**

| Function | % of flush | Notes |
|---|---|---|
| `loro::export` (snapshot) | 80-87% | Exporting full CRDT snapshot for persistence |
| `save_layer` (redb write) | 10-18% | Writing serialized doc to database |
| `save_vectors` | 2-3% | Persisting version vectors |

Flush is dominated by Loro snapshot export. This is expected — the snapshot includes full operation history for conflict resolution.

**Loro CRDT breakdown (% of non-idle CPU):**

| Function | Alice | Bob/Carol | Node | Notes |
|---|---|---|---|---|
| `loro::export` | 2.8% | 2.6-2.8% | 4.7% | Snapshot for persistence + sync |
| `diff_calc::CalcDiff` | 3.0% | 1.1% | 1.7% | Computing diffs for delta bindings |
| `loro::commit_internal` | 2.3% | 0.9% | 1.2% | Committing local mutations |
| `update_oplog_and_apply_delta` | 1.4% | 0.9% | 0.9% | Applying remote updates |
| `loro::_import_with` | 0.6% | 0.4% | 0.4% | Importing remote state |

Node spends more on `loro::export` (4.7%) because it broadcasts updates to 3 peers.

### Notable Findings

1. **ractor overhead is significant**: `ractor::actor::Actor` consumes 37% of alice's non-idle CPU and 15-17% for viewers. This is the actor framework's internal message dispatch, not application logic. Worth monitoring — if this grows, consider batching actor messages or reducing call frequency.

2. **Viewer subscription flow is expensive**: `handle_layer_permit_ack` at 35-37% for viewers is the initial layer subscription handshake. This is a one-time cost per connection but dominates the flamegraph because the test includes connection setup in the measurement window.

3. **`get_peer_role` on node (6.8%)**: This function is called on every update application to determine the sender's role. If it involves a database lookup each time, caching the role per peer session could reduce this.

4. **`gurkha::issue_layer_permit` on node (15.4%)**: Permit issuance is the top node hotspot. This runs during the subscription flow when viewers connect and request layer access. Permit computation involves crypto operations.

5. **Loro export dominates flush (80-87%)**: Persistence cost is almost entirely Loro snapshot serialization. If flush frequency is high, consider incremental export or reduced flush cadence.

6. **Memory is well-controlled**: Growth rate of 0.01-0.02 MB/msg is excellent. The delta-first binding optimizations are holding. No evidence of memory leaks beyond process-lifetime state.

7. **RSS vs heap gap (~160-200 MB per shell)**: This is GPU-mapped memory from the Slint/femtovg renderer. Not actionable without changing the rendering backend.

### Optimizations Previously Applied

| Optimization | Effect |
|---|---|
| Delta-first bindings | `process_bindings()` uses Loro delta for surgical `Insert`/`Remove` ops instead of full `Replace` |
| Remove full_data from hot path | Skip `get_content()` (O(N) JSON serialization) when delta is available |
| Incremental peer broadcast | `export_updates(from_vv)` instead of `export_snapshot()` per subscriber |
| Qt backend removal | `SLINT_NO_QT=1` in `.cargo/config.toml` |
| `max_items` binding option | Caps UI model size, drops oldest items |
| `malloc_trim` after argon2 | Returns 67 MB argon2 working memory to OS |

### Remaining Memory Sources

1. **GPU-mapped memory**: ~160-200 MB per shell from Slint/femtovg GL context, textures, framebuffers. Not heap-tracked. Inherent to the rendering backend.
2. **Loro CRDT document**: ~1 MB per 500 messages. Grows with history (operation log for conflict resolution). Inherent to CRDTs.
3. **QUIC transport**: ~3 MB per instance for iroh/quinn connection state. Fixed cost per peer connection.

### Running the Benchmark

```bash
# Full automated benchmark (builds, tests, analyzes, generates report.json)
./scripts/benchmark_full.sh 500

# Compare with baseline
./scripts/benchmark_full.sh 500 results/baseline_cff2502a

# Set new baseline
./scripts/benchmark_full.sh 500 "" --set-baseline

# Manual: individual test types
python e2e_tests/test_chat_perf.py --release -m 500 -o results/perf.json
python e2e_tests/test_chat_perf.py --release --heaptrack -m 500
python e2e_tests/test_chat_perf.py --flame-only -m 500

# Manual: analysis
python scripts/analyze_heaptrack.py /tmp/chat_perf/ -o results/heap.json
python scripts/analyze_flamegraph.py /tmp/chat_perf/ -o results/flame.json --svg
python scripts/compare_perf.py results/before.json results/after.json

# Interactive investigation
heaptrack_gui /tmp/chat_perf/alice/alice.heaptrack.zst
xdg-open results/alice_flame.svg
```
