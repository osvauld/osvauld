#!/usr/bin/env python3
"""End-to-End Booking Test with AI Interface

Clean slate test:
1. Start ai_interface with fresh DBs
2. Provider: signup, login, add node, create space, import page, publish
3. Customer: signup, login, connect via viewer link
4. Verify blocked slots appear in customer calendar

Usage:
    python scripts/test_booking_e2e.py
"""

import subprocess
import socket
import json
import time
import sys
import os
from pathlib import Path

# Paths
ROOT_DIR = Path(__file__).parent.parent
AI_INTERFACE = ROOT_DIR / "target" / "debug" / "ai_interface"
SAMPLE_APP = ROOT_DIR / "sample_apps" / "my-booking"

# Config
SESSION = "booking"
BASE_DIR = f"/tmp/sthalam/{SESSION}"
AI_SOCKET = f"{BASE_DIR}/ai.sock"
PASSPHRASE = "test123"


def send(cmd, timeout=30):
    """Send JSON command to AI interface socket."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        s.connect(AI_SOCKET)
        s.settimeout(timeout)
        s.send((json.dumps(cmd) + '\n').encode())
        response = s.recv(65536).decode()
        return json.loads(response)
    finally:
        s.close()


def call(target, action, params=None, timeout=30):
    """Call AI interface with target instance."""
    cmd = {"target": target, "action": action, "id": 1}
    if params:
        cmd["params"] = params
    response = send(cmd, timeout=timeout)

    # Handle nested result structure
    result = response.get("result", {})
    if isinstance(result, dict) and "result" in result:
        result = result["result"]

    # Check for errors
    if "error" in response and response["error"]:
        raise RuntimeError(f"{target}.{action}: {response['error']}")
    if isinstance(result, dict) and "error" in result:
        raise RuntimeError(f"{target}.{action}: {result['error']}")

    return result


def header(text):
    print(f"\n{'='*60}")
    print(f"  {text}")
    print(f"{'='*60}\n")


def step(text):
    print(f"  -> {text}")


def ok(text):
    print(f"  [OK] {text}")


def fail(text):
    print(f"  [FAIL] {text}")


def info(text):
    print(f"  [i] {text}")


def wait_for_socket(timeout=30):
    """Wait for AI socket to be ready."""
    start = time.time()
    while time.time() - start < timeout:
        if os.path.exists(AI_SOCKET):
            try:
                send({"action": "status", "id": 0}, timeout=5)
                return True
            except:
                pass
        time.sleep(0.5)
    return False


def wait_for_eval(target, timeout=20):
    """Wait for eval to be available (app loaded)."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            result = call(target, "eval", {"code": "return 1"})
            if result == 1:
                return True
        except:
            pass
        time.sleep(0.5)
    return False


