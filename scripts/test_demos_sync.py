#!/usr/bin/env python3
"""
Demo Apps Sync Integration Test

Tests the P2P sync for osvauld demo apps:
1. Snake Game: High scores sync across peers
2. Math Sim: Config changes sync across peers
3. Group Chat: Messages sync in real-time

Usage:
    python scripts/test_demos_sync.py              # Run all tests
    python scripts/test_demos_sync.py --app snake  # Run specific app test
    python scripts/test_demos_sync.py --keep       # Keep session alive after test
    python scripts/test_demos_sync.py --debug      # Keep session on failure

After test, attach to tmux:
    tmux attach -t demos_test
"""

import sys
import time
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from lib.tmux import TmuxManager


DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"
KEEP_SESSION = False
DEBUG_ON_FAIL = False


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


def setup_scenario(session_name: str, app_subdir: str, peer1_name: str = "Alice", peer2_name: str = "Bob"):
    """Setup a two-peer scenario for testing a demo app.

    Returns:
        Tuple of (tm, node, peer1, peer2, space_id, page_id, peer2_page_id, peer1_name, peer2_name)
    """
    print(f"\n[Setup] Starting instances for {app_subdir}...")

    tm = TmuxManager(
        session_name=session_name,
        base_dir=Path(f"/tmp/{session_name}"),
    )
    tm.add_node("node")
    tm.add_shell("peer1")
    tm.add_shell("peer2")
    tm.start()

    node = tm.get_client("node")
    peer1 = tm.get_client("peer1")
    peer2 = tm.get_client("peer2")
    print("      All instances ready")

    # Peer1 setup (owner)
    print(f"[Setup] {peer1_name}: signup, create space...")
    peer1.signup_or_login(peer1_name.lower())

    # Create space from main demos directory (has permit templates)
    # This imports all apps from subdirectories
    space = peer1.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]

    # Find the specific page for this app
    pages = peer1.list_pages(space_id)
    page_id = None
    app_name_map = {
        "snake-game": "Snake Game",
        "math-sim": "Math Simulation",
        "group-chat": "Group Chat",
        "landing": "Landing",
    }
    expected_app = app_name_map.get(app_subdir, app_subdir)

    print(f"      {peer1_name} has {len(pages)} pages, looking for '{expected_app}'")
    for page in pages:
        apps = peer1.list_apps(page["id"])
        app_names = [a.get("name") for a in apps]
        print(f"        Page {page['id'][:8]}... apps: {app_names}")
        for app in apps:
            if app.get("name") == expected_app:
                page_id = page["id"]
                break
        if page_id:
            break

    if not page_id:
        raise RuntimeError(f"Could not find page with app '{expected_app}'")
    print(f"      Space: {space_id[:8]}..., Page: {page_id[:8]}...")

    # Connect and publish
    print(f"[Setup] {peer1_name}: connect to node, publish...")
    peer1.connect_to_node(node)
    time.sleep(2)
    peer1.publish_to_node(space_id)
    time.sleep(2)
    print("      Published")

    # Peer2 setup (viewer)
    print(f"[Setup] {peer2_name}: signup, subscribe...")
    peer2.signup_or_login(peer2_name.lower())
    peer1.add_viewer(peer2, space_id)

    # Wait for peer2 to sync space and find the page
    # Note: Apps are synced later, so we just wait for the page to appear
    peer2_page_id = None
    for i in range(20):
        time.sleep(1)
        spaces = peer2.list_spaces()
        if spaces:
            peer2_pages = peer2.list_pages(spaces[0]["id"])
            if peer2_pages:
                # Just take the first page - apps sync separately
                peer2_page_id = peer2_pages[0]["id"]
                apps = peer2.list_apps(peer2_page_id)
                app_names = [a.get("name") for a in apps]
                print(f"      {peer2_name} synced in {i+1}s (apps: {app_names})")
                break
    if not peer2_page_id:
        raise RuntimeError(f"{peer2_name} failed to sync space")

    return tm, node, peer1, peer2, space_id, page_id, peer2_page_id, peer1_name, peer2_name


