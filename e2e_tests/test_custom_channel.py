#!/usr/bin/env python3
"""
Dynamic Channel E2E Test.

Tests the full flow:
  1. Alice (owner) creates a custom channel → dynamic layer
  2. Node receives it via sync
  3. Node fans out sync_meta to bob & carol
  4. Bob & carol discover the channel
  5. Alice sends a message, it syncs to all

Usage:
    python e2e_tests/test_custom_channel.py --keep
    python e2e_tests/test_custom_channel.py --keep --capture-logs
"""

import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

args = AppTestScenario.parse_args("Dynamic Channel E2E")

with AppTestScenario(
    name="dynamic_channel",
    app_path=str(APP_PATH),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
        "carol": {"role": "viewer", "app": "Group Chat"},
    },
    **args,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")
    carol = s.peer("carol")

    include_logs = "--capture-logs" in sys.argv

    # Start capture
    print("\n=== Dynamic Channel E2E ===")
    captures_dir = s.capture_start("dyn_channel", include_logs=include_logs)
    print(f"  Captures: {captures_dir}/")

    # Step 1: Alice creates a custom channel
    print("\n[1] Alice creates channel 'project-x'...")
    alice.eval('create_channel("project-x")')

    # Step 2: Wait for sync to node + fanout
    print("[2] Waiting 4s for sync to node + fanout...")
    time.sleep(4)

    # Step 3: Alice sends a message in the channel
    print("[3] Alice sends message in 'project-x'...")
    alice.eval('send_message("hello from alice in project-x")')

    # Step 4: Wait for message sync
    print("[4] Waiting 3s for message sync...")
    time.sleep(3)

    # Step 5: Check message counts
    alice_count = alice.eval("return get_message_count()")
    bob_count = bob.eval("return get_message_count()")
    carol_count = carol.eval("return get_message_count()")

    print(f"\n  Message counts in project-x:")
    print(f"    Alice (owner):  {alice_count}")
    print(f"    Bob (viewer):   {bob_count}")
    print(f"    Carol (viewer): {carol_count}")

    # Stop capture
    merged = s.capture_end("dyn_channel", merge=True)

    # Analyze captured events
    print(f"\n=== Capture Analysis ===")
    if not merged or not merged.exists():
        print("  No capture data!")
        sys.exit(1)

    events = [json.loads(line) for line in open(merged)]
    print(f"  Total events: {len(events)}")

    # Group by instance
    by_instance = {}
    for e in events:
        inst = e.get("instance", "?")
        by_instance.setdefault(inst, []).append(e)
    for inst, evts in sorted(by_instance.items()):
        print(f"    {inst}: {len(evts)} events")

    # Check key milestones
    print(f"\n--- Key events ---")

    # 1. Layer creation on alice
    creates = [
        e
        for e in events
        if e.get("type") == "layer_created" and "project-x" in e.get("layer", "")
    ]
    print(f"  layer_created (project-x): {len(creates)}")
    for e in creates:
        print(f"    [{e.get('instance')}] {e.get('layer')}")

    # 2. NewDynamicLayer on node
    new_dyn = [
        e
        for e in events
        if e.get("type") == "NewDynamicLayer"
        and "project-x" in str(e.get("layer", e.get("data", "")))
    ]
    print(f"  NewDynamicLayer (project-x): {len(new_dyn)}")
    for e in new_dyn:
        print(f"    [{e.get('instance')}] {e.get('layer', e.get('data', ''))}")

    # 3. sync_meta writes
    sync_meta = [e for e in events if "__sync_meta" in e.get("layer", "")]
    print(f"  __sync_meta events: {len(sync_meta)}")
    for e in sync_meta:
        print(f"    [{e.get('instance')}] {e.get('type')}: {e.get('layer')}")

    # 4. Broadcast decisions
    broadcasts = [e for e in events if e.get("type") == "broadcast_decision"]
    print(f"  broadcast_decision events: {len(broadcasts)}")
    skipped = [e for e in broadcasts if e.get("decision") == "skip"]
    sent = [e for e in broadcasts if e.get("decision") == "send"]
    print(f"    sent: {len(sent)}, skipped: {len(skipped)}")
    if skipped:
        # Group skipped by layer + reason
        skip_summary = {}
        for e in skipped:
            layer = e.get("layer", "?")
            reason = e.get("reason", "?")
            key = f"{layer} | {reason}"
            skip_summary[key] = skip_summary.get(key, 0) + 1
        print(f"    Skipped breakdown:")
        for key, count in sorted(skip_summary.items()):
            print(f"      {key}: {count}")

    # 5. LayerAccessChanged
    access = [e for e in events if e.get("type") == "LayerAccessChanged"]
    print(f"  LayerAccessChanged: {len(access)}")
    for e in access:
        print(f"    [{e.get('instance')}] {e.get('layer')} -> {e.get('data', '')}")

    # 6. layer_discovered on viewers
    discovered = [e for e in events if e.get("type") == "layer_discovered"]
    print(f"  layer_discovered: {len(discovered)}")
    for e in discovered:
        print(f"    [{e.get('instance')}] {e.get('layer')}")

    # 7. Layer subscribe results (Scribe HandleLayerSubscribe outcome)
    sub_results = [e for e in events if e.get("type") == "layer_subscribe_result"]
    print(f"  layer_subscribe_result: {len(sub_results)}")
    for e in sub_results:
        print(
            f"    [{e.get('instance')}] layer={e.get('layer_short')} peer={e.get('short_did')} "
            f"result={e.get('result')} subscriber_added={e.get('subscriber_added')} "
            f"error={e.get('error')}"
        )

    # 8. Permit issue decisions (which path was tried and result)
    permit_decisions = [e for e in events if e.get("type") == "permit_issue_decision"]
    print(f"  permit_issue_decision: {len(permit_decisions)}")
    for e in permit_decisions:
        print(
            f"    [{e.get('instance')}] layer={e.get('layer_short')} peer={e.get('short_did')} "
            f"path={e.get('path')} result={e.get('result')} reason={e.get('reason')}"
        )

    # 9. Layer subscribe protocol (courier-level accept/reject)
    sub_protocol = [e for e in events if e.get("type") == "layer_subscribe_protocol"]
    print(f"  layer_subscribe_protocol: {len(sub_protocol)}")
    for e in sub_protocol:
        print(
            f"    [{e.get('instance')}] direction={e.get('direction')} layer={e.get('layer')} "
            f"peer={e.get('peer_did', '')[:12]} result={e.get('result')} error={e.get('error')}"
        )

    # Result
    print(f"\n{'=' * 60}")
    if alice_count >= 1 and bob_count >= 1 and carol_count >= 1:
        print("  PASS: Dynamic channel synced to all peers")
    else:
        print(f"  FAIL: alice={alice_count} bob={bob_count} carol={carol_count}")
        print(f"  Captures: {merged}")
    print(f"{'=' * 60}")

    # ================================================================
    # Phase 2: Viewer writes to dynamic channel
    # ================================================================
    print("\n\n=== Viewer Write to Dynamic Channel ===")

    # Start a fresh capture for viewer write
    captures_dir2 = s.capture_start("viewer_write", include_logs=include_logs)
    print(f"  Captures: {captures_dir2}/")

    # Bob switches to 'project-x' and sends a message
    print("\n[1] Bob switches to 'project-x' channel...")
    bob.eval('switch_channel("project-x")')
    time.sleep(1)

    bob_active = bob.eval("return get_active_channel()")
    bob_msg_count_before = bob.eval("return get_message_count()")
    print(f"    Bob active channel: {bob_active}")
    print(f"    Bob message count before write: {bob_msg_count_before}")

    print("[2] Bob sends message in 'project-x'...")
    bob.eval('send_message("hello from bob in project-x")')

    # Wait for bob's message to sync
    print("[3] Waiting 5s for bob's message to sync...")
    time.sleep(5)

    # Check message counts after bob's message
    alice_count_2 = alice.eval("return get_message_count()")
    bob_count_2 = bob.eval("return get_message_count()")
    carol_count_2 = carol.eval("return get_message_count()")

    print(f"\n  Message counts after bob's message:")
    print(f"    Alice (owner):  {alice_count_2}")
    print(f"    Bob (viewer):   {bob_count_2}")
    print(f"    Carol (viewer): {carol_count_2}")

    # Stop capture
    merged2 = s.capture_end("viewer_write", merge=True)

    # Analyze viewer write capture
    print(f"\n=== Viewer Write Capture Analysis ===")
    if not merged2 or not merged2.exists():
        print("  No capture data!")
    else:
        events2 = [json.loads(line) for line in open(merged2)]
        print(f"  Total events: {len(events2)}")

        # Group by instance
        by_instance2 = {}
        for e in events2:
            inst = e.get("instance", "?")
            by_instance2.setdefault(inst, []).append(e)
        for inst, evts in sorted(by_instance2.items()):
            print(f"    {inst}: {len(evts)} events")

        # Broadcast decisions (key for viewer→node sync)
        print(f"\n--- Broadcast decisions ---")
        broadcasts2 = [e for e in events2 if e.get("type") == "broadcast_decision"]
        print(f"  Total: {len(broadcasts2)}")
        for e in broadcasts2:
            print(
                f"    [{e.get('instance')}] layer={e.get('layer')} → {e.get('decision')} {e.get('reason', '')}"
            )

        # SyncOffer / SyncAck events
        print(f"\n--- Sync protocol ---")
        sync_evts = [
            e
            for e in events2
            if e.get("type")
            in (
                "sync_offer_sent",
                "sync_offer_received",
                "sync_accept_sent",
                "sync_accept_received",
                "sync_ack_sent",
                "sync_ack_received",
                "SyncOffer",
                "SyncAccept",
                "SyncAck",
            )
        ]
        print(f"  Sync events: {len(sync_evts)}")
        for e in sync_evts:
            print(
                f"    [{e.get('instance')}] {e.get('type')}: {e.get('layer', e.get('data', ''))}"
            )

        # All events from bob (to see what bob's scribe does)
        print(f"\n--- All bob events ---")
        bob_evts = by_instance2.get("bob", [])
        for e in bob_evts:
            print(
                f"    {e.get('type')}: layer={e.get('layer', '')} {e.get('data', '')}"
            )

        # All events from node related to project-x
        print(f"\n--- Node events with project-x ---")
        node_evts = by_instance2.get("node", [])
        px_evts = [e for e in node_evts if "project-x" in str(e)]
        for e in px_evts:
            print(
                f"    {e.get('type')}: layer={e.get('layer', '')} {e.get('data', '')}"
            )

        # __sync_meta events
        print(f"\n--- __sync_meta events ---")
        sm_evts = [e for e in events2 if "__sync_meta" in str(e.get("layer", ""))]
        for e in sm_evts:
            print(f"    [{e.get('instance')}] {e.get('type')}: {e.get('layer')}")

        # Layer subscribe results
        print(f"\n--- Layer subscribe results ---")
        sub_results2 = [e for e in events2 if e.get("type") == "layer_subscribe_result"]
        for e in sub_results2:
            print(
                f"    [{e.get('instance')}] layer={e.get('layer_short')} peer={e.get('short_did')} "
                f"result={e.get('result')} error={e.get('error')}"
            )

        # Permit issue decisions
        print(f"\n--- Permit issue decisions ---")
        permit_dec2 = [e for e in events2 if e.get("type") == "permit_issue_decision"]
        for e in permit_dec2:
            print(
                f"    [{e.get('instance')}] layer={e.get('layer_short')} peer={e.get('short_did')} "
                f"path={e.get('path')} result={e.get('result')} reason={e.get('reason')}"
            )

        # Layer subscribe protocol
        print(f"\n--- Layer subscribe protocol ---")
        sub_proto2 = [e for e in events2 if e.get("type") == "layer_subscribe_protocol"]
        for e in sub_proto2:
            print(
                f"    [{e.get('instance')}] direction={e.get('direction')} layer={e.get('layer')} "
                f"result={e.get('result')} error={e.get('error')}"
            )

    # Final result
    print(f"\n{'=' * 60}")
    expected = bob_msg_count_before + 1 if bob_msg_count_before else 2
    if (
        alice_count_2 >= expected
        and bob_count_2 >= expected
        and carol_count_2 >= expected
    ):
        print("  PASS: Viewer write synced to all peers")
    else:
        print(
            f"  FAIL: alice={alice_count_2} bob={bob_count_2} carol={carol_count_2} (expected >= {expected})"
        )
        if merged2:
            print(f"  Captures: {merged2}")
    print(f"{'=' * 60}")
