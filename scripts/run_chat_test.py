#!/usr/bin/env python3
"""
Run Group Chat with THREE users for interactive testing.

This tests the third viewer bug where:
- Third viewer only sees 1 online instead of 3
- Messages from third viewer don't sync
- After restart, it works

Usage:
    python scripts/run_chat_test.py

After starting, attach to tmux:
    tmux attach -t chat_demo
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"


def main():
    print("Starting THREE-user chat test (reproducing third viewer bug)...")

    tm = TmuxManager(
        session_name="chat_demo",
        base_dir=Path("/tmp/chat_demo"),
    )
    tm.add_node("node")
    tm.add_shell("alice")
    tm.add_shell("bob")
    tm.add_shell("carol")  # Third viewer
    tm.start()

    node = tm.get_client("node")
    alice = tm.get_client("alice")
    bob = tm.get_client("bob")
    carol = tm.get_client("carol")  # Third viewer
    print("  All instances ready (alice, bob, carol)")

    # Alice setup
    print("Alice: signup, create space...")
    alice.signup_or_login("alice")
    space = alice.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]

    pages = alice.list_pages(space_id)
    page_id = pages[0]["id"]
    print(f"  Space: {space_id[:8]}..., Page: {page_id[:8]}...")

    # Connect and publish
    print("Alice: connect to node, publish...")
    alice.connect_to_node(node)

    # Wait for authentication and publish with retry
    print("  Waiting for peer authentication...")
    time.sleep(3)  # Initial delay for handshake
    for i in range(20):
        try:
            alice.publish_to_node(space_id)
            print(f"  Published after {i+1} attempts")
            break
        except RuntimeError as e:
            err = str(e)
            if "No PeerActor" in err or "No nodes" in err or "not authenticated" in err:
                time.sleep(1)
                continue
            raise
    else:
        raise RuntimeError("Failed to publish after 20 attempts")

    time.sleep(2)

    # Bob setup
    print("Bob: signup, subscribe...")
    bob.signup_or_login("bob")
    alice.add_viewer(bob, space_id)

    # Wait for Bob to sync
    print("Waiting for Bob to sync...")
    bob_page_id = None
    for i in range(15):
        time.sleep(1)
        spaces = bob.list_spaces()
        if spaces:
            bob_pages = bob.list_pages(spaces[0]["id"])
            if bob_pages:
                apps = bob.list_apps(bob_pages[0]["id"])
                app_names = [a.get("name") for a in apps]
                if "Group Chat" in app_names:
                    print(f"  Bob synced in {i+1}s (apps: {app_names})")
                    bob_page_id = bob_pages[0]["id"]
                    break

    if not bob_page_id:
        print("ERROR: Bob failed to sync Group Chat app")
        return 1

    # PRODUCTION-LIKE FLOW: Alice and Bob open apps FIRST
    print("Opening Group Chat for Alice and Bob FIRST (production flow)...")
    alice.open_app(page_id, "Group Chat")
    time.sleep(2)  # Wait for Slint event loop to process app launch
    bob.open_app(bob_page_id, "Group Chat")
    time.sleep(2)  # Wait for Slint event loop to process app launch

    # Wait for apps to be ready
    def wait_for_app(client, name):
        for i in range(15):
            time.sleep(1)
            try:
                client.eval("return 1")
                print(f"  {name} app ready after {i+1}s")
                return True
            except:
                pass
        return False

    if not wait_for_app(alice, "Alice"):
        print("ERROR: Alice app not ready")
        return 1
    if not wait_for_app(bob, "Bob"):
        print("ERROR: Bob app not ready")
        return 1

    # Set usernames for Alice and Bob
    alice.eval('USERNAME = "Alice"')
    alice.eval('my_name = USERNAME')
    bob.eval('USERNAME = "Bob"')
    bob.eval('my_name = USERNAME')

    # Check Alice and Bob see each other
    alice_count_before = alice.eval('online_count')
    bob_count_before = bob.eval('online_count')
    print(f"  Before Carol: Alice sees {alice_count_before}, Bob sees {bob_count_before}")

    # NOW Carol joins (AFTER Alice and Bob are already chatting)
    print("\n--- Carol joins LATER (production scenario) ---")
    print("Carol: signup, subscribe (THIRD VIEWER)...")
    carol.signup_or_login("carol")
    alice.add_viewer(carol, space_id)

    # Wait for Carol to sync
    print("Waiting for Carol to sync...")
    carol_page_id = None
    for i in range(15):
        time.sleep(1)
        spaces = carol.list_spaces()
        if spaces:
            carol_pages = carol.list_pages(spaces[0]["id"])
            if carol_pages:
                apps = carol.list_apps(carol_pages[0]["id"])
                app_names = [a.get("name") for a in apps]
                if "Group Chat" in app_names:
                    print(f"  Carol synced in {i+1}s (apps: {app_names})")
                    carol_page_id = carol_pages[0]["id"]
                    break

    if not carol_page_id:
        print("ERROR: Carol failed to sync Group Chat app")
        return 1

    # Carol opens app (this is where bug should appear in production)
    print("NOW Carol opens app...")
    carol.open_app(carol_page_id, "Group Chat")

    if not wait_for_app(carol, "Carol"):
        print("ERROR: Carol app not ready")
        return 1

    # Set username for Carol
    carol.eval('USERNAME = "Carol"')
    carol.eval('my_name = USERNAME')

    # Check online counts (this is where the bug shows)
    time.sleep(2)
    print("\n" + "=" * 50)
    print("  CHECKING ONLINE COUNTS (BUG CHECK)")
    print("=" * 50)
    try:
        alice_count = alice.eval('online_count')
        bob_count = bob.eval('online_count')
        carol_count = carol.eval('online_count')
        print(f"  Alice sees: {alice_count} online (should be 3)")
        print(f"  Bob sees:   {bob_count} online (should be 3)")
        print(f"  Carol sees: {carol_count} online (should be 3) <-- BUG if 1")
        if carol_count == 1:
            print("\n  *** BUG REPRODUCED: Carol only sees 1 online! ***")
    except Exception as e:
        print(f"  Could not check online counts: {e}")

    print("\n" + "=" * 50)
    print("  CHAT READY - THREE windows open!")
    print("=" * 50)
    print("\nAttach to tmux: tmux attach -t chat_demo")
    print("\nType messages in any window and see them sync!")
    print("Watch Carol's window - messages may not sync!")
    print("\nPress Ctrl+C to exit (session stays alive)")

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nSession still running.")
        print("Attach: tmux attach -t chat_demo")
        print("Kill:   tmux kill-session -t chat_demo")


if __name__ == "__main__":
    sys.exit(main() or 0)