def test_snake_game():
    """Test Snake Game high score sync."""
    print("\n" + "=" * 60)
    print("  Snake Game Sync Test")
    print("=" * 60)

    tm, node, peer1, peer2, space_id, page_id, peer2_page_id, name1, name2 = setup_scenario(
        "snake_test", "snake-game", "Alice", "Bob"
    )

    try:
        # Open apps
        print("\n[1/5] Opening apps...")
        peer1.open_app(page_id, "Snake Game")
        time.sleep(2)
        peer2.open_app(peer2_page_id, "Snake Game")
        time.sleep(2)
        print("      Both apps opened")

        # Initialize both apps with username context
        print("\n[2/5] Initializing apps...")
        peer1.eval(f'USERNAME = "{name1}"')
        peer2.eval(f'USERNAME = "{name2}"')
        peer1.eval("on_init()")
        peer2.eval("on_init()")
        time.sleep(1)
        print("      Apps initialized")

        # Peer1 submits a high score
        print(f"\n[3/5] {name1}: submitting high score...")
        peer1.eval(f"""
            game = {{ score = 150 }}
            scores_layer = loro:get_or_create_layer(permit:page_id() .. "/scores", "list")
            scores_layer:push({{
                player = USERNAME or "{name1}",
                score = 150,
                timestamp = os.time(),
                date = os.date("%Y-%m-%d")
            }})
        """)
        time.sleep(1)

        peer1_scores = peer1.eval("return scores_layer:length()")
        print(f"      {name1} has {peer1_scores} score(s)")
        assert peer1_scores >= 1, f"{name1} should have at least 1 score"

        # Wait for sync to peer2
        print(f"\n[4/5] Waiting for sync to {name2}...")
        def check_peer2_scores():
            count = peer2.eval("""
                local layer = loro:get_layer(permit:page_id() .. "/scores", "list")
                return layer and layer:length() or 0
            """)
            return count >= 1
        wait_for(check_peer2_scores, timeout=15, desc=f"{name2} scores sync")

        peer2_scores = peer2.eval("""
            local layer = loro:get_layer(permit:page_id() .. "/scores", "list")
            return layer:length()
        """)
        print(f"      {name2} has {peer2_scores} score(s) - SYNC OK")

        # Verify score data
        print("\n[5/5] Verifying score data...")
        score_data = peer2.eval("""
            local layer = loro:get_layer(permit:page_id() .. "/scores", "list")
            return layer:get(0)
        """)
        print(f"      Score: player={score_data.get('player')}, score={score_data.get('score')}")
        assert score_data.get("score") == 150, "Score should be 150"
        assert score_data.get("player") == name1, f"Player should be {name1}"

        print("\n" + "=" * 60)
        print("  [SUCCESS] Snake Game Sync Test Passed!")
        print("=" * 60)
        return True

    except Exception as e:
        print(f"\n[FAIL] {e}")
        import traceback
        traceback.print_exc()
        return False

    finally:
        if not KEEP_SESSION and not DEBUG_ON_FAIL:
            tm.stop()


def test_math_sim():
    """Test Math Simulation config sync."""
    print("\n" + "=" * 60)
    print("  Math Simulation Sync Test")
    print("=" * 60)

    tm, node, peer1, peer2, space_id, page_id, peer2_page_id, name1, name2 = setup_scenario(
        "mathsim_test", "math-sim", "Charlie", "Diana"
    )

    try:
        # Open apps
        print("\n[1/5] Opening apps...")
        peer1.open_app(page_id, "Math Simulation")
        time.sleep(2)
        peer2.open_app(peer2_page_id, "Math Simulation")
        time.sleep(2)
        print("      Both apps opened")

        # Initialize both apps with username context
        print("\n[2/5] Initializing apps...")
        peer1.eval(f'USERNAME = "{name1}"')
        peer2.eval(f'USERNAME = "{name2}"')
        peer1.eval("on_init()")
        peer2.eval("on_init()")
        time.sleep(1)
        print("      Apps initialized")

        # Peer1 changes config
        print(f"\n[3/5] {name1}: changing simulation config...")
        peer1.eval("""
            config_layer = loro:get_or_create_layer(permit:page_id() .. "/sim_config", "map")
            config_layer:set("settings", {
                gravity = 1.5,
                bounce = 0.7,
                particle_count = 200
            })
        """)
        time.sleep(1)

        peer1_config = peer1.eval("return config_layer:get('settings')")
        print(f"      {name1} config: gravity={peer1_config.get('gravity')}")
        assert peer1_config.get("gravity") == 1.5, f"{name1} gravity should be 1.5"

        # Wait for sync to peer2
        print(f"\n[4/5] Waiting for sync to {name2}...")
        def check_peer2_config():
            config = peer2.eval("""
                local layer = loro:get_layer(permit:page_id() .. "/sim_config", "map")
                return layer and layer:get("settings")
            """)
            return config and config.get("gravity") == 1.5
        wait_for(check_peer2_config, timeout=15, desc=f"{name2} config sync")

        peer2_config = peer2.eval("""
            local layer = loro:get_layer(permit:page_id() .. "/sim_config", "map")
            return layer:get("settings")
        """)
        print(f"      {name2} config: gravity={peer2_config.get('gravity')} - SYNC OK")

        # Verify all settings
        print("\n[5/5] Verifying all settings...")
        assert peer2_config.get("gravity") == 1.5, "Gravity should be 1.5"
        assert peer2_config.get("bounce") == 0.7, "Bounce should be 0.7"
        assert peer2_config.get("particle_count") == 200, "Particle count should be 200"

        print("\n" + "=" * 60)
        print("  [SUCCESS] Math Simulation Sync Test Passed!")
        print("=" * 60)
        return True

    except Exception as e:
        print(f"\n[FAIL] {e}")
        import traceback
        traceback.print_exc()
        return False

    finally:
        if not KEEP_SESSION and not DEBUG_ON_FAIL:
            tm.stop()


