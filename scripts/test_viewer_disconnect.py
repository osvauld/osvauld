#!/usr/bin/env python3
"""
Test: Viewer Disconnect Warning Suppression

Verifies the bug fix for:
- After Carol (viewer) quits, node should NOT log a WARN about missing device info
- Instead, it should log DEBUG: "Viewer not connected (will reconnect when online)"

This test:
1. Starts node, Alice (owner), Carol (viewer)
2. Alice creates space, publishes to node
3. Alice invites Carol as viewer
4. Carol syncs and connects
5. Carol disconnects (Ctrl+C simulation)
6. Verify node logs show DEBUG not WARN for missing device info

Usage:
    python scripts/test_viewer_disconnect.py
    python scripts/test_viewer_disconnect.py --keep   # Keep session alive after test
"""

import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"
KEEP_SESSION = False


def capture_node_logs(session_name: str, window_index: int = 0) -> str:
    """Capture logs from a tmux window."""
    result = subprocess.run(
        ["tmux", "capture-pane", "-t", f"{session_name}:{window_index}", "-p", "-S", "-500"],
        capture_output=True,
        text=True,
    )
    return result.stdout


def main():
    global KEEP_SESSION
    if "--keep" in sys.argv:
        KEEP_SESSION = True

    print("=" * 60)
    print("  Viewer Disconnect Warning Suppression Test")
    print("=" * 60)

    tm = TmuxManager(
        session_name="disconnect_test",
        base_dir=Path("/tmp/disconnect_test"),
    )
    tm.add_node("node")
    tm.add_shell("alice")
    tm.add_shell("carol")
    tm.start()

    node = tm.get_client("node")
    alice = tm.get_client("alice")
    carol = tm.get_client("carol")
    print("[1/7] All instances ready (node, alice, carol)")

    try:
        # Alice setup
        print("[2/7] Alice: signup, create space...")
        alice.signup_or_login("alice")
        space = alice.create_space_with_pages(str(DEMOS_APP))
        space_id = space["id"]
        pages = alice.list_pages(space_id)
        page_id = pages[0]["id"]
        print(f"       Space: {space_id[:8]}..., Page: {page_id[:8]}...")

        # Connect and publish
        print("[3/7] Alice: connect to node, publish...")
        alice.connect_to_node(node)
        time.sleep(3)

        for i in range(10):
            try:
                alice.publish_to_node(space_id)
                print(f"       Published after {i+1} attempts")
                break
            except RuntimeError as e:
                if "No PeerActor" in str(e) or "not authenticated" in str(e):
                    time.sleep(1)
                    continue
                raise
        else:
            raise RuntimeError("Failed to publish after 10 attempts")

        time.sleep(2)

        # Carol setup
        print("[4/7] Carol: signup, get invited...")
        carol.signup_or_login("carol")
        alice.add_viewer(carol, space_id)

        # Wait for Carol to sync
        print("[5/7] Waiting for Carol to sync...")
        carol_synced = False
        for i in range(15):
            time.sleep(1)
            spaces = carol.list_spaces()
            if spaces:
                carol_pages = carol.list_pages(spaces[0]["id"])
                if carol_pages:
                    print(f"       Carol synced in {i+1}s")
                    carol_synced = True
                    break

        if not carol_synced:
            print("ERROR: Carol failed to sync")
            return 1

        # Give some time for handshake and contact storage
        print("[6/7] Waiting for handshake and contact storage...")
        time.sleep(3)

        # Capture node logs BEFORE disconnect
        logs_before = capture_node_logs("disconnect_test", 0)

        # Now simulate Carol disconnect by stopping her shell
        print("[7/7] Simulating Carol disconnect...")
        # Send Ctrl+C to Carol's window to stop her shell
        subprocess.run([
            "tmux", "send-keys", "-t", "disconnect_test:2", "C-c"
        ])
        time.sleep(2)

        # Capture node logs AFTER disconnect
        logs_after = capture_node_logs("disconnect_test", 0)

        # Check for the bug: WARN about "No device info found"
        # The fix changes this to DEBUG level in Node mode
        new_logs = logs_after[len(logs_before):]

        print("\n" + "=" * 60)
        print("  Checking node logs after Carol disconnect...")
        print("=" * 60)

        # Look for the warning pattern that should NOT appear
        has_warn_no_device = "WARN" in new_logs and "No device info found" in new_logs
        has_debug_viewer_not_connected = "Viewer not connected" in new_logs

        if has_warn_no_device:
            print("\n[FAIL] Found WARN about 'No device info found'!")
            print("       The bug is NOT fixed.")
            # Print relevant log lines
            for line in new_logs.split("\n"):
                if "No device info" in line or "WARN" in line:
                    print(f"       > {line}")
            return 1

        print("\n[PASS] No WARN about 'No device info found' in logs")

        # Note: DEBUG logs might not appear depending on log level configuration
        # The important thing is no WARN
        if has_debug_viewer_not_connected:
            print("[INFO] DEBUG log 'Viewer not connected' found (good)")
        else:
            print("[INFO] DEBUG log not visible (log level may filter it)")

        print("\n" + "=" * 60)
        print("  [SUCCESS] Viewer Disconnect Warning Suppression Test Passed!")
        print("=" * 60)
        return 0

    except Exception as e:
        print(f"\n[FAIL] Test error: {e}")
        import traceback
        traceback.print_exc()
        return 1

    finally:
        if not KEEP_SESSION:
            tm.stop()
        else:
            print("\nSession still running: tmux attach -t disconnect_test")


if __name__ == "__main__":
    sys.exit(main())
