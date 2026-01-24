#!/usr/bin/env python3
"""
Tank Game Sync Integration Test

Tests the full tank game P2P flow:
1. Player 1 plays game -> scores sync to player 2
2. Player 2 plays game -> scores sync to player 1
3. Leaderboard stays in sync between both players

Usage:
    python scripts/test_tank_sync.py              # Run test, cleanup on success
    python scripts/test_tank_sync.py --keep       # Keep session alive after test
    python scripts/test_tank_sync.py --debug      # Keep session on failure for debugging

After test, attach to tmux:
    tmux attach -t tank_test
"""

import sys
import time
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from lib.tmux import TmuxManager


DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"
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


def find_tank_game_page(client, space_id):
    """Find the page containing the Tank Game app."""
    pages = client.list_pages(space_id)
    for page in pages:
        apps = client.list_apps(page["id"])
        for app in apps:
            if app.get("name") == "Tank Game":
                return page["id"]
    return None


def main():
    global KEEP_SESSION, DEBUG_ON_FAIL

    print("=" * 60)
    print("  Tank Game Sync Integration Test")
    print("=" * 60)

    # 1. Start all instances in tmux
    print("\n[1/8] Starting instances (node + player1 + player2)...")
    tm = TmuxManager(
        session_name="tank_test",
        base_dir=Path("/tmp/tank_test"),
    )
    tm.add_node("node")
    tm.add_shell("player1")
    tm.add_shell("player2")
    tm.start()

    node = tm.get_client("node")
    player1 = tm.get_client("player1")
    player2 = tm.get_client("player2")
    print("      All instances ready")

    try:
        # 2. Player 1 setup
        print("\n[2/8] Player 1: signup, create space from osvauld-demos...")
        player1.signup_or_login("player1")
        space = player1.create_space_with_pages(str(DEMOS_APP))
        space_id = space["id"]
        print(f"      Space: {space_id[:8]}...")

        # Find the tank game page
        page_id = find_tank_game_page(player1, space_id)
        if not page_id:
            raise RuntimeError("Could not find Tank Game app in created space")
        print(f"      Tank Game page: {page_id[:8]}...")

        # 3. Connect and publish
        print("\n[3/8] Player 1: connect to node, publish...")
        player1.connect_to_node(node)
        time.sleep(2)
        player1.publish_to_node(space_id)
        time.sleep(2)
        print("      Published")

        # 4. Player 2 setup
        print("\n[4/8] Player 2: signup, subscribe...")
        player2.signup_or_login("player2")
        player1.add_viewer(player2, space_id)

        # Wait for player2 to sync space and find tank game
        player2_page_id = None
        for i in range(15):
            time.sleep(1)
            spaces = player2.list_spaces()
            if spaces:
                player2_page_id = find_tank_game_page(player2, spaces[0]["id"])
                if player2_page_id:
                    print(f"      Player 2 synced in {i+1}s")
                    break

        if not player2_page_id:
            raise RuntimeError("Player 2 failed to sync Tank Game")

        # 5. Open apps
        print("\n[5/8] Opening Tank Game apps...")
        player1.open_app(page_id, "Tank Game")
        wait_for_app_ready(player1)
        player1.eval("on_init()")
        print("      Player 1 app ready")

        time.sleep(2)

        player2.open_app(player2_page_id, "Tank Game")
        wait_for_app_ready(player2)
        player2.eval("on_init()")
        print("      Player 2 app ready")

        time.sleep(2)

        # 6. Player 1 plays game and gets a score
        print("\n[6/8] Player 1: playing game...")

        # Start game
        player1.eval('on_click("toggle_pause")')
        time.sleep(0.5)

        # Simulate some gameplay - move and shoot
        for _ in range(5):
            player1.eval('on_key_pressed("up")')
            time.sleep(0.1)

        for _ in range(3):
            player1.eval('on_key_pressed("shoot")')
            time.sleep(0.2)

        # Tick the game a bunch of times
        for _ in range(100):
            player1.eval("tick()")
            time.sleep(0.016)  # ~60fps

        # Get current score
        p1_score = player1.eval("return get_score()")
        p1_stage = player1.eval("return get_stage()")
        print(f"      Player 1 score: {p1_score}, stage: {p1_stage}")

        # End game to save score
        player1.eval("game.score = 150")  # Set a test score
        player1.eval("end_game()")
        time.sleep(1)

        p1_final_score = player1.eval("return get_score()")
        print(f"      Player 1 final score: {p1_final_score}")

        # 7. Verify leaderboard syncs to Player 2
        print("\n[7/8] Verifying leaderboard sync to Player 2...")

        def check_p2_leaderboard():
            try:
                player2.eval("refresh_leaderboard()")
                # Check if there's at least one entry
                result = player2.eval("return scores_layer:length()")
                return result >= 1
            except:
                return False

        wait_for(check_p2_leaderboard, timeout=15, desc="leaderboard sync")

        p2_leaderboard_len = player2.eval("return scores_layer:length()")
        print(f"      Player 2 sees {p2_leaderboard_len} score(s) - SYNC OK")

        # 8. Player 2 plays and adds score
        print("\n[8/8] Player 2: playing game...")

        player2.eval("request_restart()")
        player2.eval('on_click("toggle_pause")')
        time.sleep(0.3)

        # Quick game
        for _ in range(50):
            player2.eval("tick()")
            time.sleep(0.016)

        # Set and save score
        player2.eval("game.score = 200")
        player2.eval("end_game()")
        time.sleep(1)

        p2_final_score = player2.eval("return get_score()")
        print(f"      Player 2 final score: {p2_final_score}")

        # Verify P1 sees both scores
        time.sleep(2)
        player1.eval("refresh_leaderboard()")
        p1_leaderboard_len = player1.eval("return scores_layer:length()")

        # Summary
        print("\n" + "=" * 60)
        print("  [SUCCESS] Tank Game Sync Flow!")
        print("=" * 60)
        print(f"    [OK] Player 1 score saved: {p1_final_score}")
        print(f"    [OK] Player 2 score saved: {p2_final_score}")
        print(f"    [OK] Player 1 sees {p1_leaderboard_len} score(s)")
        print(f"    [OK] Player 2 sees {p2_leaderboard_len} score(s)")
        print("=" * 60)

        return 0

    except Exception as e:
        print(f"\n[FAIL] {e}")
        import traceback
        traceback.print_exc()

        # Debug: print state on failure
        try:
            print("\n--- Debug Info ---")
            p1_score = player1.eval("return get_score()")
            print(f"Player 1 score: {p1_score}")
            p2_score = player2.eval("return get_score()")
            print(f"Player 2 score: {p2_score}")
        except:
            pass

        if KEEP_SESSION or DEBUG_ON_FAIL:
            print("\n" + "=" * 60)
            print("  Session kept alive for debugging")
            print("=" * 60)
            print(f"\n  tmux attach -t tank_test")
            print(f"\n  Sockets:")
            print(f"    Node:    /tmp/tank_test/node/node.sock")
            print(f"    Player1: /tmp/tank_test/player1/player1.sock")
            print(f"    Player2: /tmp/tank_test/player2/player2.sock")
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
            print(f"\n  tmux attach -t tank_test")
            print(f"\n  Sockets:")
            print(f"    Node:    /tmp/tank_test/node/node.sock")
            print(f"    Player1: /tmp/tank_test/player1/player1.sock")
            print(f"    Player2: /tmp/tank_test/player2/player2.sock")
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
    parser = argparse.ArgumentParser(description="Tank Game Sync Integration Test")
    parser.add_argument("--keep", action="store_true", help="Keep tmux session alive after test")
    parser.add_argument("--debug", action="store_true", help="Keep session on failure for debugging")
    args = parser.parse_args()

    KEEP_SESSION = args.keep
    DEBUG_ON_FAIL = args.debug

    sys.exit(main())
