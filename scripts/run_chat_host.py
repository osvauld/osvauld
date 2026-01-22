#!/usr/bin/env python3
"""
Host a Group Chat session for external viewers.

Usage:
    python scripts/run_chat_host.py

This starts:
1. A kunki node (relay)
2. An owner instance with the chat app

Then you can connect a viewer from another system using the node address.
"""

import sys
import time
from pathlib import Path

# Unbuffered output
sys.stdout.reconfigure(line_buffering=True)

sys.path.insert(0, str(Path(__file__).parent))

from lib.tmux import TmuxManager

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"


def main():
    print("Starting Chat Host...")

    tm = TmuxManager(
        session_name="chat_host",
        base_dir=Path("/tmp/chat_host"),
    )
    tm.add_node("node")
    tm.add_shell("owner")
    tm.start()

    node = tm.get_client("node")
    owner = tm.get_client("owner")
    print("  Instances ready")

    # Get node address
    node_addr = node.get_connection_string()
    print(f"\n  Node address: {node_addr}")

    # Owner setup
    print("\nOwner: signup, create space...")
    owner.signup_or_login("host")
    space = owner.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]

    pages = owner.list_pages(space_id)
    page_id = pages[0]["id"]

    apps = owner.list_apps(page_id)
    app_names = [a.get("name") for a in apps]
    print(f"  Space: {space_id[:8]}...")
    print(f"  Page: {page_id[:8]}...")
    print(f"  Apps: {app_names}")

    # Connect and publish
    print("\nOwner: connect to node, publish...")
    owner.connect_to_node(node)
    time.sleep(2)
    owner.publish_to_node(space_id)
    time.sleep(2)
    print("  Published!")

    # Open chat app
    print("\nOpening Group Chat...")
    owner.open_app(page_id, "Group Chat")
    time.sleep(2)
    owner.eval('USERNAME = "Host"')
    owner.eval("on_init()")

    # Get share info for viewer
    print("\n" + "=" * 60)
    print("  CHAT HOST READY")
    print("=" * 60)
    print(f"\nNode address: {node_addr}")
    print(f"Space ID:     {space_id}")
    print(f"Page ID:      {page_id}")
    print("\nTo connect a viewer from another system:")
    print(f"  1. Connect to node: {node_addr}")
    print(f"  2. Subscribe to space: {space_id}")
    print("\nAttach to tmux: tmux attach -t chat_host")
    print("Press Ctrl+C to exit (session stays alive)")

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nSession still running.")
        print("Attach: tmux attach -t chat_host")
        print("Kill:   tmux kill-session -t chat_host")


if __name__ == "__main__":
    sys.exit(main() or 0)
