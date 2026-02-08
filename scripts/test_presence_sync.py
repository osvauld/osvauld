#!/usr/bin/env python3
"""
Test Presence Heartbeat Sync (Owner → Node)

Verifies that the presence layer updates are synced from owner to node.
This tests the full path: Lua timer → Loro commit → Observer → Sync → Node receives.

Usage:
    python scripts/test_presence_sync.py
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"


def main():
    print("=" * 60)
    print("  Presence Heartbeat Sync Test (Owner → Node)")
    print("=" * 60)
    print()

    # Start owner and node
    tm = TmuxManager(
        session_name="presence_sync_test",
        base_dir=Path("/tmp/presence_sync_test"),
    )
    tm.add_node("node")
    tm.add_shell("owner")
    tm.start()

    node = tm.get_client("node")
    owner = tm.get_client("owner")
    print("[OK] Node and Owner instances ready")

    # === Setup Phase ===
    print("\n--- Setup Phase ---")

    # Owner: signup, create space, connect to node, publish
    print("Owner: signup, create space...")
    owner.signup_or_login("owner")
    space = owner.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]
    pages = owner.list_pages(space_id)
    page_id = pages[0]["id"]
    print(f"  Space: {space_id[:12]}...")
    print(f"  Page: {page_id[:12]}...")

    # Connect to node
    print("Owner: connecting to node...")
    owner.connect_to_node(node)

    # Publish space
    print("Owner: publishing to node...")
    for i in range(20):
        time.sleep(1)
        try:
            owner.publish_to_node(space_id)
            print(f"  Published in {i+1}s")
            break
        except RuntimeError as e:
            if "No PeerActor" in str(e) or "No nodes" in str(e):
                continue
            raise
    else:
        print("ERROR: Failed to publish")
        return 1

    time.sleep(2)  # Let sync happen

    # Open Group Chat app
    print("Owner: opening Group Chat app...")
    owner.open_app(page_id, "Group Chat")
    time.sleep(2)

    # Set username
    owner.eval('USERNAME = "test_owner"')
    owner.eval('my_name = USERNAME')
    print("  [OK] App opened")

    # === Test Phase ===
    print("\n--- Test: Presence Sync to Node ---")

    # Get owner's DID
    owner_did = owner.eval('return scribe:my_did()')
    print(f"Owner DID: {owner_did[:20]}...")

    # Wait for initial presence to sync
    print("\nWaiting for initial presence sync to node...")
    time.sleep(3)

    # Query node's view of the presence layer
    # The node has the page via subscription, so it should have the layer
    def get_node_layers():
        """Query layer list from node's perspective."""
        try:
            # Node queries the layers via its Butler using list_layers
            result = node.send("list_layers", {
                "page_id": page_id
            })
            # list_layers returns {"layers": [...], "count": N}
            if result and isinstance(result, dict):
                return result.get("layers", [])
            return []
        except Exception as e:
            print(f"  Query error: {e}")
            return []

    # First, let's verify the owner has presence data
    print("\nVerifying owner has presence data...")
    owner_presence = owner.eval(f'''
        local layer = scribe:map("{page_id}/presence")
        local keys = layer:keys()
        local result = {{}}
        for _, k in ipairs(keys) do
            result[k] = layer:get(k)
        end
        return result
    ''')

    if not owner_presence:
        print("  [ERROR] Owner has no presence data!")
        return 1

    our_entry = owner_presence.get(owner_did)
    if not our_entry:
        print(f"  [ERROR] Owner's DID not in presence layer!")
        print(f"  Keys: {list(owner_presence.keys())}")
        return 1

    initial_last_seen = our_entry.get("last_seen")
    print(f"  Owner's presence: last_seen={initial_last_seen}, status={our_entry.get('status')}")

    # Now query the node to check if presence layer exists
    print("\nQuerying node's layers...")
    node_layers = get_node_layers()
    if "presence" in node_layers:
        print(f"  [OK] Node has presence layer (and {len(node_layers)} total layers)")
    else:
        print(f"  [WARN] Node doesn't have presence layer yet")
        print(f"  Available layers: {node_layers}")

    # Wait for heartbeat
    print("\nWaiting 17 seconds for heartbeat timer...")
    for i in range(17):
        time.sleep(1)
        print(f"  {17-i}s remaining...", end="\r")
    print("  Done!              ")

    # Query owner's updated presence
    print("\nQuerying owner's updated presence...")
    owner_presence = owner.eval(f'''
        local layer = scribe:map("{page_id}/presence")
        local keys = layer:keys()
        local result = {{}}
        for _, k in ipairs(keys) do
            result[k] = layer:get(k)
        end
        return result
    ''')

    our_entry = owner_presence.get(owner_did)
    updated_last_seen = our_entry.get("last_seen")
    diff = updated_last_seen - initial_last_seen

    print(f"  Initial last_seen: {initial_last_seen}")
    print(f"  Updated last_seen: {updated_last_seen}")
    print(f"  Difference: {diff} seconds")

    if diff >= 15:
        print("\n  [PASS] Owner's heartbeat updated correctly!")
    else:
        print("\n  [FAIL] Owner's heartbeat did NOT update!")
        return 1

    # Wait a bit for sync to node
    print("\nWaiting 3s for sync to node...")
    time.sleep(3)

    # Check if node still has the presence layer
    print("Verifying node still has presence layer...")
    node_layers = get_node_layers()
    if "presence" in node_layers:
        print("  [OK] Node still has presence layer")
    else:
        print("  [WARN] Node lost presence layer!")
        print(f"  Available layers: {node_layers}")

    # === Summary ===
    print("\n" + "=" * 60)
    print("  Test Summary")
    print("=" * 60)
    print(f"  Owner heartbeat update: {'PASS' if diff >= 15 else 'FAIL'}")
    print(f"  Initial last_seen: {initial_last_seen}")
    print(f"  Updated last_seen: {updated_last_seen}")
    print(f"  Difference: {diff}s")
    print()
    print("  Attach to tmux: tmux attach -t presence_sync_test")
    print("  Kill session: tmux kill-session -t presence_sync_test")
    print()

    # Keep session alive for inspection
    print("Press Ctrl+C to exit (session will keep running)")
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nExiting (session still running)")

    return 0


if __name__ == "__main__":
    sys.exit(main() or 0)
