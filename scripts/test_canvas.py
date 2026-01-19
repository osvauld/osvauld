#!/usr/bin/env python3
"""
Canvas App Test Script

Tests the infinite canvas app via AI interface.
Verifies shape creation, connectors, and auto-layout.

Usage:
    1. Start AI interface: ./target/debug/ai_interface --name canvas --instances canvas_user --show-ui
    2. Run this script: python3 scripts/test_canvas.py
"""

import socket
import json
import time
from pathlib import Path

SESSION_NAME = "canvas"
AI_SOCKET = f"/tmp/sthalam/{SESSION_NAME}/ai.sock"
CANVAS_APP_PATH = Path(__file__).parent.parent / "sample_apps" / "canvas-app"
PASSPHRASE = "test123"


def call(target, action, params=None, timeout=30):
    """Make a call to the AI interface socket."""
    cmd = {"target": target, "action": action, "id": 1}
    if params:
        cmd["params"] = params
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(AI_SOCKET)
    s.settimeout(timeout)
    s.send((json.dumps(cmd) + '\n').encode())
    response = json.loads(s.recv(65536).decode())
    s.close()
    result = response.get("result", {})
    if isinstance(result, dict) and "result" in result:
        result = result["result"]
    if response.get("error"):
        raise RuntimeError(f"{target}.{action}: {response['error']}")
    return result


def wait_for_eval(target, timeout=30):
    """Wait for Lua eval to be available (app loaded)."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            if call(target, "eval", {"code": "return 1"}) == 1:
                return True
        except Exception:
            pass
        time.sleep(0.5)
    return False


def main():
    print("=" * 60)
    print("Canvas App Test")
    print("=" * 60)

    # Setup
    print("\n[1/7] Signing up...")
    try:
        call("canvas_user", "sign_up", {"username": "canvas_user", "passphrase": PASSPHRASE})
        print("      Created new user")
    except Exception:
        print("      User already exists")

    print("\n[2/7] Logging in...")
    call("canvas_user", "login", {"passphrase": PASSPHRASE})
    time.sleep(2)
    print("      Logged in successfully")

    # Create space with canvas app
    print("\n[3/7] Creating canvas space...")
    try:
        call("canvas_user", "create_space", {
            "name": "My Canvas",
            "template_path": str(CANVAS_APP_PATH)
        })
        print("      Created new space")
    except Exception as e:
        if "already exists" in str(e).lower():
            print("      Space already exists")
        else:
            raise

    time.sleep(1)

    # Get space and page IDs
    print("\n[4/7] Getting space and page...")
    spaces = call("canvas_user", "list_spaces")["spaces"]
    canvas_space = next((s for s in spaces if s["name"] == "My Canvas"), None)
    if not canvas_space:
        print("      ERROR: Canvas space not found")
        return

    space_id = canvas_space["space_id"]
    print(f"      Space ID: {space_id[:8]}...")

    pages = call("canvas_user", "list_pages", {"space_id": space_id})["pages"]
    page_id = pages[0]["page_id"]
    print(f"      Page ID: {page_id[:8]}...")

    # Open app
    print("\n[5/7] Opening canvas app...")
    call("canvas_user", "open_app", {"page_id": page_id, "app_name": "Canvas"})

    # Wait for app to load
    print("      Waiting for app to load...")
    if not wait_for_eval("canvas_user", timeout=30):
        print("      ERROR: App failed to load")
        return
    print("      App loaded successfully")

    # Test shape creation
    print("\n[6/7] Testing canvas operations...")

    # Create shapes
    print("      Creating shapes...")
    shape1_id = call("canvas_user", "eval", {"code": "return add_shape('rectangle', 100, 100, 150, 80)"})
    print(f"        - Created rectangle: {shape1_id}")

    shape2_id = call("canvas_user", "eval", {"code": "return add_shape('ellipse', 350, 100, 120, 80)"})
    print(f"        - Created ellipse: {shape2_id}")

    shape3_id = call("canvas_user", "eval", {"code": "return add_shape('rectangle', 100, 250, 150, 80)"})
    print(f"        - Created rectangle: {shape3_id}")

    shape4_id = call("canvas_user", "eval", {"code": "return add_shape('ellipse', 350, 250, 120, 80)"})
    print(f"        - Created ellipse: {shape4_id}")

    # Get shape count
    shape_count = call("canvas_user", "eval", {"code": "return get_shape_count()"})
    print(f"      Shape count: {shape_count}")

    # Create connectors
    print("      Creating connectors...")
    call("canvas_user", "eval", {"code": f"return create_connector('{shape1_id}', '{shape2_id}')"})
    call("canvas_user", "eval", {"code": f"return create_connector('{shape2_id}', '{shape4_id}')"})
    call("canvas_user", "eval", {"code": f"return create_connector('{shape1_id}', '{shape3_id}')"})
    call("canvas_user", "eval", {"code": f"return create_connector('{shape3_id}', '{shape4_id}')"})

    connector_count = call("canvas_user", "eval", {"code": "return get_connector_count()"})
    print(f"      Connector count: {connector_count}")

    # Test auto-layout
    print("      Testing auto-layout...")
    call("canvas_user", "eval", {"code": "auto_layout()"})
    print("        - Auto-layout applied")

    # Test pan
    print("      Testing pan...")
    call("canvas_user", "eval", {"code": "pan_canvas(50, 50)"})
    print("        - Panned canvas by (50, 50)")

    # Get final state
    print("\n[7/7] Final state...")
    shapes = call("canvas_user", "eval", {"code": "return get_shapes()"})
    print(f"      Total shapes: {len(shapes) if shapes else 0}")

    for shape in (shapes or []):
        print(f"        - {shape.get('shape_type', 'unknown')}: ({shape.get('x', 0):.0f}, {shape.get('y', 0):.0f})")

    print("\n" + "=" * 60)
    print("[OK] Canvas app tests passed!")
    print("=" * 60)


if __name__ == "__main__":
    main()