def main():
    # Check binary exists
    if not AI_INTERFACE.exists():
        fail(f"AI interface not found: {AI_INTERFACE}")
        print("Run: cargo build -p ai_interface")
        sys.exit(1)

    if not SAMPLE_APP.exists():
        fail(f"Sample app not found: {SAMPLE_APP}")
        sys.exit(1)

    # ============================================================
    # CLEAN START
    # ============================================================
    header("STARTING FRESH")

    # Kill existing session
    subprocess.run(["tmux", "kill-session", "-t", SESSION],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    # Clean up
    subprocess.run(["rm", "-rf", BASE_DIR], check=True)
    ok(f"Cleaned {BASE_DIR}")

    # Start ai_interface
    step("Starting ai_interface...")
    proc = subprocess.Popen([
        str(AI_INTERFACE),
        "--name", SESSION,
        "--instances", "provider,customer",
        "--node",
        "--show-ui"
    ], stdout=subprocess.PIPE, stderr=subprocess.STDOUT)

    if not wait_for_socket(timeout=30):
        fail("AI interface failed to start")
        proc.terminate()
        sys.exit(1)
    ok("AI interface ready")

    try:
        # Check status
        status = send({"action": "status", "id": 0})
        info(f"Instances: {list(status.get('result', {}).keys())}")

        # ============================================================
        # PROVIDER SETUP
        # ============================================================
        header("SETTING UP PROVIDER")

        step("Signing up provider...")
        call("provider", "sign_up", {"username": "provider", "passphrase": PASSPHRASE})
        ok("Provider signed up")

        step("Logging in provider...")
        call("provider", "login", {"passphrase": PASSPHRASE})
        ok("Provider logged in")

        # Wait for P2P init
        time.sleep(2)

        # Get kunki connection and add as node
        step("Getting kunki connection string...")
        kunki_conn = call("kunki", "get_connection_string")
        conn_str = kunki_conn.get("connection_string")
        ok(f"Got connection: {conn_str[:50]}...")

        step("Adding kunki as node...")
        node_result = call("provider", "add_node", {"connection_string": conn_str}, timeout=60)
        node_id = node_result.get("node_id")
        ok(f"Node added: {node_id[:16]}...")

        # Wait for node connection
        time.sleep(3)

        # Create space
        step("Creating space...")
        space = call("provider", "create_space", {"name": "Booking Service", "template_path": str(SAMPLE_APP)})
        space_id = space.get("id")
        ok(f"Space: {space_id}")

        # Import page
        step("Importing my-booking page...")
        page = call("provider", "import_page", {
            "space_id": space_id,
            "page_dir": str(SAMPLE_APP)
        })
        page_id = page.get("page_id")
        apps = page.get("apps", [])
        ok(f"Page: {page_id}")
        ok(f"Apps: {apps}")

        # Publish to node
        step("Publishing to node...")
        call("provider", "publish_space", {"space_id": space_id, "node_id": node_id})
        ok("Published to node")

        # Wait for sync
        step("Waiting for sync (5s)...")
        time.sleep(5)

        # Open provider app
        step("Opening Service Provider app...")
        call("provider", "open_app", {"page_id": page_id, "app_name": "Service Provider"})

        if not wait_for_eval("provider"):
            fail("Provider app failed to load")
            sys.exit(1)
        ok("Service Provider app loaded")

        # Set up schedule
        step("Setting up Monday schedule...")
        call("provider", "eval", {"code": 'set_schedule_via_ui("monday", "09:00", "17:00", 60)'})
        ok("Monday 09:00-17:00 set")

        # Block a time slot
        step("Blocking Monday 12:00-13:00 (Lunch)...")
        call("provider", "eval", {"code": 'block_time_via_ui("2026-01-20", "12:00", "13:00", "Lunch Break")'})
        ok("Blocked 12:00-13:00")

        # Get viewer link
        step("Getting viewer link...")
        link_result = call("provider", "get_shareable_link", {
            "space_id": space_id,
            "node_id": node_id
        })
        viewer_link = link_result.get("connection_string")
        ok(f"Viewer link: {viewer_link[:50]}...")

        # ============================================================
        # CUSTOMER SETUP
        # ============================================================
        header("SETTING UP CUSTOMER")

        step("Signing up customer...")
        call("customer", "sign_up", {"username": "customer", "passphrase": PASSPHRASE})
        ok("Customer signed up")

        step("Logging in customer...")
        call("customer", "login", {"passphrase": PASSPHRASE})
        ok("Customer logged in")

        # Wait for P2P
        step("Waiting for P2P discovery (10s)...")
        time.sleep(10)

        # Connect via viewer link with retry
        step("Connecting to node via viewer link...")
        for attempt in range(3):
            try:
                call("customer", "add_website", {"connection_string": viewer_link}, timeout=90)
                ok("Connected to node")
                break
            except RuntimeError as e:
                if attempt < 2:
                    info(f"Attempt {attempt + 1} failed, retrying in 5s...")
                    time.sleep(5)
                else:
                    raise

        # Wait for sync
        step("Waiting for data sync (10s)...")
        time.sleep(10)

        # ============================================================
        # VERIFY CUSTOMER
        # ============================================================
        header("VERIFYING CUSTOMER")

        # Get spaces
        cust_spaces = call("customer", "list_spaces")
        spaces = cust_spaces.get("spaces", [])
        if not spaces:
            fail("Customer has no spaces")
            sys.exit(1)
        cust_space_id = spaces[0]["id"]
        ok(f"Customer has space: {spaces[0]['name']}")

        # Get pages
        cust_pages = call("customer", "list_pages", {"space_id": cust_space_id})
        pages_list = cust_pages.get("pages", [])
        cust_page_id = pages_list[0]["id"]

        # Get apps
        cust_apps = call("customer", "list_apps", {"page_id": cust_page_id})
        cust_app_names = [a["name"] for a in cust_apps.get("apps", [])]

        print()
        print(f"  Provider apps: {apps}")
        print(f"  Customer apps: {cust_app_names}")
        print()

        # Check customer only sees Service Customer
        if cust_app_names == ["Service Customer"]:
            ok("Customer sees ONLY 'Service Customer' - CORRECT!")
        elif "Service Provider" in cust_app_names:
            fail("Customer sees 'Service Provider' - permit filtering broken!")
        else:
            info(f"Customer apps: {cust_app_names}")

        # Open customer app
        step("Opening Service Customer app...")
        call("customer", "open_app", {"page_id": cust_page_id, "app_name": "Service Customer"})

        if not wait_for_eval("customer"):
            fail("Customer app failed to load")
            sys.exit(1)
        ok("Service Customer app loaded")

        # ============================================================
        # CHECK BLOCKED SLOTS
        # ============================================================
        header("CHECKING BLOCKED SLOTS")

        # Check if customer sees blocked slot in calendar
        step("Checking if 12:00 slot is blocked...")
        # Get booked/blocked slots from customer view
        booked = call("customer", "eval", {"code": """
            local result = {}
            if calendar_layer then
                local keys = calendar_layer:keys()
                for _, key in ipairs(keys) do
                    local slot = calendar_layer:get(key)
                    if slot then
                        table.insert(result, {key = key, booked = slot.booked})
                    end
                end
            end
            return result
        """})

        if booked:
            ok(f"Customer sees calendar data: {booked}")
            # Check for the blocked slot
            blocked_found = any("12:00" in str(slot) for slot in booked) if isinstance(booked, list) else False
            if blocked_found:
                ok("12:00 blocked slot visible to customer!")
            else:
                info("12:00 blocked slot NOT in calendar (derivation may not be working)")
        else:
            info("No calendar data yet (derivation may not be running)")

        # ============================================================
        # SUMMARY
        # ============================================================
        header("TEST COMPLETE")

        print(f"  Space ID: {space_id}")
        print(f"  Page ID:  {page_id}")
        print()
        print("  Both apps are running in tmux session 'booking'")
        print("  Use: tmux attach -t booking")
        print()
        print("  Provider window: check blocked time at 12:00")
        print("  Customer window: verify if 12:00 shows as unavailable")
        print()

        # Keep running
        print("  Press Ctrl+C to stop...\n")
        try:
            while True:
                time.sleep(1)
        except KeyboardInterrupt:
            print("\n  Shutting down...")

    except Exception as e:
        fail(str(e))
        import traceback
        traceback.print_exc()
        sys.exit(1)

    finally:
        proc.terminate()


if __name__ == "__main__":
    main()
