#!/usr/bin/env python3
"""
Production owner node for Osvauld demos.

Runs a kunki relay node + owner shell with all demo apps published.
Designed to be run as a systemd service with auto-restart.

Usage:
    python scripts/run_production.py

    # Or with systemd (see scripts/osvauld-production.service)
    sudo systemctl start osvauld-production

Environment Variables:
    OSVAULD_DATA_DIR    - Data directory (default: /var/lib/osvauld)
    OSVAULD_USERNAME    - Owner username (default: production)
    OSVAULD_PASSPHRASE  - Passphrase (default: production)
"""

import os
import sys
import time
import signal
import json
from pathlib import Path
from datetime import datetime

# Unbuffered output for journald
sys.stdout.reconfigure(line_buffering=True)
sys.stderr.reconfigure(line_buffering=True)

sys.path.insert(0, str(Path(__file__).parent))

from lib.tmux import TmuxManager

# Configuration from environment
DATA_DIR = Path(os.environ.get("OSVAULD_DATA_DIR", "/tmp/osvauld-production"))
USERNAME = os.environ.get("OSVAULD_USERNAME", "production")
PASSPHRASE = os.environ.get("OSVAULD_PASSPHRASE", "test")

# App paths
PROJECT_ROOT = Path(__file__).parent.parent
DEMOS_APP = PROJECT_ROOT / "sample_apps" / "osvauld-demos"

# State file for persistence across restarts
STATE_FILE = DATA_DIR / "production_state.json"


def log(msg: str):
    """Log with timestamp."""
    ts = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    print(f"[{ts}] {msg}", flush=True)


def save_state(state: dict):
    """Save state to file."""
    STATE_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(STATE_FILE, "w") as f:
        json.dump(state, f, indent=2)


def load_state() -> dict:
    """Load state from file."""
    if STATE_FILE.exists():
        with open(STATE_FILE) as f:
            return json.load(f)
    return {}


def main():
    log("=" * 60)
    log("Osvauld Production Node Starting")
    log("=" * 60)
    log(f"Data directory: {DATA_DIR}")
    log(f"Username: {USERNAME}")
    log(f"Demos path: {DEMOS_APP}")

    # Check demos exist
    if not DEMOS_APP.exists():
        log(f"ERROR: Demos directory not found: {DEMOS_APP}")
        return 1

    # Start tmux session with node + owner
    log("\nStarting instances...")
    tm = TmuxManager(
        session_name="osvauld_production",
        base_dir=DATA_DIR,
        passphrase=PASSPHRASE,
    )
    tm.add_node("relay")
    tm.add_shell("owner")

    try:
        # Don't wipe data on restart (fresh=False preserves data)
        tm.start(fresh=False, timeout=60)
    except Exception as e:
        log(f"ERROR: Failed to start instances: {e}")
        return 1

    relay = tm.get_client("relay")
    owner = tm.get_client("owner")
    log("  Instances ready")

    # Get relay address
    relay_addr = relay.get_connection_string()
    log(f"\nRelay address: {relay_addr}")

    # Owner setup
    log("\nOwner: signup/login...")
    try:
        owner.signup_or_login(USERNAME, PASSPHRASE)
    except Exception as e:
        log(f"  Note: {e}")

    # Check for existing space or create new
    spaces = owner.list_spaces()
    space_id = None
    page_id = None

    if spaces:
        log(f"  Found {len(spaces)} existing space(s)")
        space_id = spaces[0].get("id")
        pages = owner.list_pages(space_id)
        if pages:
            page_id = pages[0].get("id")

    if not space_id:
        log("  Creating new space from demos...")
        space = owner.create_space_with_pages(str(DEMOS_APP))
        space_id = space["id"]
        pages = owner.list_pages(space_id)
        page_id = pages[0]["id"] if pages else None

    log(f"  Space: {space_id[:16]}...")

    # List apps
    if page_id:
        apps = owner.list_apps(page_id)
        app_names = [a.get("name") for a in apps]
        log(f"  Page: {page_id[:16]}...")
        log(f"  Apps: {app_names}")

    # Connect to relay and publish
    log("\nOwner: connecting to relay...")
    try:
        owner.connect_to_node(relay)
        time.sleep(2)
        log("  Connected!")

        log("Owner: publishing space...")
        owner.publish_to_node(space_id)
        time.sleep(2)
        log("  Published!")
    except Exception as e:
        log(f"  Warning: {e}")

    # Get viewer link
    try:
        viewer_link = owner.get_viewer_link(space_id)
        log(f"\nViewer link: {viewer_link}")
    except Exception as e:
        log(f"  Could not get viewer link: {e}")
        viewer_link = None

    # Save state
    state = {
        "relay_address": relay_addr,
        "space_id": space_id,
        "page_id": page_id,
        "viewer_link": viewer_link,
        "started_at": datetime.now().isoformat(),
    }
    save_state(state)

    # Print connection info
    log("\n" + "=" * 60)
    log("  PRODUCTION NODE READY")
    log("=" * 60)
    log(f"\nRelay:       {relay_addr}")
    log(f"Space ID:    {space_id}")
    if viewer_link:
        log(f"Viewer link: {viewer_link}")
    log(f"\nState saved to: {STATE_FILE}")
    log("\nTo connect viewers:")
    log(f"  1. Add relay: {relay_addr}")
    log(f"  2. Subscribe to space: {space_id}")
    log("\nTmux session: tmux attach -t osvauld_production")

    # Setup signal handlers for graceful shutdown
    running = True

    def handle_signal(signum, frame):
        nonlocal running
        log(f"\nReceived signal {signum}, shutting down...")
        running = False

    signal.signal(signal.SIGTERM, handle_signal)
    signal.signal(signal.SIGINT, handle_signal)

    # Health check loop
    log("\nRunning health checks every 30s...")
    check_interval = 30
    last_check = time.time()

    while running:
        time.sleep(1)

        if time.time() - last_check >= check_interval:
            last_check = time.time()
            try:
                # Quick health check
                relay.ping()
                owner.ping()
            except Exception as e:
                log(f"Health check failed: {e}")
                # systemd will restart us

    log("Shutdown complete")
    return 0


if __name__ == "__main__":
    sys.exit(main() or 0)
