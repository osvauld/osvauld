#!/usr/bin/env python3
"""
Test Presence Heartbeat (Local, No Node)

Verifies that the presence layer updates locally via heartbeat timer.
Does NOT require a node - tests local Lua → Scribe → Loro path.

Usage:
    python scripts/test_presence_local.py
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"


def main():
    print("=" * 60)
    print("  Presence Heartbeat Test (Local, No Node)")
    print("=" * 60)
    print()

    # Start just one shell instance - no node needed
    tm = TmuxManager(
        session_name="presence_local_test",
        base_dir=Path("/tmp/presence_local_test"),
    )
    tm.add_shell("owner")
    tm.start()

    owner = tm.get_client("owner")
    print("[OK] Shell instance ready")

    # === Setup Phase ===
    print("\n--- Setup Phase ---")

    # Sign up and login
    print("Signup and login...")
    owner.signup_or_login("owner", wait_p2p=False)  # Don't wait for P2P since no node
    print("  [OK] Logged in")

    # Create space with chat app
    print("Creating space with Group Chat app...")
    space = owner.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]
    pages = owner.list_pages(space_id)
    page_id = pages[0]["id"]
    print(f"  Space: {space_id[:12]}...")
    print(f"  Page: {page_id[:12]}...")

    # Open Group Chat app
    print("Opening Group Chat app...")
    owner.open_app(page_id, "Group Chat")
    time.sleep(2)  # Let app initialize

    # Set username for presence
    owner.eval('USERNAME = "test_owner"')
    owner.eval('my_name = USERNAME')
    print("  [OK] App opened")

    # === Test Phase ===
    print("\n--- Test: Presence Heartbeat ---")

    # Query initial presence
    print("Querying initial presence state...")
    time.sleep(2)  # Let initial presence write happen

    try:
        # Get our DID
        my_did = owner.eval('return scribe:my_did()')
        print(f"  My DID: {my_did[:20]}...")

        # Get presence data via the presence layer
        # The presence layer is named "<page_id>/presence"
        presence_data = owner.eval(f'''
            local layer = scribe:map("{page_id}/presence")
            local keys = layer:keys()
            local result = {{}}
            for _, k in ipairs(keys) do
                result[k] = layer:get(k)
            end
            return result
        ''')

        if not presence_data:
            print("  [ERROR] No presence data found!")
            return 1

        print(f"  Presence entries: {len(presence_data)}")

        # Find our entry
        our_entry = presence_data.get(my_did)
        if not our_entry:
            print(f"  [ERROR] Our DID not found in presence layer!")
            print(f"  Keys: {list(presence_data.keys())}")
            return 1

        initial_last_seen = our_entry.get("last_seen")
        print(f"  Initial last_seen: {initial_last_seen}")
        print(f"  Status: {our_entry.get('status')}")
        print(f"  Name: {our_entry.get('name')}")

    except Exception as e:
        print(f"  [ERROR] Failed to query presence: {e}")
        return 1

    # Wait for heartbeat (presence.lua uses 15s interval)
    print("\nWaiting 17 seconds for heartbeat timer...")
    for i in range(17):
        time.sleep(1)
        print(f"  {17-i}s remaining...", end="\r")
    print("  Done!              ")

    # Query updated presence
    print("\nQuerying updated presence state...")
    try:
        presence_data = owner.eval(f'''
            local layer = scribe:map("{page_id}/presence")
            local keys = layer:keys()
            local result = {{}}
            for _, k in ipairs(keys) do
                result[k] = layer:get(k)
            end
            return result
        ''')

        our_entry = presence_data.get(my_did)
        if not our_entry:
            print(f"  [ERROR] Our DID no longer in presence layer!")
            return 1

        updated_last_seen = our_entry.get("last_seen")
        print(f"  Updated last_seen: {updated_last_seen}")

        diff = updated_last_seen - initial_last_seen
        print(f"  Difference: {diff} seconds")

        if diff >= 15:
            print(f"\n  [PASS] Heartbeat updated last_seen correctly!")
            print(f"         Initial: {initial_last_seen}")
            print(f"         Updated: {updated_last_seen}")
            print(f"         Diff: {diff}s (expected >= 15s)")
        else:
            print(f"\n  [FAIL] Heartbeat did NOT update last_seen!")
            print(f"         Initial: {initial_last_seen}")
            print(f"         Updated: {updated_last_seen}")
            print(f"         Diff: {diff}s (expected >= 15s)")
            return 1

    except Exception as e:
        print(f"  [ERROR] Failed to query updated presence: {e}")
        return 1

    # === Summary ===
    print("\n" + "=" * 60)
    print("  Test Summary")
    print("=" * 60)
    print("  Presence heartbeat is working correctly!")
    print("  The Loro observer fires after map.insert() + CommitLayer.")
    print()
    print("  Attach to tmux: tmux attach -t presence_local_test")
    print("  Kill session: tmux kill-session -t presence_local_test")
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
