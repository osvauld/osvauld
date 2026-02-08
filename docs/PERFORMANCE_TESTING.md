# Performance Testing Guide

This guide covers tools and techniques for profiling and benchmarking osvauld's P2P sync system.

## Quick Start

```bash
# Build release binaries
cargo build --release -p kunki -p sthalam

# Run performance test
python scripts/test_chat_reactive.py --release --messages 100 --output results/baseline.json
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
python scripts/test_chat_reactive.py --release --messages 100
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
python scripts/test_chat_reactive.py --flame-only --messages 50

# .folded files are in each instance's data dir:
# /tmp/chat_reactive_test/node/node.folded
# /tmp/chat_reactive_test/alice/alice.folded
# etc.
```

#### Converting .folded files to SVG

```bash
cargo install inferno

# Single instance
inferno-flamegraph /tmp/chat_reactive_test/node/node.folded > node_flame.svg

# Open in browser
xdg-open node_flame.svg
```

#### Automated analysis script

```bash
# Analyze all .folded files from a test run
python scripts/analyze_flamegraph.py /tmp/chat_reactive_test/

# With SVG generation and JSON export
python scripts/analyze_flamegraph.py /tmp/chat_reactive_test/ --svg -o results/flame_analysis.json

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
python scripts/test_chat_reactive.py --release --heaptrack -m 100

# .zst files are in each instance's data dir:
# /tmp/chat_reactive_test/alice/alice.zst
# /tmp/chat_reactive_test/node/node.zst
```

#### Automated analysis

```bash
# Analyze all instances
python scripts/analyze_heaptrack.py /tmp/chat_reactive_test/

# Save to JSON for comparison
python scripts/analyze_heaptrack.py /tmp/chat_reactive_test/ -o results/heap_v1.json

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
heaptrack_gui /tmp/chat_reactive_test/alice/alice.zst

# Text mode — peak consumers
heaptrack_print -f /tmp/chat_reactive_test/alice/alice.zst -p -n 10

# Filter by subsystem
heaptrack_print -f /tmp/chat_reactive_test/alice/alice.zst --filter-bt-function argon2 -p

# Leak analysis
heaptrack_print -f /tmp/chat_reactive_test/alice/alice.zst -l -n 10
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
python scripts/test_chat_reactive.py --release -m 100 -o results/v1.json

# After changes
python scripts/test_chat_reactive.py --release -m 100 -o results/v2.json

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
   python scripts/test_chat_reactive.py --release -m 100 -o results/slow.json
   ```

2. **Generate flamegraph (where is CPU time going?):**
   ```bash
   python scripts/test_chat_reactive.py --flame-only -m 100
   inferno-flamegraph /tmp/chat_reactive_test/node/node.folded > node.svg
   ```

3. **If you suspect async issues, check with tokio-console:**
   ```bash
   python scripts/test_chat_reactive.py --profile -m 100
   # In another terminal:
   tokio-console http://localhost:6669
   ```

4. **Fix and compare:**
   ```bash
   python scripts/test_chat_reactive.py --release -m 100 -o results/fixed.json
   python scripts/compare_perf.py results/slow.json results/fixed.json
   ```

### Before a Release

1. Run full benchmark suite:
   ```bash
   cargo bench --package integration_tests
   ```

