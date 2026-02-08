#!/usr/bin/env python3
"""
Minimal test for app opening - isolates the "app not ready" bug.

Usage:
    python scripts/test_app_open.py          # Simple test (no sync)
    python scripts/test_app_open.py --sync   # With node and Bob syncing
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"


def wait_for_app(client, name, timeout=15):
    """Wait for app to be ready, return True on success."""
    for i in range(timeout):
        time.sleep(1)
        try:
            result = client.eval("return 1")
            print(f"   {name} app ready after {i+1}s (eval returned {result})")
            return True
        except Exception as e:
            print(f"   {name} attempt {i+1}: {e}")
    return False


def test_simple():
    """Simple test - just Alice, no sync."""
    print("=== Simple App Open Test (no sync) ===\n")

    tm = TmuxManager(
        session_name="app_open_test",
        base_dir=Path("/tmp/app_open_test"),
    )
    tm.add_shell("alice")
    tm.start()

    alice = tm.get_client("alice")
    print("1. Alice instance ready")

    alice.signup_or_login("alice")
    print("2. Alice signed up")

    space = alice.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]
    pages = alice.list_pages(space_id)
    page_id = pages[0]["id"]
    print(f"3. Space created: {space_id[:8]}...")

    print("4. Opening Group Chat app...")
    alice.open_app(page_id, "Group Chat")

    print("5. Waiting for app ready...")
    if not wait_for_app(alice, "Alice"):
        print("\n   FAILED: App not ready")
        return 1

    print("\n=== Test PASSED ===")
    return 0


def test_with_sync():
    """Test with node and Bob syncing - reproduces the bug scenario."""
    print("=== App Open Test WITH SYNC (reproduces bug) ===\n")

    tm = TmuxManager(
        session_name="app_open_test",
        base_dir=Path("/tmp/app_open_test"),
    )
    tm.add_node("node")
    tm.add_shell("alice")
    tm.add_shell("bob")
    tm.start()

    node = tm.get_client("node")
    alice = tm.get_client("alice")
    bob = tm.get_client("bob")
    print("1. All instances ready (node, alice, bob)")

    # Alice setup
    alice.signup_or_login("alice")
    space = alice.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]
    pages = alice.list_pages(space_id)
    page_id = pages[0]["id"]
    print(f"2. Alice: space={space_id[:8]}..., page={page_id[:8]}...")

    # Connect and publish
    alice.connect_to_node(node)
    print("3. Alice connected to node, waiting for auth...")
    time.sleep(3)

    for i in range(10):
        try:
            alice.publish_to_node(space_id)
            print(f"4. Alice published after {i+1} attempts")
            break
        except RuntimeError as e:
            if "not authenticated" in str(e) or "No PeerActor" in str(e):
                time.sleep(1)
                continue
            raise
    else:
        print("   FAILED: Could not publish")
        return 1

    time.sleep(2)

    # Bob syncs
    bob.signup_or_login("bob")
    alice.add_viewer(bob, space_id)
    print("5. Bob added as viewer, syncing...")

    bob_page_id = None
    for i in range(15):
        time.sleep(1)
        spaces = bob.list_spaces()
        if spaces:
            bob_pages = bob.list_pages(spaces[0]["id"])
            if bob_pages:
                apps = bob.list_apps(bob_pages[0]["id"])
                if any(a.get("name") == "Group Chat" for a in apps):
                    bob_page_id = bob_pages[0]["id"]
                    print(f"6. Bob synced in {i+1}s")
                    break

    if not bob_page_id:
        print("   FAILED: Bob did not sync")
        return 1

    # NOW open apps (this is where the bug appears)
    print("\n7. Opening apps (production flow)...")
    alice.open_app(page_id, "Group Chat")
    time.sleep(2)
    bob.open_app(bob_page_id, "Group Chat")
    time.sleep(2)

    print("\n8. Checking if apps are ready...")
    alice_ok = wait_for_app(alice, "Alice")
    bob_ok = wait_for_app(bob, "Bob")

    if not alice_ok:
        print("\n   *** BUG REPRODUCED: Alice app not ready ***")
        print("   Attach to tmux: tmux attach -t app_open_test")
        return 1

    if not bob_ok:
        print("\n   FAILED: Bob app not ready")
        return 1

    print("\n=== Test PASSED ===")
    return 0


def main():
    if "--sync" in sys.argv:
        return test_with_sync()
    else:
        return test_simple()


if __name__ == "__main__":
    sys.exit(main() or 0)