def test_group_chat():
    """Test Group Chat message sync."""
    print("\n" + "=" * 60)
    print("  Group Chat Sync Test")
    print("=" * 60)

    tm, node, peer1, peer2, space_id, page_id, peer2_page_id, name1, name2 = setup_scenario(
        "chat_test", "group-chat", "Eve", "Frank"
    )

    try:
        # Open apps
        print("\n[1/6] Opening apps...")
        peer1.open_app(page_id, "Group Chat")
        time.sleep(2)
        peer2.open_app(peer2_page_id, "Group Chat")
        time.sleep(2)
        print("      Both apps opened")

        # Initialize both apps with username context
        print("\n[2/6] Initializing apps...")
        peer1.eval(f'USERNAME = "{name1}"')
        peer2.eval(f'USERNAME = "{name2}"')
        peer1.eval("on_init()")
        peer2.eval("on_init()")
        time.sleep(1)
        print("      Apps initialized")

        # Peer1 sends a message
        print(f"\n[3/6] {name1}: sending message...")
        peer1.eval(f"""
            messages_layer = loro:get_or_create_layer(permit:page_id() .. "/messages", "list")
            messages_layer:push({{
                id = "msg-001",
                sender_did = permit:my_did(),
                sender_name = USERNAME or "{name1}",
                text = "Hello from {name1}!",
                timestamp = os.time()
            }})
        """)
        time.sleep(1)

        peer1_msgs = peer1.eval("return messages_layer:length()")
        print(f"      {name1} has {peer1_msgs} message(s)")
        assert peer1_msgs >= 1, f"{name1} should have at least 1 message"

        # Wait for sync to peer2
        print(f"\n[4/6] Waiting for sync to {name2}...")
        def check_peer2_messages():
            count = peer2.eval("""
                local layer = loro:get_layer(permit:page_id() .. "/messages", "list")
                return layer and layer:length() or 0
            """)
            return count >= 1
        wait_for(check_peer2_messages, timeout=15, desc=f"{name2} messages sync")

        peer2_msgs = peer2.eval("""
            local layer = loro:get_layer(permit:page_id() .. "/messages", "list")
            return layer:length()
        """)
        print(f"      {name2} has {peer2_msgs} message(s) - SYNC OK")

        # Peer2 replies
        print(f"\n[5/6] {name2}: sending reply...")
        peer2.eval(f"""
            messages_layer = loro:get_or_create_layer(permit:page_id() .. "/messages", "list")
            messages_layer:push({{
                id = "msg-002",
                sender_did = permit:my_did(),
                sender_name = USERNAME or "{name2}",
                text = "Hello back from {name2}!",
                timestamp = os.time()
            }})
        """)
        time.sleep(1)

        # Wait for reply to sync back to peer1
        print(f"\n[6/6] Waiting for reply sync to {name1}...")
        def check_peer1_reply():
            count = peer1.eval("return messages_layer:length()")
            return count >= 2
        wait_for(check_peer1_reply, timeout=15, desc=f"{name1} reply sync")

        peer1_final = peer1.eval("return messages_layer:length()")
        print(f"      {name1} now has {peer1_final} message(s) - BIDIRECTIONAL SYNC OK")

        # Verify messages
        msg1 = peer1.eval("return messages_layer:get(0)")
        msg2 = peer1.eval("return messages_layer:get(1)")
        print(f"      Message 1: [{msg1.get('sender_name')}] {msg1.get('text')}")
        print(f"      Message 2: [{msg2.get('sender_name')}] {msg2.get('text')}")

        print("\n" + "=" * 60)
        print("  [SUCCESS] Group Chat Sync Test Passed!")
        print("=" * 60)
        return True

    except Exception as e:
        print(f"\n[FAIL] {e}")
        import traceback
        traceback.print_exc()
        return False

    finally:
        if not KEEP_SESSION and not DEBUG_ON_FAIL:
            tm.stop()


def main():
    global KEEP_SESSION, DEBUG_ON_FAIL

    parser = argparse.ArgumentParser(description="Demo Apps Sync Integration Test")
    parser.add_argument("--app", choices=["snake", "math", "chat", "all"], default="all",
                        help="Which app to test")
    parser.add_argument("--keep", action="store_true", help="Keep tmux session alive after test")
    parser.add_argument("--debug", action="store_true", help="Keep session on failure for debugging")
    args = parser.parse_args()

    KEEP_SESSION = args.keep
    DEBUG_ON_FAIL = args.debug

    results = {}

    if args.app in ["snake", "all"]:
        results["snake"] = test_snake_game()

    if args.app in ["math", "all"]:
        results["math"] = test_math_sim()

    if args.app in ["chat", "all"]:
        results["chat"] = test_group_chat()

    # Summary
    print("\n" + "=" * 60)
    print("  Test Summary")
    print("=" * 60)
    for name, passed in results.items():
        status = "[PASS]" if passed else "[FAIL]"
        print(f"    {status} {name}")
    print("=" * 60)

    all_passed = all(results.values())
    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())
