#!/usr/bin/env python3
"""
Tank Raylib Multiplayer Demo

Launches 3 players connected via P2P, each opening Tank Raylib.
Each player gets a raylib window; moving in one window shows the tank
in other windows. Verification is visual (raylib VMs are not eval-accessible).

Usage:
    python scripts/test_tank_sync.py              # Launch and keep alive for play
    python scripts/test_tank_sync.py --no-keep    # Launch, wait briefly, then cleanup

After launch, attach to tmux:
    tmux attach -t tank_test
"""

import sys
import time
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager


DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"


def find_tank_raylib_page(client, space_id):
    """Find the page containing the Tank Raylib app."""
    pages = client.list_pages(space_id)
    for page in pages:
        apps = client.list_apps(page["id"])
        for app in apps:
            if app.get("name") == "Tank Raylib":
                return page["id"]
    return None


def main():
    print("=" * 60)
    print("  Tank Raylib Multiplayer Demo")
    print("=" * 60)

    # 1. Start all instances in tmux
    print("\n[1/6] Starting instances (node + player1 + player2 + player3)...")
    tm = TmuxManager(
        session_name="tank_test",
        base_dir=Path("/tmp/tank_test"),
    )
    tm.add_node("node")
    tm.add_shell("player1")
    tm.add_shell("player2")
    tm.add_shell("player3")
    tm.start()

    node = tm.get_client("node")
    player1 = tm.get_client("player1")
    player2 = tm.get_client("player2")
    player3 = tm.get_client("player3")
    print("      All instances ready (player1, player2, player3)")

    try:
        # 2. Player 1 setup
        print("\n[2/6] Player 1: signup, create space from osvauld-demos...")
        player1.signup_or_login("player1")
        space = player1.create_space_with_pages(str(DEMOS_APP))
        space_id = space["id"]
        print(f"      Space: {space_id[:8]}...")

        # Find the tank raylib page
        page_id = find_tank_raylib_page(player1, space_id)
        if not page_id:
            raise RuntimeError("Could not find Tank Raylib app in created space")
        print(f"      Tank Raylib page: {page_id[:8]}...")

        # 3. Connect and publish
        print("\n[3/6] Player 1: connect to node, publish...")
        player1.connect_to_node(node)
        time.sleep(2)
        player1.publish_to_node(space_id)
        time.sleep(2)
        print("      Published")

        # 4. Player 2 setup
        print("\n[4/6] Player 2 + Player 3: signup, subscribe...")
        player2.signup_or_login("player2")
        player1.add_viewer(player2, space_id)

        # Wait for player2 to sync
        player2_page_id = None
        for i in range(15):
            time.sleep(1)
            spaces = player2.list_spaces()
            if spaces:
                player2_page_id = find_tank_raylib_page(player2, spaces[0]["id"])
                if player2_page_id:
                    print(f"      Player 2 synced in {i+1}s")
                    break

        if not player2_page_id:
            raise RuntimeError("Player 2 failed to sync Tank Raylib")

        # Player 3 setup
        player3.signup_or_login("player3")
        player1.add_viewer(player3, space_id)

        player3_page_id = None
        for i in range(15):
            time.sleep(1)
            spaces = player3.list_spaces()
            if spaces:
                player3_page_id = find_tank_raylib_page(player3, spaces[0]["id"])
                if player3_page_id:
                    print(f"      Player 3 synced in {i+1}s")
                    break

        if not player3_page_id:
            raise RuntimeError("Player 3 failed to sync Tank Raylib")

        # 5. Open Tank Raylib apps (each opens a raylib window)
        print("\n[5/6] Opening Tank Raylib apps...")
        player1.open_app(page_id, "Tank Raylib")
        time.sleep(2)
        print("      Player 1 window opened")

        player2.open_app(player2_page_id, "Tank Raylib")
        time.sleep(2)
        print("      Player 2 window opened")

        player3.open_app(player3_page_id, "Tank Raylib")
        time.sleep(2)
        print("      Player 3 window opened")

        # 6. Summary
        print("\n[6/6] All windows launched!")
        print("\n" + "=" * 60)
        print("  Tank Raylib Multiplayer - Ready")
        print("=" * 60)
        print("\n  3 raylib windows should be visible.")
        print("  Move with WASD/arrows, shoot with SPACE.")
        print("  Each player's tank should appear in other windows.")
        print("\n  Verify:")
        print("    - Each window shows a green local tank")
        print("    - Remote tanks appear in different colors")
        print("    - Player names shown above each tank")
        print("    - HUD shows player count in top-right")

        return 0

    except Exception as e:
        print(f"\n[FAIL] {e}")
        import traceback
        traceback.print_exc()
        return 1

    finally:
        if KEEP_SESSION:
            print("\n" + "=" * 60)
            print("  Session kept alive for play")
            print("=" * 60)
            print(f"\n  tmux attach -t tank_test")
            print(f"\n  Sockets:")
            print(f"    Node:    /tmp/tank_test/node/node.sock")
            print(f"    Player1: /tmp/tank_test/player1/player1.sock")
            print(f"    Player2: /tmp/tank_test/player2/player2.sock")
            print(f"    Player3: /tmp/tank_test/player3/player3.sock")
            print("\n  Press Ctrl+C to stop and cleanup")
            try:
                import signal
                signal.pause()
            except KeyboardInterrupt:
                pass
            tm.stop()
        else:
            print("\nCleaning up...")
            tm.stop()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Tank Raylib Multiplayer Demo")
    parser.add_argument("--no-keep", action="store_true", help="Cleanup after launch instead of keeping alive")
    args = parser.parse_args()

    KEEP_SESSION = not args.no_keep

    sys.exit(main())
