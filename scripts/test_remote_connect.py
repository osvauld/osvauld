#!/usr/bin/env python3
"""
Remote Connection Test — sets up owner + node, prints connection string for external peer.

Starts one owner and one node on the local machine. The owner creates a space,
connects to the node, publishes, and generates a shareable link. The script then
prints the connection string and diagnostic info (endpoint addr, relay URLs) so
you can share it with someone outside the network.

The script stays alive waiting for the remote peer to connect.

Usage:
    python scripts/test_remote_connect.py
    python scripts/test_remote_connect.py --app-path sample_apps/osvauld-demos --app-name "Group Chat"
    python scripts/test_remote_connect.py --keep    # keep instances running after Ctrl+C
    python scripts/test_remote_connect.py --release  # use release binary
"""

import argparse
import base64
import json
import signal
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.scenario import Scenario


def parse_args():
    parser = argparse.ArgumentParser(description="Remote Connection Test")
    parser.add_argument("--keep", action="store_true", help="Keep instances running on exit")
    parser.add_argument("--app-path", type=str, default=None, help="Path to app directory to import")
    parser.add_argument("--app-name", type=str, default=None, help="App name to open after setup")
    parser.add_argument("--release", action="store_true", help="Use release binary")
    parser.add_argument("--fresh", action="store_true", default=True, help="Clean data dirs (default: True)")
    parser.add_argument("--no-fresh", action="store_false", dest="fresh", help="Keep existing data dirs")
    return parser.parse_args()


def decode_connection_string(conn_str: str) -> dict:
    """Decode base64 connection string to inspect its contents."""
    try:
        decoded = json.loads(base64.b64decode(conn_str))
        return decoded
    except Exception as e:
        return {"error": str(e)}


def main():
    args = parse_args()
    scenario = Scenario(owner=1, node=1, passphrase="test123", fresh=args.fresh)

    keep = args.keep

    def signal_handler(sig, frame):
        print("\n\nShutting down...")
        if not keep:
            scenario.stop()
        else:
            print("--keep flag set, instances still running")
            print(f"  Node socket: {scenario.node.socket_path}")
            print(f"  Owner socket: {scenario.owner.socket_path}")
        sys.exit(0)

    signal.signal(signal.SIGINT, signal_handler)

    try:
        print("=" * 60)
        print("  Remote Connection Test")
        print("=" * 60)

        # Start node + owner
        print("\n[1/5] Starting node and owner...")
        scenario.start()

        node = scenario.node
        owner = scenario.owner

        print(f"  Node started: {node.name}")
        print(f"  Owner started: {owner.name}")

        # Signup owner
        print("\n[2/5] Signing up owner...")
        owner.client.signup_or_login("alice", "test123")
        print("  Owner signed up as 'alice'")

        # Get node connection string and diagnostics
        print("\n[3/5] Getting node diagnostics...")
        node_conn_str = node.connection_string
        node_details = decode_connection_string(node_conn_str)

        print(f"  Node name: {node_details.get('name', 'unknown')}")
        print(f"  Relay URL: {node_details.get('relay', 'NONE')}")
        if node_details.get("device_public_key"):
            key_bytes = base64.b64decode(node_details["device_public_key"])
            node_id_hex = key_bytes.hex()
            print(f"  Node ID: {node_id_hex}")

        # Owner connects to node
        print("\n[4/5] Owner connecting to node...")
        result = owner.client.connect_to_node(node)
        nodes = owner.client.list_nodes()
        if nodes:
            node_id = nodes[0].get("node_id", "")
            print(f"  Connected to node: {node_id[:16]}...")

            # Wait for auth
            print("  Waiting for authentication...")
            if owner.client.wait_for_node_auth(node_id, timeout=30):
                print("  Authenticated with node")
            else:
                print("  WARNING: Auth timeout (continuing anyway)")

            # Create space + import page if app path given
            if args.app_path:
                print(f"\n  Importing app from {args.app_path}...")
                space_result = owner.client.create_space_with_pages(args.app_path)
                space_id = space_result.get("space_id", "")
                print(f"  Space created: {space_id[:16]}...")

                # Publish space to node
                print("  Publishing space to node...")
                owner.client.publish_space(space_id, node_id)
                time.sleep(2)  # wait for publish

                # Get shareable link
                print("\n[5/5] Generating shareable link...")
                viewer_link = owner.client.get_shareable_link(space_id, node_id)
                viewer_details = decode_connection_string(viewer_link)

                print(f"\n{'=' * 60}")
                print("  SHAREABLE LINK (give this to remote peer)")
                print(f"{'=' * 60}")
                print(f"\n{viewer_link}\n")
                print(f"{'=' * 60}")

                print(f"\n  Link details:")
                print(f"    Relay: {viewer_details.get('relay', 'NONE')}")
                if viewer_details.get("device_public_key"):
                    vk_bytes = base64.b64decode(viewer_details["device_public_key"])
                    print(f"    Node ID: {vk_bytes.hex()}")

                if args.app_name:
                    # Open app on owner
                    pages = owner.client.list_pages(space_id)
                    if pages:
                        page_id = pages[0].get("id", "")
                        print(f"\n  Opening app '{args.app_name}' on owner...")
                        owner.client.open_app(page_id, args.app_name)
            else:
                # No app, just generate owner connection string
                print("\n[5/5] Generating connection string...")
                owner_conn_str = owner.client.get_connection_string()
                owner_details = decode_connection_string(owner_conn_str)

                print(f"\n{'=' * 60}")
                print("  NODE CONNECTION STRING (give this to remote peer)")
                print(f"{'=' * 60}")
                print(f"\n{node_conn_str}\n")
                print(f"{'=' * 60}")
        else:
            print("  WARNING: No nodes found after connect")

        print(f"\n{'=' * 60}")
        print("  READY — waiting for remote peer to connect")
        print("  Press Ctrl+C to stop")
        print(f"{'=' * 60}")
        print(f"\n  Node socket: {node.socket_path}")
        print(f"  Owner socket: {owner.socket_path}")
        print()

        # Poll and print connection events
        while True:
            try:
                nodes = owner.client.list_nodes()
                for n in nodes:
                    status = "connected" if n.get("is_connected") else "disconnected"
                    print(f"\r  Node {n.get('name', '?')}: {status}    ", end="", flush=True)
            except Exception:
                pass
            time.sleep(5)

    except KeyboardInterrupt:
        pass
    finally:
        if not keep:
            print("\nCleaning up...")
            scenario.stop()
        else:
            print("\n--keep flag set, instances still running")


if __name__ == "__main__":
    main()
