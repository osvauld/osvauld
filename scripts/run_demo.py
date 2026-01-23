#!/usr/bin/env python3
"""
Run a demo app interactively.

Usage:
    python scripts/run_demo.py guide      # Run Sthalam Guide
    python scripts/run_demo.py snake      # Run snake game
    python scripts/run_demo.py math       # Run math simulation
    python scripts/run_demo.py chat       # Run group chat

After starting, attach to tmux:
    tmux attach -t demo
"""

import sys
import time
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from lib.tmux import TmuxManager


DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

APP_MAP = {
    "snake": ("snake-game", "Snake Game"),
    "math": ("math-sim", "Math Simulation"),
    "chat": ("group-chat", "Group Chat"),
    "guide": ("guide", "Sthalam Guide"),
}


def main():
    parser = argparse.ArgumentParser(description="Run a demo app interactively")
    parser.add_argument("app", choices=list(APP_MAP.keys()), help="Which app to run")
    args = parser.parse_args()

    app_subdir, app_name = APP_MAP[args.app]

    print(f"Starting {app_name}...")

    # Start tmux session with one shell
    tm = TmuxManager(
        session_name="demo",
        base_dir=Path("/tmp/demo"),
    )
    tm.add_shell("user")
    tm.start()

    client = tm.get_client("user")
    print("  Instance ready")

    # Signup
    print("  Signing up as 'demo_user'...")
    client.signup_or_login("demo_user")

    # Create space from demos directory
    print("  Creating space...")
    space = client.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]

    # List pages and find the app
    pages = client.list_pages(space_id)
    print(f"  Space has {len(pages)} page(s)")

    page_id = None
    for page in pages:
        apps = client.list_apps(page["id"])
        app_names = [a.get("name") for a in apps]
        print(f"    Page {page['id'][:8]}... apps: {app_names}")
        if app_name in app_names:
            page_id = page["id"]

    if not page_id:
        print(f"  ERROR: Could not find app '{app_name}'")
        print("  Keeping session alive for debugging. Attach with: tmux attach -t demo")
        return 1

    # Open the app
    print(f"  Opening {app_name}...")
    result = client.open_app(page_id, app_name)
    print(f"    Result: {result}")

    # Wait for app to be ready
    print("  Waiting for app to load...")
    for i in range(10):
        time.sleep(1)
        try:
            client.eval("return 1")
            print(f"    App ready after {i+1}s")
            break
        except Exception as e:
            if i == 9:
                print(f"    ERROR: App not ready after 10s: {e}")
                return 1

    # Initialize
    print("  Initializing app...")
    client.eval('USERNAME = "DemoUser"')
    client.eval("on_init()")

    print(f"\n{app_name} is now running!")
    print("Attach to tmux session: tmux attach -t demo")
    print("Press Ctrl+C to exit (session will stay alive)")

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nSession still running. Attach with: tmux attach -t demo")
        print("To kill: tmux kill-session -t demo")


if __name__ == "__main__":
    sys.exit(main() or 0)
