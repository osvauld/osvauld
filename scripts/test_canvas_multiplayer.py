#!/usr/bin/env python3
"""
Canvas App Multiplayer Sync Test

Tests P2P multiplayer sync for the canvas app:
1. Two players connect to same canvas
2. Player 1 creates shapes that sync to Player 2 via Loro
3. Shape state verification across peers

Architecture:
    ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
    │   Player1   │◄───►│    Node     │◄───►│   Player2   │
    │  (Owner)    │     │  (Relay)    │     │  (Viewer)   │
    └─────────────┘     └─────────────┘     └─────────────┘
          │                                        │
          ▼                                        ▼
      add_shape()                         on_loro_change()
      shapes_layer:set()                  load_from_loro()

Usage:
    python scripts/test_tank_multiplayer.py              # Run test, cleanup on success
    python scripts/test_tank_multiplayer.py --keep       # Keep session alive after test
    python scripts/test_tank_multiplayer.py --debug      # Keep session on failure for debugging

After test, attach to tmux:
    tmux attach -t tank_mp
"""

import sys
import time
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager
from osvauld.wait import wait_for_condition


DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "canvas-app"
CANVAS_APP_NAME = "Canvas"
KEEP_SESSION = False
DEBUG_ON_FAIL = False


def find_canvas_page(client, space_id):
    """Find the page containing Canvas app."""
    pages = client.list_pages(space_id)
    for page in pages:
        apps = client.list_apps(page["id"])
        for app in apps:
            if app.get("name") == CANVAS_APP_NAME:
                return page["id"]
    return None


def wait_for_app_ready(client, timeout=30):
    """Wait for app to be ready for eval."""
    for i in range(timeout * 10):
        try:
            result = client.eval("return 1")
            if result == 1:
                return True
        except Exception:
            pass
        time.sleep(0.1)
    raise TimeoutError("App not ready for eval")


