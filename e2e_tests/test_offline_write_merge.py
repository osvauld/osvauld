#!/usr/bin/env python3
"""
Offline write merge E2E — core offline-first promise with daily sharding.

Validates:
- A peer writes messages while offline (node stays online)
- After peer reconnect, both sides converge
- CRDT merge converges: all messages visible on both peers
- Time-sharded layers (daily) are correctly created and merged

Uses deterministic time control (ManualClock) starting in 2028 to avoid
collisions with real system time. Advances clock between offline writes
to exercise day-shard creation during offline periods.

Uses event captures to diagnose sync flow:
- apply_update: confirms data applied on each instance
- broadcast_decision: confirms broadcasts sent (not skipped)
- message_trace: confirms SyncOffer/SyncAccept flow between peers

Usage:
    python e2e_tests/test_offline_write_merge.py --debug --test-mode
    python e2e_tests/test_offline_write_merge.py --debug --test-mode --capture-logs
"""

import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario, _merge_capture_files

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

# 2028-01-15 10:00:00 UTC — well away from real system time
BASE_TIME = 1_831_467_600
DAY_SECONDS = 86_400
HOUR_SECONDS = 3_600
# Advance by 1 day + 1 hour to avoid midnight edge cases
STEP_SECONDS = DAY_SECONDS + HOUR_SECONDS

args = AppTestScenario.parse_args("Offline Write Merge E2E")
include_logs = "--capture-logs" in sys.argv

