#!/usr/bin/env python3
"""Visual test launcher for Osvauld apps.

Launches the visual_test binary and manages reload lifecycle.
No Rust recompilation needed — change Lua/Slint and press 'r' to reload.

Usage:
    python scripts/visual_test.py <app_dir> [Name:role ...]
    python scripts/visual_test.py --preset chat
    python scripts/visual_test.py --preset ecomm
    python scripts/visual_test.py --preset chat10

Presets:
    chat    - group-chat with Alice (owner) + Bob (viewer)
    chat10  - group-chat with 10 users
    ecomm   - my-shop with ShopOwner (owner) + Customer (viewer)
    landing - sthalam landing page (read-only)
    docs    - protocol docs browser (read-only)

Controls:
    r + Enter  — reload all windows (re-reads Lua/Slint from disk)
    q + Enter  — quit
    Ctrl+C     — quit
"""

import subprocess
import sys
import threading
import signal
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent

PRESETS = {
    "chat": {
        "app_dir": "sample_apps/osvauld-demos/group-chat",
        "peers": ["Alice:owner", "Bob:viewer"],
    },
    "chat10": {
        "app_dir": "sample_apps/osvauld-demos/group-chat",
        "peers": [
            "Alice:owner", "Bob:viewer", "Carol:viewer", "Dave:viewer",
            "Eve:viewer", "Frank:viewer", "Grace:viewer", "Heidi:viewer",
            "Ivan:viewer", "Judy:viewer",
        ],
    },
    "ecomm": {
        "app_dir": "sample_apps/my-shop/shop-owner",
        "peers": ["ShopOwner:owner", "Customer:viewer"],
    },
    "landing": {
        "app_dir": "sample_apps/osvauld-demos/sthalam-landing",
        "peers": ["Alice:owner"],
    },
    "docs": {
        "app_dir": "sample_apps/osvauld-demos/protocol-docs",
        "peers": ["Alice:owner"],
    },
}

BINARY = PROJECT_ROOT / "target" / "debug" / "examples" / "visual_test"


def build_binary():
    """Build the visual_test binary if needed."""
    print("Building visual_test...")
    result = subprocess.run(
        ["cargo", "build", "--example", "visual_test", "-p", "app_test"],
        cwd=PROJECT_ROOT,
    )
    if result.returncode != 0:
        print("Build failed!")
        sys.exit(1)


def launch(app_dir: str, peers: list[str]) -> subprocess.Popen:
    """Launch the visual_test binary."""
    cmd = [str(BINARY), app_dir, *peers]
    return subprocess.Popen(cmd, cwd=PROJECT_ROOT)


def run(app_dir: str, peers: list[str]):
    """Main loop — launch binary, wait for input, reload on 'r'."""
    build_binary()

    proc = launch(app_dir, peers)
    print(f"\n  r + Enter = reload  |  q + Enter = quit  |  Ctrl+C = quit\n")

    def input_loop():
        """Read commands from stdin in a background thread."""
        nonlocal proc
        try:
            while True:
                line = input().strip().lower()
                if line == "r":
                    print("Reloading...")
                    proc.terminate()
                    proc.wait()
                    proc = launch(app_dir, peers)
                    print("Reloaded. (r = reload, q = quit)")
                elif line == "q":
                    proc.terminate()
                    proc.wait()
                    sys.exit(0)
        except (EOFError, KeyboardInterrupt):
            proc.terminate()

    thread = threading.Thread(target=input_loop, daemon=True)
    thread.start()

    # Wait for process to exit (user closed all windows)
    # Re-check after wait() returns — reload thread may have replaced proc
    import time
    while True:
        try:
            proc.wait()
            # Brief pause to let reload thread assign new proc
            time.sleep(0.3)
            # If reload thread spawned a new process, keep waiting
            if proc.poll() is None:
                continue
            break
        except KeyboardInterrupt:
            proc.terminate()
            proc.wait()
            break


def main():
    signal.signal(signal.SIGINT, lambda *_: sys.exit(0))

    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    if sys.argv[1] == "--preset":
        if len(sys.argv) < 3 or sys.argv[2] not in PRESETS:
            print(f"Available presets: {', '.join(PRESETS.keys())}")
            sys.exit(1)
        preset = PRESETS[sys.argv[2]]
        run(preset["app_dir"], preset["peers"])
    else:
        app_dir = sys.argv[1]
        peers = sys.argv[2:] if len(sys.argv) > 2 else ["Alice:owner", "Bob:viewer"]
        run(app_dir, peers)


if __name__ == "__main__":
    main()