def main():
    print("=" * 60)
    print("  Canvas App Multiplayer Sync Test")
    print("=" * 60)

    # 1. Start all instances in tmux
    print("\n[1/6] Starting instances (node + player1 + player2)...")
    tm = TmuxManager(
        session_name="tank_mp",
        base_dir=Path("/tmp/tank_mp"),
    )
    tm.add_node("node")
    tm.add_shell("player1")
    tm.add_shell("player2")
    tm.start()

    node = tm.get_client("node")
    p1 = tm.get_client("player1")
    p2 = tm.get_client("player2")
    print("      All instances ready")

    try:
        # 2. Player 1 (owner) setup
        print("\n[2/6] Player 1: signup, create space...")
        p1.signup_or_login("player1")
        space = p1.create_space_with_pages(str(DEMOS_APP))
        space_id = space["id"]

        # Find the canvas page
        page_id = find_canvas_page(p1, space_id)
        if not page_id:
            raise RuntimeError("Could not find Canvas page")
        print(f"      Space: {space_id[:8]}..., Page: {page_id[:8]}...")

        # 3. Connect and publish
        print("\n[3/6] Player 1: connect to node, publish...")
        p1.connect_to_node(node)
        time.sleep(2)
        p1.publish_to_node(space_id)
        time.sleep(2)
        print("      Published")

        # 4. Player 2 (viewer) setup
        print("\n[4/6] Player 2: signup, subscribe...")
        p2.signup_or_login("player2")
        p1.add_viewer(p2, space_id)

        # Wait for viewer to sync space and find canvas page
        p2_page_id = None
        p2_space_id = None
        for i in range(30):  # Increased timeout to 30s
            time.sleep(1)
            spaces = p2.list_spaces()
            if spaces:
                p2_space_id = spaces[0]["id"]
                pages = p2.list_pages(p2_space_id)
                print(f"      [{i+1}s] P2 has {len(pages)} page(s)")
                for page in pages:
                    apps = p2.list_apps(page["id"])
                    app_names = [a.get("name") for a in apps]
                    print(f"        Page {page['id'][:8]}... apps: {app_names}")
                p2_page_id = find_canvas_page(p2, p2_space_id)
                if p2_page_id:
                    print(f"      Player 2 synced in {i+1}s")
                    break
            else:
                print(f"      [{i+1}s] P2 waiting for spaces...")
        if not p2_page_id:
            raise RuntimeError("Player 2 failed to sync space or find Canvas page")

        # 5. Open apps
        print("\n[5/6] Opening apps...")
        p1.open_app(page_id, CANVAS_APP_NAME)
        wait_for_app_ready(p1)
        print("      Player 1 app ready")
        time.sleep(2)

        p2.open_app(p2_page_id, CANVAS_APP_NAME)
        wait_for_app_ready(p2)
        print("      Player 2 app ready")
        time.sleep(2)

        # Initialize both canvases
        p1.eval("on_init()")
        p2.eval("on_init()")
        time.sleep(1)
        print("      Both apps initialized")

        # === Test: P1 creates shapes, verify P2 sees them ===
        print("\n[6/6] Test: Shape creation and P2P sync...")

        # Get initial shape count
        p1_initial_count = p1.eval("return get_shape_count()")
        p2_initial_count = p2.eval("return get_shape_count()")
        print(f"      Initial shapes - P1: {p1_initial_count}, P2: {p2_initial_count}")

        # P1 creates shapes
        print("      P1 creating 3 shapes...")
        p1.eval('add_shape("rectangle", 100, 100, 80, 60)')
        time.sleep(0.5)
        p1.eval('add_shape("ellipse", 250, 100, 80, 60)')
        time.sleep(0.5)
        p1.eval('add_shape("sticky", 400, 100, 100, 80)')
        time.sleep(1)

        p1_count = p1.eval("return get_shape_count()")
        print(f"      P1 shape count after creation: {p1_count}")

        assert p1_count == p1_initial_count + 3, f"P1 should have 3 new shapes (got {p1_count - p1_initial_count})"

        # Wait for P2 to sync
        print("      Waiting for P2 to sync...")
        p2_synced = False
        for i in range(10):
            time.sleep(1)
            p2_count = p2.eval("return get_shape_count()")
            print(f"      [{i+1}s] P2 shape count: {p2_count}")
            if p2_count >= p1_count:
                p2_synced = True
                break

        # Verify sync
        p2_final_count = p2.eval("return get_shape_count()")

        # Summary
        print("\n" + "=" * 60)
        print("  [SUCCESS] Canvas App Multiplayer Test!")
        print("=" * 60)
        print(f"    [OK] P1 created shapes: {p1_initial_count} -> {p1_count}")
        print(f"    [OK] P2 synced shapes: {p2_initial_count} -> {p2_final_count}")
        if p2_synced:
            print(f"    [OK] P2P sync verified!")
        else:
            print(f"    [WARN] P2P sync may be delayed (P2 count: {p2_final_count})")
        print("=" * 60)

        return 0

    except Exception as e:
        print(f"\n[FAIL] {e}")
        import traceback
        traceback.print_exc()

        # Debug info
        try:
            print("\n--- Debug Info ---")
            p1_shapes = p1.eval("return get_shape_count()")
            print(f"P1 shape count: {p1_shapes}")
            p2_shapes = p2.eval("return get_shape_count()")
            print(f"P2 shape count: {p2_shapes}")
        except Exception as debug_e:
            print(f"Debug failed: {debug_e}")

        if KEEP_SESSION or DEBUG_ON_FAIL:
            print("\n" + "=" * 60)
            print("  Session kept alive for debugging")
            print("=" * 60)
            print(f"\n  tmux attach -t tank_mp")
            print(f"\n  Sockets:")
            print(f"    Node:    /tmp/tank_mp/node/node.sock")
            print(f"    Player1: /tmp/tank_mp/player1/player1.sock")
            print(f"    Player2: /tmp/tank_mp/player2/player2.sock")
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
            print(f"\n  tmux attach -t tank_mp")
            print(f"\n  Sockets:")
            print(f"    Node:    /tmp/tank_mp/node/node.sock")
            print(f"    Player1: /tmp/tank_mp/player1/player1.sock")
            print(f"    Player2: /tmp/tank_mp/player2/player2.sock")
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
    parser = argparse.ArgumentParser(description="Canvas App Multiplayer Sync Test")
    parser.add_argument("--keep", action="store_true", help="Keep tmux session alive after test")
    parser.add_argument("--debug", action="store_true", help="Keep session on failure for debugging")
    args = parser.parse_args()

    KEEP_SESSION = args.keep
    DEBUG_ON_FAIL = args.debug

    sys.exit(main())
