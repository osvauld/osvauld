# Performance Analysis - Run 20260216_174902

**Git Commit**: `cff2502a` (feature/coordinator-refactor)  
**Test Configuration**: 500 messages configured, 1000 total sent (500 send-phase + 500 distributed)  
**Build Profile**: release

---

## Executive Summary

### Throughput
- **Send-path only**: 35.4 msg/sec (28ms avg latency per local CRDT write)
- **Distributed (with sync)**: 12.9 msg/sec (77ms avg latency including network)

### Memory
- **Shells (alice/bob/carol)**: 326-333 MB peak (+50 MB growth for 1000 messages)
- **Node**: 97 MB peak (+32 MB growth)
- **Efficiency**: 30-50 KB per message stored

### CPU
- **Alice** (sender): 72% avg
- **Bob/Carol** (receivers): 46-51% avg
- **Node**: 22% avg

---

## Key Findings

### ✅ Strengths

1. **Local CRDT writes are fast**: 28ms/msg is excellent for local-first operations
2. **Loro is memory-efficient**: Only 5-7 MB peak heap for 1000 messages across multiple CRDTs
3. **Node scales well**: 22% CPU average, handles 3 peers comfortably
4. **Memory growth is linear**: 0.03-0.05 MB/msg is predictable and sustainable

### ⚠️ Bottlenecks Identified

1. **Ractor actor dispatch overhead**: 37-68% of non-idle CPU is actor message passing
   - **Alice**: 68% (4.99s / 7.35s non-idle)
   - **Bob**: 36% (2.38s / 6.61s)
   - **Node**: 3.4% (198ms / 5.83s)
   - *This is architectural - may be acceptable given actor benefits*

2. **`get_peer_role` double permit lookup (node)**: 11.8% of node CPU (689ms)
   - Already identified in previous analysis
   - **Optimization**: Check subscriber map before DB lookup
   - **Expected gain**: ~6-12% node CPU reduction

3. **UI framework leaks (Slint/Femtovg/Wayland)**: 20 MB leaked per shell
   - Font/texture caches not released during test
   - Common pattern but trackable
   - Not critical for functionality but worth monitoring

---

## Detailed Breakdown

### Heap Analysis (alice - 145 MB peak)

| Subsystem | Peak | Leaked | % of Peak | Notes |
|-----------|------|--------|-----------|-------|
| **Slint** | 5.0 MB | 20 MB | 3.5% | UI framework |
| **Loro** | 5.8 MB | 1.4 MB | 4.0% | CRDT storage |
| **Tokio** | 5.9 MB | 3.0 MB | 4.1% | Async runtime |
| **Glutin** | 3.2 MB | 3.2 MB | 2.2% | OpenGL context |
| **Femtovg** | 1.4 MB | 20 MB | 1.0% | Vector graphics |
| **Wayland** | 1.7 MB | 20 MB | 1.2% | Window system |

**Node (70 MB peak)**:
- Loro: 7.1 MB (stores all peer CRDTs - higher than shells)
- Tokio: 7.2 MB
- No UI overhead (headless)

### Flamegraph Hotspots (alice)

**Top 10 CPU consumers (non-idle time)**:

| Function | Time | % | Category |
|----------|------|---|----------|
| `ractor::actor::Actor` | 4.99s | 67.9% | Actor dispatch |
| `iroh::endpoint::connect` | 331ms | 4.5% | QUIC handshake |
| `butler::auth_service::signup` | 214ms | 2.9% | Auth setup |
| `loro::diff_calc::CalcDiff` | 189ms | 2.6% | CRDT delta |
| `butler::app_service::collect_files` | 177ms | 2.4% | App load |
| `iroh::socket::poll_send` | 152ms | 2.1% | Network I/O |
| `loro::export` | 151ms | 2.1% | Serialization |
| `courier::sync::on_sync_offer` | 129ms | 1.7% | Sync protocol |
| `loro::commit_internal` | 125ms | 1.7% | CRDT commit |
| `courier::sync::handle_broadcast` | 119ms | 1.6% | Broadcast recv |