2. Run stress tests:
   ```bash
   python scripts/test_chat_reactive.py --release --messages 500 -o results/release.json
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

## Baseline Findings (Feb 2026, commit 0c50d055)

### CPU Profile (flamegraph, 50 messages, 4 instances)

Application code uses <0.1% of total CPU. The system is **I/O-bound, not CPU-bound**.

**Node application hotspots** (excluding networking idle loops):

| Function | % of app CPU | Notes |
|---|---|---|
| `iroh::router.accept` | 15.9% | QUIC connection acceptance |
| `butler::auth_service::login` | 15.6% | Argon2 key derivation (by design) |
| `iroh::transports::poll_send` | 9.1% | QUIC packet sending |
| `ractor::actor::Actor` | 9.1% | Actor framework overhead |
| `courier::handle_broadcast_received` | 8.8% | Processing sync broadcasts |
| `scribe::actor::handle` | 5.8% | CRDT message processing |
| `courier::send_message` | 4.4% | Sending protocol messages |
| `loro::export` | 3.2% | CRDT state export for sync |

**Scribe actor internals** (consistent across all instances):

| Function | % of scribe CPU | Notes |
|---|---|---|
| `handle_apply_update` | 74-86% | Applying incoming CRDT updates |
| `handle_map_insert` / `handle_list_push` | 3-10% | Local mutations |
| `handle_flush` | 2-7% | Persisting to disk |
| `handle_reconcile_with_peers` | 7% (node only) | Reconciliation |
| `handle_subscribe` | 2-3% | Setting up sync subscriptions |

**handle_flush breakdown** (persistence cost):

| Function | % of flush | Notes |
|---|---|---|
| `save_layer` (redb write) | ~42% | Serializing Loro doc to storage |
| `loro::export` (snapshot) | ~43% | Exporting full CRDT snapshot |
| `save_vectors` | ~14% | Persisting version vectors |

### Resource Profile

| Metric | 500 msgs (release) | Notes |
|---|---|---|
| Throughput | 3.7 msg/sec | Constrained by 0.1s send delay |
| Node memory | ~47 MB | Stable, healthy |
| Shell memory (peak) | ~290 MB | After 500 messages |
| Shell growth rate | ~0.1 MB/msg | Delta-first bindings, O(1) per message |
| Node CPU | ~1% | Idle, healthy |
| Shell CPU | 7-8% | Low |

### Memory Growth: Optimizations Applied

The original shell memory grew **super-linearly** (~8 MB/msg at 100 msgs) due to the binding pipeline cloning the full array on every update (O(N^2)). This has been fixed.

**Optimizations applied (Feb 2026):**

| Optimization | Effect | Result |
|---|---|---|
| Delta-first bindings | `process_bindings()` uses Loro delta for surgical `Insert`/`Remove` ops instead of full `Replace` | ~98% memory reduction per update |
| Remove full_data from hot path | Skip `get_content()` (O(N) JSON serialization) when delta is available | Eliminates O(N) serialization per change |
| Incremental peer broadcast | `export_updates(from_vv)` instead of `export_snapshot()` per subscriber | Node memory growth near zero |
| Qt backend removal | `SLINT_NO_QT=1` in `.cargo/config.toml` | ~50% peak memory reduction |
| `max_items` binding option | Caps UI model size, drops oldest items | Constant memory for growing layers |
| `malloc_trim` after argon2 | Returns 64 MB argon2 working memory to OS | ~64 MB freed after login |

**Current results (500 messages, release build, 3 shells + 1 node):**

| Metric | Before | After | Change |
|---|---|---|---|
| Shell peak memory | ~1000 MB (100 msgs) | ~290 MB (500 msgs) | ~95% reduction |
| Shell growth rate | ~8 MB/msg | ~0.1 MB/msg | ~98% reduction |
| Node growth rate | ~0.33 MB/msg | ~0.01 MB/msg | ~97% reduction |
| Shell CPU (avg) | 12-16% | 7-8% | ~50% reduction |
| Throughput | 2.8 msg/sec | 3.7 msg/sec | +33% |

**Remaining memory sources:**

1. **Loro CRDT document**: Grows with history. The Loro doc retains full operation history for conflict resolution. This is inherent to CRDTs and proportional to data volume.
2. **Slint/femtovg base cost**: ~180 MB fixed overhead for the GL context and font rendering (winit backend).
3. **Argon2 working memory**: 64 MB allocated during login, freed via `malloc_trim(0)` after hashing.

### Known Characteristics

1. **No CPU bottlenecks**. Application code uses <0.1% of total CPU. Argon2 login cost is by design. All other hotspots are proportional to work done.

2. **Memory growth is now O(1) per message** in the binding/UI pipeline. Remaining growth (~0.1 MB/msg) comes from the Loro CRDT document retaining operation history.

3. **Throughput** (3.7 msg/sec with 0.1s send delay) is constrained by test script pacing and relay round-trip latency, not CPU.

### Centralized Test + Analysis

The test script automatically runs flamegraph analysis when `--flame-only` or `--profile` is used:

```bash
# Single command: test + perf report + flamegraph analysis + JSON export
python scripts/test_chat_reactive.py --release --flame-only -m 100 -o results/run.json

# The JSON output includes:
# - throughput, memory, CPU, timeline (PerfReport)
# - memory growth rate per instance
# - flamegraph analysis (hotspots, scribe breakdown, etc.)
```

Compare runs:

```bash
python scripts/compare_perf.py results/before.json results/after.json
# Compares: throughput, memory peak, memory growth rate, CPU, latency
```
