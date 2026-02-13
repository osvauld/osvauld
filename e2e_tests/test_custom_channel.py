#!/usr/bin/env python3
"""
Custom Channel + DM Capture Test.

Runs two end-to-end cases in one scenario and captures events for each case:
1) Custom channel flow (owner + viewers)
2) DM flow (owner <-> viewer)

Usage:
    python e2e_tests/test_custom_channel.py
    python e2e_tests/test_custom_channel.py --keep
    python e2e_tests/test_custom_channel.py --capture-logs
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

args = AppTestScenario.parse_args("Custom Channel Capture Test")

with AppTestScenario(
    name="custom_channel_test",
    app_path=str(DEMOS_APP),
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

    def print_capture_summary(label, captures_dir, merged):
        if merged and merged.exists():
            line_count = sum(1 for _ in open(merged))
            print(f"  Captured {line_count} events -> {merged}")
            print("  Per-instance captures:")
            for name in ["node", "alice", "bob", "carol"]:
                f = captures_dir / f"{label}_{name}.jsonl"
                if f.exists():
                    n = sum(1 for _ in open(f))
                    print(f"    {name:8s}: {n} events")
            print("  Quick filters:")
            print(f"    grep 'sync_event' {merged}")
            print(f"    grep 'message_trace' {merged}")
            print(f"    grep 'page_update' {merged}")
            print(f"    grep 'LayerChanged' {merged}")
            print(f"    grep 'NewDynamicLayer' {merged}")
            print(f"    grep 'LayerAccessChanged' {merged}")
        else:
            print("  Warning: No capture data collected")

    print("\n=== CASE 1: Custom channel flow ===")

    # Start capture for custom channel flow
    print("[1/5] Starting channel event capture...")
    captures_dir = s.capture_start("custom_channel", include_logs=include_logs)
    print(f"  Capturing to {captures_dir}/")

    # Alice creates a custom channel (dynamic layer)
    print("[2/5] Alice creates custom channel 'project-x'...")
    alice.eval('create_channel("project-x")')
    time.sleep(2)

    # Alice sends a message in the new channel (create_channel auto-switches)
    print("[3/5] Alice sends message in 'project-x'...")
    alice.eval('send_message("Owner message in project-x")')
    time.sleep(2)

    # Bob switches to project-x and tries to send
    print("[4/5] Bob switches to 'project-x' and sends message...")
    bob.eval('switch_channel("project-x")')
    time.sleep(1)
    try:
        bob.eval('send_message("Bob message in project-x")')
        print("  Bob send_message returned OK")
    except Exception as e:
        print(f"  Bob send_message error: {e}")

    # Carol switches and tries to send
    print("  Carol switches to 'project-x' and sends message...")
    carol.eval('switch_channel("project-x")')
    time.sleep(1)
    try:
        carol.eval('send_message("Carol message in project-x")')
        print("  Carol send_message returned OK")
    except Exception as e:
        print(f"  Carol send_message error: {e}")

    # Wait for sync
    print("[5/5] Waiting 2s for sync...")
    time.sleep(2)

    # Check what each peer sees
    alice_count = alice.eval("return get_message_count()")
    bob_count = bob.eval("return get_message_count()")
    carol_count = carol.eval("return get_message_count()")
    print(f"\n  Message counts in project-x:")
    print(f"    Alice (owner):  {alice_count}")
    print(f"    Bob (viewer):   {bob_count}")
    print(f"    Carol (viewer): {carol_count}")

    # Stop channel capture and dump
    print("\n  Stopping channel capture and merging...")
    merged = s.capture_end("custom_channel", merge=True)
    print_capture_summary("custom_channel", captures_dir, merged)

    print("\n=== CASE 2: DM flow ===")

    alice_did = alice.eval("return permit:my_did()")
    bob_did = bob.eval("return permit:my_did()")

    print("[1/5] Starting DM event capture...")
    captures_dir_dm = s.capture_start("dm_flow", include_logs=include_logs)
    print(f"  Capturing to {captures_dir_dm}/")

    print("[2/5] Alice creates DM with Bob...")
    alice.eval(f'create_dm("{bob_did}", "bob")')
    time.sleep(2)

    print("[3/5] Alice sends DM message...")
    alice.eval('send_dm_message("Hello Bob (DM)")')
    time.sleep(2)

    print("[4/5] Bob opens Alice's DM and replies...")
    opened = False
    for _ in range(10):
        opened = bool(bob.eval(f'return open_dm("{alice_did}", "alice")'))
        if opened:
            break
        time.sleep(0.5)
    print(f"  Bob open_dm: {'OK' if opened else 'NOT FOUND'}")
    time.sleep(1)
    bob.eval('send_dm_message("Hello Alice (DM reply)")')
    time.sleep(2)

    print("[5/5] Collecting DM counts...")
    alice_dm_count = alice.eval("return get_dm_message_count()")
    bob_dm_count = bob.eval("return get_dm_message_count()")
    carol_dm_count = carol.eval("return get_dm_message_count()")
    print("  DM message counts:")
    print(f"    Alice (owner):  {alice_dm_count}")
    print(f"    Bob (viewer):   {bob_dm_count}")
    print(f"    Carol (viewer): {carol_dm_count}")

    print("\n  Stopping DM capture and merging...")
    merged_dm = s.capture_end("dm_flow", merge=True)
    print_capture_summary("dm_flow", captures_dir_dm, merged_dm)

    print(f"\n{'=' * 60}")
    if alice_count >= 3 and bob_count >= 3 and carol_count >= 3:
        print("  [CHANNEL OK] Custom channel messages synced to all peers")
    else:
        print(f"  [CHANNEL FAIL] Expected 3 messages on all peers, got alice={alice_count} bob={bob_count} carol={carol_count}")

    if alice_dm_count >= 2 and bob_dm_count >= 2:
        print("  [DM OK] DM messages synced between Alice and Bob")
    else:
        print("  [DM ISSUE] DM sync is incomplete")

    if carol_dm_count == 0:
        print("  [DM OK] Carol has no active DM messages")
    else:
        print("  [DM NOTE] Carol has DM messages; inspect captures for why")

    print("  Captures available for both cases:")
    print("    - custom_channel_*.jsonl + custom_channel_merged.jsonl")
    print("    - dm_flow_*.jsonl + dm_flow_merged.jsonl")
    print(f"{'=' * 60}")