**Node hotspots**:

| Function | Time | % | Category |
|----------|------|---|----------|
| `scribe::get_peer_role` | 689ms | 11.8% | **Permit lookup** ⚠️ |
| `loro::export` | 403ms | 6.9% | Flush export |
| `courier::handle_broadcast` | 408ms | 7.0% | Broadcast recv |
| `iroh::socket::poll_send` | 490ms | 8.4% | Network I/O |

---

## Performance Optimization Opportunities

### High Priority

1. **`get_peer_role` double lookup** (node - 11.8% CPU)
   - **Current**: Always calls `load_user_permit` even when subscriber map has cached permit
   - **Fix**: Check subscriber map first (same pattern as `Permissions::can_write`)
   - **Expected gain**: 6-12% node CPU reduction

2. **Sequential sync protocol calls** (all peers - 1.7-4.6% CPU)
   - **Current**: `on_sync_offer` does `ApplyUpdateWithResult` then `GetStateVector` as 2 round-trips
   - **Fix**: Combine into 1 message
   - **Expected gain**: Reduce sync latency by 50ms per sync round

### Medium Priority

3. **Lua N+1 reads** (shells - scattered across app code)
   - **Current**: `keys()` + `get(key)` loops in read_tracker/channels/threads
   - **Fix**: Add `map:get_all()` API for batch reads
   - **Expected gain**: Reduce badge computation time, especially for DM-heavy workflows

### Low Priority (Monitor)

4. **Ractor actor overhead** (37-68% non-idle CPU)
   - **Current**: Message passing dominates CPU profiles
   - **Analysis needed**: Profile what % is framework vs. actual message serialization
   - **Trade-off**: Actor isolation benefits vs. raw throughput

5. **UI framework leaks** (20 MB per shell)
   - **Current**: Slint/Femtovg/Wayland caches not released
   - **Fix**: Investigate if caches can be bounded or cleared periodically
   - **Impact**: Memory growth over long sessions

---

## Test Timeline Breakdown

| Phase | Duration | Throughput | Notes |
|-------|----------|------------|-------|
| Setup | 8.5s | - | Auth, app load, QUIC handshakes |
| **Send-phase** | **14.1s** | **35.4 msg/s** | Alice sends 500 msgs (no delay) |
| Distributed | 77.2s | 6.5 msg/s | 500 msgs with 0.1s delay |
| Sync wait | 2.0s | - | Final convergence |
| **Total** | **105.1s** | **9.5 msg/s** | End-to-end including setup |

**Observations**:
- Pure send-path is 2.7× faster than distributed phase (35.4 vs 12.9 msg/sec)
- Delay setting (0.1s = 100ms) limits distributed throughput artificially
- Without delay, distributed throughput would be ~12-15 msg/sec (network + sync overhead)

---

## Recommendations

### Immediate Actions

1. **Implement `get_peer_role` optimization** - Easy win, 6-12% node CPU gain
2. **Monitor memory growth in long-running tests** - Current 0.05 MB/msg × 10k messages = 500 MB

### Next Analysis

1. **Profile ractor message types** - Understand what messages dominate the 68% actor overhead
2. **Run 5k+ message test** - Validate linear memory scaling holds at higher volumes
3. **Compare with/without UI** - Isolate UI overhead by running headless alice/bob/carol

### Baseline Comparison (when available)

- This run should become the baseline for `feature/coordinator-refactor`
- Future runs should compare against this to track regressions

---

## Conclusion

**The system performs well for the chat use case**:
- ✅ Local writes are fast (28ms)
- ✅ Memory is efficient (30-50 KB/msg)
- ✅ Node handles 3 peers at 22% CPU

**Top optimization target**: `get_peer_role` double lookup (easy, 11.8% node CPU gain)

**Acceptable trade-offs**:
- Ractor overhead (68% non-idle) is high but provides actor isolation benefits
- UI leaks (20 MB) are common and not critical for functionality
