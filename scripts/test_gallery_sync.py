#!/usr/bin/env python3
"""
Photo Gallery Sync Integration Test

Tests the full gallery P2P flow:
1. Owner adds photos -> syncs to viewer
2. Viewer sees photos
3. Owner adds more photos -> syncs to viewer

Usage:
    python scripts/test_gallery_sync.py              # Run test, cleanup on success
    python scripts/test_gallery_sync.py --keep       # Keep session alive after test
    python scripts/test_gallery_sync.py --debug      # Keep session on failure for debugging

After test, attach to tmux:
    tmux attach -t gallery_test
"""

import sys
import time
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager


GALLERY_APP = Path(__file__).parent.parent / "sample_apps" / "photo-gallery"
KEEP_SESSION = False  # Set by args
DEBUG_ON_FAIL = False  # Set by args


def wait_for(condition_fn, timeout=30, interval=1, desc="condition"):
    """Wait for a condition to be true."""
    for i in range(int(timeout / interval)):
        try:
            result = condition_fn()
            if result:
                return result
        except Exception:
            pass
        time.sleep(interval)
    raise TimeoutError(f"Timeout waiting for {desc}")


def wait_for_app_ready(client, timeout=30):
    """Wait for app to be ready for eval."""
    for i in range(timeout):
        try:
            result = client.eval("return 1")
            if result == 1:
                return True
        except Exception:
            pass
        time.sleep(1)
    raise TimeoutError("App not ready for eval")


def main():
    print("=" * 60)
    print("  Photo Gallery Sync Integration Test")
    print("=" * 60)

    # 1. Start all instances in tmux
    print("\n[1/7] Starting instances (node + owner + viewer)...")
    tm = TmuxManager(
        session_name="gallery_test",
        base_dir=Path("/tmp/gallery_test"),
    )
    tm.add_node("node")
    tm.add_shell("owner")
    tm.add_shell("viewer")
    tm.start()

    node = tm.get_client("node")
    owner = tm.get_client("owner")
    viewer = tm.get_client("viewer")
    print("      All instances ready")

    try:
        # 2. Owner setup
        print("\n[2/7] Owner: signup, create space...")
        owner.signup_or_login("owner")
        space = owner.create_space_with_pages(str(GALLERY_APP))
        space_id = space["id"]
        page_id = space["pages"][0]["page_id"]
        print(f"      Space: {space_id[:8]}..., Page: {page_id[:8]}...")

        # 3. Connect and publish
        print("\n[3/7] Owner: connect to node, publish...")
        owner.connect_to_node(node)
        time.sleep(2)
        owner.publish_to_node(space_id)
        time.sleep(2)
        print("      Published")

        # 4. Viewer setup
        print("\n[4/7] Viewer: signup, subscribe...")
        viewer.signup_or_login("viewer")
        owner.add_viewer(viewer, space_id)

        # Wait for viewer to sync space
        viewer_page_id = None
        for i in range(15):
            time.sleep(1)
            spaces = viewer.list_spaces()
            if spaces:
                pages = viewer.list_pages(spaces[0]["id"])
                if pages:
                    viewer_page_id = pages[0]["id"]
                    print(f"      Viewer synced in {i+1}s")
                    break
        if not viewer_page_id:
            raise RuntimeError("Viewer failed to sync space")

        # 5. Open apps
        print("\n[5/7] Opening apps...")
        owner.open_app(page_id, "Gallery")
        wait_for_app_ready(owner)
        print("      Owner app ready")
        time.sleep(2)
        viewer.open_app(viewer_page_id, "Gallery")
        wait_for_app_ready(viewer)
        print("      Both apps opened")
        time.sleep(2)

        # 6. Owner adds photos
        print("\n[6/7] Owner: adding photos...")
        owner.eval('add_photo_for_test("hash-001", "vacation.jpg", "image/jpeg", 2048)')
        time.sleep(1)
        owner.eval('add_photo_for_test("hash-002", "birthday.png", "image/png", 3072)')
        time.sleep(1)
        owner.eval('add_photo_for_test("hash-003", "sunset.jpg", "image/jpeg", 1536)')
        time.sleep(2)

        owner_photo_count = owner.eval("return get_photo_count()")
        print(f"      Owner has {owner_photo_count} photo(s)")
        assert owner_photo_count == 3, f"Expected 3 photos, got {owner_photo_count}"

        # 7. Verify photos sync to viewer
        print("\n[7/7] Verifying photo sync to viewer...")
        def check_viewer_photos():
            count = viewer.eval("return get_photo_count()")
            return count >= 3
        wait_for(check_viewer_photos, timeout=15, desc="viewer photo sync")

        viewer_photo_count = viewer.eval("return get_photo_count()")
        print(f"      Viewer has {viewer_photo_count} photo(s) - SYNC OK")

        # Verify photo details
        viewer_photos = viewer.eval("return get_photos()")
        if viewer_photos:
            print(f"      Photo filenames: {[p.get('filename') for p in viewer_photos]}")

        # Summary
        print("\n" + "=" * 60)
        print("  [SUCCESS] Photo Gallery Sync Flow!")
        print("=" * 60)
        print(f"    [OK] Photo sync: owner ({owner_photo_count}) -> viewer ({viewer_photo_count})")
        print("=" * 60)

        return 0

    except Exception as e:
        print(f"\n[FAIL] {e}")
        import traceback
        traceback.print_exc()

        # Debug: print state on failure
        try:
            print("\n--- Debug Info ---")
            owner_photos = owner.eval("return get_photo_count()")
            print(f"Owner photos: {owner_photos}")
            viewer_photos = viewer.eval("return get_photo_count()")
            print(f"Viewer photos: {viewer_photos}")
        except:
            pass

        if KEEP_SESSION or DEBUG_ON_FAIL:
            print("\n" + "=" * 60)
            print("  Session kept alive for debugging")
            print("=" * 60)
            print(f"\n  tmux attach -t gallery_test")
            print(f"\n  Sockets:")
            print(f"    Node:   /tmp/gallery_test/node/node.sock")
            print(f"    Owner:  /tmp/gallery_test/owner/owner.sock")
            print(f"    Viewer: /tmp/gallery_test/viewer/viewer.sock")
            print("\n  Press Ctrl+C to stop and cleanup")
            try:
                import signal
                signal.pause()
            except KeyboardInterrupt:
                pass
            tm.stop()
        else:
            tm.stop()
        return 1

    finally:
        if KEEP_SESSION:
            print("\n" + "=" * 60)
            print("  Session kept alive (--keep flag)")
            print("=" * 60)
            print(f"\n  tmux attach -t gallery_test")
            print(f"\n  Sockets:")
            print(f"    Node:   /tmp/gallery_test/node/node.sock")
            print(f"    Owner:  /tmp/gallery_test/owner/owner.sock")
            print(f"    Viewer: /tmp/gallery_test/viewer/viewer.sock")
            print("\n  Press Ctrl+C to stop and cleanup")
            try:
                import signal
                signal.pause()
            except KeyboardInterrupt:
                pass
            tm.stop()
        elif not DEBUG_ON_FAIL:
            print("\nCleaning up...")
            tm.stop()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Photo Gallery Sync Integration Test")
    parser.add_argument("--keep", action="store_true", help="Keep tmux session alive after test")
    parser.add_argument("--debug", action="store_true", help="Keep session on failure for debugging")
    args = parser.parse_args()

    KEEP_SESSION = args.keep
    DEBUG_ON_FAIL = args.debug

    sys.exit(main())
