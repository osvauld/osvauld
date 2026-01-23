#!/usr/bin/env python3
"""
Test that app reload syncs to viewer.

Tests:
1. Owner creates space, viewer connects
2. VIEWER opens app first (before any changes)
3. Owner modifies and refreshes the app
4. Wait for viewer's app to auto-reload
5. Check if viewer sees updated code
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from lib.tmux import TmuxManager

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

def main():
    print("=" * 60)
    print("Testing App Reload Sync")
    print("=" * 60)

    test_dir = Path("/tmp/test-reload-sync")

    # Start tmux with relay + owner + viewer
    print("\n1. Starting instances...")
    tm = TmuxManager(
        session_name="test_reload",
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

    # Setup owner
    print("\n2. Owner: creating space...")
    owner.signup_or_login("test_owner", "test")
    space = owner.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]
    pages = owner.list_pages(space_id)
    page_id = pages[0]["id"]
    print(f"   Space: {space_id[:16]}...")

    # Owner publishes to relay
    print("\n3. Owner: publishing to relay...")
    owner.connect_to_node(relay)
    time.sleep(2)
    owner.publish_to_node(space_id)
    time.sleep(2)
    viewer_link = owner.get_viewer_link(space_id)
    print("   Published")

    # Viewer connects
    print("\n4. Viewer: connecting...")
    viewer.signup_or_login("test_viewer", "test")
    viewer.add_website(viewer_link)

    # Wait for sync
    for i in range(10):
        time.sleep(2)
        viewer_spaces = viewer.list_spaces()
        if viewer_spaces:
            break

    viewer_space_id = viewer_spaces[0]["id"]
    viewer_pages = viewer.list_pages(viewer_space_id)
    viewer_page_id = viewer_pages[0]["id"]
    print("   Connected and synced")

    # VIEWER opens app FIRST (before any modifications)
    print("\n5. Viewer: opening Sthalam Guide FIRST...")
    viewer.open_app(viewer_page_id, "Sthalam Guide")
    time.sleep(3)

    # Check initial state
    try:
        viewer.eval('USERNAME = "TestViewer"')
        viewer.eval("on_init()")
        viewer_tab_before = viewer.eval('return ui:get("active_tab")')
        print(f"   Viewer active_tab BEFORE: {viewer_tab_before}")
    except Exception as e:
        print(f"   Error getting initial state: {e}")
        viewer_tab_before = None

    # Modify the guide app - change initial tab
    print("\n6. Modifying guide app.lua (active_tab: 0 -> 2)...")
    guide_lua = DEMOS_APP / "guide" / "app.lua"
    original_content = guide_lua.read_text()

    modified_content = original_content.replace(
        'ui:set("active_tab", 0)',
        'ui:set("active_tab", 2)  -- MODIFIED BY TEST'
    )
    guide_lua.write_text(modified_content)
    print("   Modified app.lua")

    # Owner opens and refreshes the app
    print("\n7. Owner: opening and refreshing app...")
    owner.open_app(page_id, "Sthalam Guide")
    time.sleep(2)

    result = owner.refresh_app("Sthalam Guide", str(DEMOS_APP / "guide"))
    print(f"   Refresh result: {result}")

    # Wait for sync to viewer and auto-reload
    print("\n8. Waiting for viewer to auto-reload (10s)...")
    time.sleep(10)

    # Check viewer's state after reload
    print("\n9. Checking viewer after reload...")
    try:
        viewer_tab_after = viewer.eval('return ui:get("active_tab")')
        print(f"   Viewer active_tab AFTER: {viewer_tab_after}")
    except Exception as e:
        print(f"   Error getting state: {e}")
        viewer_tab_after = None

    # Restore original file
    print("\n10. Restoring original app.lua...")
    guide_lua.write_text(original_content)
    print("   Restored")

    # Results
    print("\n" + "=" * 60)
    print("RESULTS")
    print("=" * 60)
    print(f"Viewer active_tab BEFORE refresh: {viewer_tab_before}")
    print(f"Viewer active_tab AFTER refresh:  {viewer_tab_after}")

    if viewer_tab_after == 2 or viewer_tab_after == 2.0:
        print("\n✓ SUCCESS: App reload synced to viewer!")
        result_code = 0
    elif viewer_tab_before != viewer_tab_after:
        print(f"\n~ PARTIAL: Value changed ({viewer_tab_before} -> {viewer_tab_after})")
        result_code = 1
    else:
        print("\n✗ FAILED: Viewer did not receive updated app")
        result_code = 1

    print("\nTmux session: tmux attach -t test_reload")
    print("To kill: tmux kill-session -t test_reload")

    return result_code


if __name__ == "__main__":
    sys.exit(main())