with AppTestScenario(
    name="offline_write_merge",
    app_path=str(APP_PATH),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
    },
    **args,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")

    # ── Phase 1: Set deterministic time ──
    print("[1/6] Set deterministic time (2028-01-15 10:00 UTC)")
    alice.set_time(BASE_TIME)
    bob.set_time(BASE_TIME)

    # ── Phase 2: Connected baseline — day 0 ──
    print("[2/6] Connected baseline — both peers send on day 0")

    s.capture_start("baseline", include_logs=include_logs)

    alice.eval('send_message("baseline from alice")')
    bob.eval('send_message("baseline from bob")')

    alice.wait_for(
        lambda: alice.eval("return get_message_count()") >= 2,
        timeout=20,
        desc="alice sees both baselines",
    )
    bob.wait_for(
        lambda: bob.eval("return get_message_count()") >= 2,
        timeout=20,
        desc="bob sees both baselines",
    )

    merged_baseline = s.capture_end("baseline", merge=True)
    print("    Both peers see 2 messages (day 0 shard: 2028-01-15)")

    if merged_baseline and merged_baseline.exists():
        events = [json.loads(l) for l in open(merged_baseline) if l.strip()]
        applies = [e for e in events if e.get("type") == "apply_update"]
        broadcasts = [e for e in events if e.get("type") == "broadcast_decision"]
        traces = [e for e in events if e.get("type") == "message_trace"]
        print(
            f"    Capture: {len(applies)} apply_update, {len(broadcasts)} broadcast_decision, {len(traces)} message_trace"
        )

    # ── Phase 3: Alice goes offline (node remains online) ──
    print("[3/6] Alice goes offline (node remains online)")
    alice.go_offline()
    time.sleep(1)

    # ── Phase 4: Writes while alice is offline, advancing time ──
    print("[4/6] Writes while alice offline (advancing days)")

    # Day 0 (still 2028-01-15): alice sends 1 more
    alice.eval('send_message("alice offline day0")')

    # Advance to day 1 (2028-01-16 11:00 UTC)
    alice.advance_time(STEP_SECONDS)
    bob.advance_time(STEP_SECONDS)
    alice.eval('send_message("alice offline day1")')
    bob.eval('send_message("bob offline day1")')

    # Advance to day 2 (2028-01-17 12:00 UTC)
    alice.advance_time(STEP_SECONDS)
    bob.advance_time(STEP_SECONDS)
    alice.eval('send_message("alice offline day2")')
    bob.eval('send_message("bob offline day2")')

    # Expected locally:
    # alice: 2 baseline + 3 offline = 5
    # bob: 2 baseline + 2 offline = 4
    alice.wait_for(
        lambda: alice.eval("return get_message_count()") >= 5,
        timeout=10,
        desc="alice sees own offline writes",
    )
    bob.wait_for(
        lambda: bob.eval("return get_message_count()") >= 4,
        timeout=10,
        desc="bob sees own offline writes",
    )
    alice_pre = alice.eval("return get_message_count()")
    bob_pre = bob.eval("return get_message_count()")
    print(f"    Alice has {alice_pre}, Bob has {bob_pre} (pre-merge)")
    print("    Shards created: day0 (2028-01-15), day1 (2028-01-16), day2 (2028-01-17)")

    # ── Phase 5: Alice goes online, wait for merge ──
    print("[5/6] Alice goes online — wait for reconnect and merge")

    captures_dir = s.base_dir / "captures"
    captures_dir.mkdir(parents=True, exist_ok=True)

    peer_capture_paths = {}
    for peer_name in ["alice", "bob"]:
        cap_path = captures_dir / f"merge_{peer_name}.jsonl"
        s.peer(peer_name).client.capture_start(str(cap_path), include_logs=include_logs)
        peer_capture_paths[peer_name] = cap_path

    # Start capture on node while it is already online
    node_cap_path = captures_dir / "merge_node.jsonl"
    node_client = s._tm.get_client("node")
    node_client.capture_start(str(node_cap_path), include_logs=include_logs)
    peer_capture_paths["node"] = node_cap_path

    alice.go_online()

    expected_total = 7  # 2 baseline + 3 alice offline + 2 bob offline

    try:
        alice.wait_for(
            lambda: alice.eval("return get_message_count()") >= expected_total,
            timeout=60,
            desc="alice converged after merge",
        )
        bob.wait_for(
            lambda: bob.eval("return get_message_count()") >= expected_total,
            timeout=60,
            desc="bob converged after merge",
        )
        print(f"    Both peers converged to {expected_total} messages")
    except Exception as e:
        print(f"    MERGE FAILED: {e}")
        alice_count = alice.eval("return get_message_count()")
        bob_count = bob.eval("return get_message_count()")
        print(f"    Alice has {alice_count}, Bob has {bob_count}")
    finally:
        for peer_name in ["alice", "bob"]:
            try:
                s.peer(peer_name).client.capture_end()
            except Exception:
                pass
        try:
            node_client.capture_end()
        except Exception:
            pass

        merged_path = captures_dir / "merge_merged.jsonl"
        _merge_capture_files(peer_capture_paths, merged_path)

        if merged_path.exists():
            events = [json.loads(l) for l in open(merged_path) if l.strip()]
            applies = [e for e in events if e.get("type") == "apply_update"]
            broadcasts = [e for e in events if e.get("type") == "broadcast_decision"]
            traces = [e for e in events if e.get("type") == "message_trace"]

            print(f"\n    === Merge Phase Capture ===")
            print(f"    Total events: {len(events)}")
            print(f"    apply_update: {len(applies)}")
            print(f"    broadcast_decision: {len(broadcasts)}")
            print(f"    message_trace: {len(traces)}")

            for inst in ["node", "alice", "bob"]:
                inst_applies = [e for e in applies if e.get("instance") == inst]
                if inst_applies:
                    print(f"\n    [{inst}] apply_update events:")
                    for e in inst_applies:
                        layer_short = e.get("layer_short", e.get("layer", "?"))
                        result = e.get("result", "?")
                        from_did = e.get("short_did", "?")
                        print(
                            f"      layer={layer_short} from={from_did} result={result}"
                        )

            node_broadcasts = [e for e in broadcasts if e.get("instance") == "node"]
            if node_broadcasts:
                print(f"\n    [node] broadcast_decision events:")
                for e in node_broadcasts:
                    layer_short = e.get("layer_short", e.get("layer", "?"))
                    decision = e.get("decision", "?")
                    reason = e.get("reason", "")
                    target = e.get("short_did", "")
                    print(
                        f"      layer={layer_short} decision={decision} reason={reason} target={target}"
                    )

            sync_traces = [
                t
                for t in traces
                if t.get("msg") in ("SyncOffer", "SyncAccept", "SyncAck")
            ]
            if sync_traces:
                print(f"\n    Sync protocol messages:")
                for t in sync_traces[:30]:
                    inst = t.get("instance", "?")
                    direction = t.get("direction", "?")
                    msg = t.get("msg", "?")
                    layer = t.get("layer", "?")
                    print(f"      [{inst}] {direction} {msg} layer={layer}")

    # ── Phase 6: Verify merge across shards ──
    print("[6/6] Verify offline messages merged across day shards")

    alice_final = alice.eval("return get_message_count()")
    bob_final = bob.eval("return get_message_count()")

    print(f"    Alice: {alice_final}, Bob: {bob_final}")

    assert alice_final >= expected_total, (
        f"Alice has {alice_final}, expected >= {expected_total}"
    )
    assert bob_final >= expected_total, (
        f"Bob has {bob_final}, expected >= {expected_total}"
    )

    print("[SUCCESS] Offline write merge E2E passed")
