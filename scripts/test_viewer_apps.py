#!/usr/bin/env python3
"""
Test that viewer receives all apps after connecting.

Tests:
1. Relay node starts
2. Owner creates space with all demo apps
3. Owner publishes to relay
4. Viewer connects via viewer link (includes space_id in permit)
5. Verify viewer sees all apps
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

def main():
    print("=" * 60)
    print("Testing Viewer App Subscription")
    print("=" * 60)

    # Clean start
    test_dir = Path("/tmp/test-viewer-apps")

    # Start tmux with relay + owner + viewer
    print("\n1. Starting instances (relay + owner + viewer)...")
    tm = TmuxManager(
        session_name="test_viewer",
        base_dir=test_dir,
        passphrase="test",
    )
    tm.add_node("relay")
    tm.add_shell("owner")
    tm.add_shell("viewer")

    try:
        tm.start(fresh=True, timeout=60)
    except Exception as e:
        print(f"ERROR: Failed to start: {e}")
        return 1

    relay = tm.get_client("relay")
    owner = tm.get_client("owner")
    viewer = tm.get_client("viewer")
    print("   Instances ready")

    # Get relay address
    relay_addr = relay.get_connection_string()
    print(f"   Relay: {relay_addr[:50]}...")

    # Owner setup
    print("\n2. Owner: signup and create space...")
    owner.signup_or_login("test_owner", "test")

    space = owner.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]
    print(f"   Space created: {space_id[:16]}...")

    # List owner's apps
    pages = owner.list_pages(space_id)
    page_id = pages[0]["id"]
    owner_apps = owner.list_apps(page_id)
    owner_app_names = sorted([a.get("name") for a in owner_apps])
    print(f"   Owner apps: {owner_app_names}")

    # Owner connects to relay and publishes
    print("\n3. Owner: connecting to relay and publishing...")
    owner.connect_to_node(relay)
    time.sleep(2)
    owner.publish_to_node(space_id)
    time.sleep(2)
    print("   Published to relay")

    # Get viewer link (includes space_id in permit)
    print("\n4. Getting viewer link...")
    viewer_link = owner.get_viewer_link(space_id)
    print(f"   Viewer link: {viewer_link[:50]}...")

    # Viewer setup
    print("\n5. Viewer: signup and connect via viewer link...")
    viewer.signup_or_login("test_viewer", "test")

    print("   Connecting with viewer link...")
    try:
        result = viewer.add_website(viewer_link)
        print(f"   Connection result: {result}")
    except Exception as e:
        print(f"   Connection error: {e}")

    # Wait for sync
    print("\n6. Waiting for sync...")
    for i in range(10):
        time.sleep(2)
        viewer_spaces = viewer.list_spaces()
        if viewer_spaces:
            print(f"   Sync complete after {(i+1)*2}s")
            break
        print(f"   Waiting... ({(i+1)*2}s)")
    else:
        print("   Timeout waiting for sync")

    # Check viewer's spaces
    print("\n7. Checking viewer's spaces and apps...")
    viewer_spaces = viewer.list_spaces()
    print(f"   Viewer has {len(viewer_spaces)} space(s)")

    result_code = 1
    if viewer_spaces:
        viewer_space_id = viewer_spaces[0].get("id")
        viewer_pages = viewer.list_pages(viewer_space_id)
        print(f"   Viewer has {len(viewer_pages)} page(s)")

        if viewer_pages:
            viewer_page_id = viewer_pages[0].get("id")
            viewer_apps = viewer.list_apps(viewer_page_id)
            viewer_app_names = sorted([a.get("name") for a in viewer_apps])
            print(f"   Viewer apps: {viewer_app_names}")

            # Compare
            print("\n" + "=" * 60)
            print("RESULTS")
            print("=" * 60)
            print(f"Owner apps:  {owner_app_names}")
            print(f"Viewer apps: {viewer_app_names}")

            if owner_app_names == viewer_app_names:
                print("\n✓ SUCCESS: Viewer has all apps!")
                result_code = 0
            else:
                missing = set(owner_app_names) - set(viewer_app_names)
                extra = set(viewer_app_names) - set(owner_app_names)
                print(f"\n✗ MISMATCH:")
                if missing:
                    print(f"  Missing: {missing}")
                if extra:
                    print(f"  Extra: {extra}")
        else:
            print("   ERROR: Viewer has no pages")
    else:
        print("   ERROR: Viewer has no spaces")

    print("\nTmux session: tmux attach -t test_viewer")
    print("To kill: tmux kill-session -t test_viewer")

    return result_code


if __name__ == "__main__":
    sys.exit(main())
