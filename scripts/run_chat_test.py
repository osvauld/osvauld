#!/usr/bin/env python3
"""
Run Group Chat with two users for interactive testing.

Usage:
    python scripts/run_chat_test.py

After starting, attach to tmux:
    tmux attach -t chat_demo
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from lib.tmux import TmuxManager

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"


def main():
    print("Starting two-user chat test...")

    tm = TmuxManager(
        session_name="chat_demo",
        base_dir=Path("/tmp/chat_demo"),
    )
    tm.add_node("node")
    tm.add_shell("alice")
    tm.add_shell("bob")
    tm.start()

    node = tm.get_client("node")
    alice = tm.get_client("alice")
    bob = tm.get_client("bob")
    print("  All instances ready")

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
    time.sleep(2)
    alice.publish_to_node(space_id)
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

    # Open apps
    print("Opening Group Chat for both users...")
    alice.open_app(page_id, "Group Chat")
    time.sleep(2)
    bob.open_app(bob_page_id, "Group Chat")
    time.sleep(2)

    # Initialize with usernames
    alice.eval('USERNAME = "Alice"')
    bob.eval('USERNAME = "Bob"')
    alice.eval("on_init()")
    bob.eval("on_init()")

    print("\n" + "=" * 50)
    print("  CHAT READY - Two windows open!")
    print("=" * 50)
    print("\nAttach to tmux: tmux attach -t chat_demo")
    print("\nType messages in either window and see them sync!")
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
