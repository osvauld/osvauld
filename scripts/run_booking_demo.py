#!/usr/bin/env python3
"""Booking Demo Automation Script

Automates the complete booking flow using the AI interface:
1. Starts AI interface with provider, customer, and kunki node
2. Sets up provider (signup, login, add node, create space, import page, publish)
3. Gets viewer link for customer
4. Sets up customer (signup, login, connect via viewer link)
5. Provider sets schedule
6. Customer makes a booking
7. Provider confirms the booking

Usage:
    ./scripts/run_booking_demo.py                  # Full demo with fresh setup
    ./scripts/run_booking_demo.py --skip-setup     # Skip ai_interface startup (use existing)
    ./scripts/run_booking_demo.py --keep-alive     # Keep running after demo

Prerequisites:
    cargo build -p slint_shell -p kunki -p ai_interface
"""

import subprocess
import socket
import json
import time
import sys
import os
from pathlib import Path
from datetime import datetime, timedelta

# Paths
SCRIPT_DIR = Path(__file__).parent
ROOT_DIR = SCRIPT_DIR.parent
AI_INTERFACE_BINARY = ROOT_DIR / "target" / "debug" / "ai_interface"
SAMPLE_APP_PATH = ROOT_DIR / "sample_apps" / "my-booking"

# Config
SESSION_NAME = "booking"
AI_SOCKET = f"/tmp/sthalam/{SESSION_NAME}/ai.sock"
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
        result = result.get("result", {})

    # Check for errors
    error = response.get("error")
    if not error and isinstance(response.get("result"), dict):
        error = response["result"].get("error")

    if error:
        raise RuntimeError(f"{target}.{action}: {error}")

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


def wait_for_socket(sock_path, timeout=30):
    """Wait for socket to be ready."""
    start = time.time()
    while time.time() - start < timeout:
        if os.path.exists(sock_path):
            try:
                status = send({"action": "status", "id": 0})
                if status:
                    return True
            except:
                pass
        time.sleep(0.5)
    return False


def wait_for_eval(target, timeout=30):
    """Wait for Lua eval to be available (app loaded)."""
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


def start_ai_interface():
    """Start the AI interface with provider, customer, and kunki."""
    header("STARTING AI INTERFACE")

    # Clean up existing session
    base_dir = Path(f"/tmp/sthalam/{SESSION_NAME}")
    if base_dir.exists():
        step("Cleaning up existing session...")
        import shutil
        for sock in base_dir.glob("*.sock"):
            sock.unlink(missing_ok=True)

    # Kill existing tmux session
    subprocess.run(["tmux", "kill-session", "-t", SESSION_NAME],
                   capture_output=True, check=False)

    # Start ai_interface
    step("Starting ai_interface...")
    cmd = [
        str(AI_INTERFACE_BINARY),
        "--name", SESSION_NAME,
        "--instances", "provider,customer",
        "--node",
        "--show-ui"
    ]

    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True
    )

    # Wait for ready message
    for line in proc.stdout:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
            if msg.get("status") == "ready":
                ok(f"AI interface ready: {msg.get('instances')}")
                break
        except json.JSONDecodeError:
            pass

    # Wait for socket
    if not wait_for_socket(AI_SOCKET, timeout=60):
        fail("AI interface socket not ready")
        proc.terminate()
        sys.exit(1)

    # Give iroh discovery time to propagate
    step("Waiting for iroh discovery (15s)...")
    time.sleep(15)

    return proc


def setup_provider():
    """Set up the provider: signup, login, add node, create space, import page, publish."""
    header("SETTING UP PROVIDER")

    # Signup
    step("Signing up provider...")
    try:
        call("provider", "sign_up", {"username": "provider", "passphrase": PASSPHRASE})
        ok("Provider signed up")
    except RuntimeError as e:
        if "already" in str(e).lower() or "exists" in str(e).lower():
            info("Provider already signed up")
        else:
            raise

    # Login
    step("Logging in provider...")
    call("provider", "login", {"passphrase": PASSPHRASE})
    ok("Provider logged in")

    # Wait for P2P
    time.sleep(3)

    # Get kunki connection string
    step("Getting kunki connection string...")
    kunki_result = call("kunki", "get_connection_string")
    conn_str = kunki_result.get("connection_string")
    if not conn_str:
        fail("Could not get kunki connection string")
        sys.exit(1)
    ok(f"Got kunki connection: {conn_str[:50]}...")

    # Add kunki as node
    step("Adding kunki as node...")
    call("provider", "add_node", {"connection_string": conn_str}, timeout=60)
    ok("Kunki node added")

    # Wait for connection
    time.sleep(5)

    # Get node_id
    nodes_result = call("provider", "list_nodes")
    nodes = nodes_result.get("nodes", [])
    if not nodes:
        fail("No nodes found after adding kunki")
        sys.exit(1)
    node_id = nodes[0]["node_id"]
    info(f"Connected to node: {node_id[:16]}...")

    # Create space
    step("Creating Booking Service space...")
    call("provider", "create_space", {
        "name": "Booking Service",
        "template_path": str(SAMPLE_APP_PATH)
    })

    # Query for the actual space_id (create_space triggers UI callback)
    time.sleep(1)  # Give time for space creation
    spaces_result = call("provider", "list_spaces")
    spaces = spaces_result.get("spaces", [])
    booking_space = next((s for s in spaces if s["name"] == "Booking Service"), None)
    if not booking_space:
        fail("Space 'Booking Service' not found after creation")
        sys.exit(1)
    space_id = booking_space["id"]
    ok(f"Space created: {space_id}")

    # Import booking page
    step("Importing my-booking page...")
    page_result = call("provider", "import_page", {
        "space_id": space_id,
        "page_dir": str(SAMPLE_APP_PATH)
    })
    page_id = page_result.get("page_id")
    apps = page_result.get("apps", [])
    ok(f"Page imported: {page_id}")
    info(f"Apps: {apps}")

    # Publish to node
    step("Publishing space to node...")
    call("provider", "publish_space", {"space_id": space_id, "node_id": node_id})
    ok("Space published to node")

    # Wait for sync
    step("Waiting for sync (5s)...")
    time.sleep(5)

    # Open Service Provider app
    step("Opening Service Provider app...")
    call("provider", "open_app", {"page_id": page_id, "app_name": "Service Provider"})

    if not wait_for_eval("provider"):
        fail("Provider app failed to load")
        sys.exit(1)
    ok("Service Provider app loaded")

    # Set up Monday schedule
    step("Setting up Monday schedule (09:00-17:00, 60min slots)...")
    call("provider", "eval", {"code": 'set_schedule_via_ui("monday", "09:00", "17:00", 60)'})
    ok("Monday schedule set")

    # Get provider apps
    provider_apps = call("provider", "list_apps", {"page_id": page_id})
    provider_app_names = [a["name"] for a in provider_apps.get("apps", [])]
    info(f"Provider sees apps: {provider_app_names}")

    return {
        "space_id": space_id,
        "page_id": page_id,
        "node_id": node_id,
        "apps": provider_app_names
    }


def get_viewer_link(space_id, node_id):
    """Get viewer link via provider for customer connection."""
    header("GETTING VIEWER LINK")

    step("Getting shareable link from node...")
    link_result = call("provider", "get_shareable_link", {
        "space_id": space_id,
        "node_id": node_id
    })
    viewer_link = link_result.get("connection_string")
    if not viewer_link:
        fail("Could not get viewer link")
        sys.exit(1)
    ok(f"Got viewer link: {viewer_link[:60]}...")

    return viewer_link


def setup_customer(viewer_link):
    """Set up the customer: signup, login, connect via viewer link."""
    header("SETTING UP CUSTOMER")

    # Signup
    step("Signing up customer...")
    try:
        call("customer", "sign_up", {"username": "customer", "passphrase": PASSPHRASE})
        ok("Customer signed up")
    except RuntimeError as e:
        if "already" in str(e).lower() or "exists" in str(e).lower():
            info("Customer already signed up")
        else:
            raise

    # Login
    step("Logging in customer...")
    call("customer", "login", {"passphrase": PASSPHRASE})
    ok("Customer logged in")

    # Wait for P2P discovery
    step("Waiting for P2P discovery (10s)...")
    time.sleep(10)

    # Connect via viewer link (with retries)
    step("Connecting customer via viewer link...")
    max_retries = 3
    for attempt in range(max_retries):
        try:
            call("customer", "add_website", {"connection_string": viewer_link}, timeout=90)
            ok("Customer connected to node")
            break
        except RuntimeError as e:
            if attempt < max_retries - 1:
                info(f"Attempt {attempt + 1} failed, retrying in 5s...")
                time.sleep(5)
            else:
                raise

    # Wait for sync
    step("Waiting for data sync (5s)...")
    time.sleep(5)

    return True


def verify_customer_apps():
    """Verify customer only sees Service Customer app."""
    header("VERIFYING CUSTOMER APPS")

    # Get customer spaces
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
    if not pages_list:
        fail("Customer has no pages")
        sys.exit(1)
    cust_page_id = pages_list[0]["id"]

    # Get apps - THE KEY CHECK
    cust_apps = call("customer", "list_apps", {"page_id": cust_page_id})
    cust_app_names = [a["name"] for a in cust_apps.get("apps", [])]

    print()
    print(f"  Customer apps: {cust_app_names}")
    print()

    if cust_app_names == ["Service Customer"]:
        ok("Customer sees ONLY 'Service Customer' app - CORRECT!")
    elif "Service Provider" in cust_app_names:
        fail("Customer sees 'Service Provider' app - BUG! Permit not filtering correctly")
        sys.exit(1)
    else:
        info(f"Customer apps: {cust_app_names}")

    # Open customer app
    step("Opening Service Customer app...")
    call("customer", "open_app", {"page_id": cust_page_id, "app_name": "Service Customer"})

    if not wait_for_eval("customer"):
        fail("Customer app failed to load")
        sys.exit(1)
    ok("Service Customer app loaded")

    return {
        "space_id": cust_space_id,
        "page_id": cust_page_id,
        "apps": cust_app_names
    }


def customer_make_booking():
    """Customer makes a booking."""
    header("CUSTOMER MAKES BOOKING")

    # Get tomorrow's date for booking (need a Monday for our schedule)
    today = datetime.now()
    # Find next Monday
    days_until_monday = (7 - today.weekday()) % 7
    if days_until_monday == 0:
        days_until_monday = 7  # Next Monday, not today
    next_monday = (today + timedelta(days=days_until_monday)).strftime("%Y-%m-%d")

    step(f"Booking appointment for {next_monday} (Monday) at 10:00...")

    # Set form fields and submit directly (single line to avoid JSON parsing issues)
    lua_code = (
        f'on_field_changed("booking_date", "{next_monday}"); '
        f'on_field_changed("booking_time", "10:00"); '
        f'on_field_changed("booking_service", "Consultation"); '
        f'on_field_changed("booking_notes", "Booked via AI demo"); '
        f'on_field_changed("customer_name", "Test Customer"); '
        f'on_field_changed("customer_phone", "555-0123"); '
        f'do_submit_booking_direct(); '
        f'return get_bookings_count()'
    )

    result = call("customer", "eval", {"code": lua_code})
    booking_count = result if isinstance(result, int) else result.get("result", 0)

    if booking_count > 0:
        ok(f"Booking created! Customer has {booking_count} booking(s)")
    else:
        fail("Failed to create booking")
        return False

    # Get booking details
    step("Retrieving booking details...")
    bookings_result = call("customer", "eval", {"code": "return get_my_bookings()"})
    bookings = bookings_result if isinstance(bookings_result, list) else bookings_result.get("result", [])

    if bookings:
        booking = bookings[-1]  # Get latest
        info(f"Booking ID: {booking.get('id', 'unknown')}")
        info(f"Date: {booking.get('date')}, Time: {booking.get('start_time')} - {booking.get('end_time')}")
        info(f"Status: {booking.get('status')}")
        return booking

    return None


def provider_confirm_booking():
    """Provider confirms the customer's booking."""
    header("PROVIDER CONFIRMS BOOKING")

    # Wait for booking to sync to provider
    step("Waiting for booking to sync (5s)...")
    time.sleep(5)

    # Get bookings on provider side
    step("Checking bookings on provider side...")
    bookings_result = call("provider", "eval", {"code": "return get_all_bookings()"})
    bookings = bookings_result if isinstance(bookings_result, list) else bookings_result.get("result", [])

    if not bookings:
        info("No bookings visible to provider yet")
        info("(Booking syncs via kunki node - may need more time)")
        return False

    # Find pending booking
    pending = [b for b in bookings if b.get("status") == "pending"]
    if not pending:
        info("No pending bookings to confirm")
        return False

    booking = pending[0]
    customer_did = booking.get("customer_did", "")
    booking_id = booking.get("id", "")

    info(f"Found pending booking: {booking_id}")
    info(f"Customer: {customer_did[:16]}...")

    # Confirm the booking
    step("Confirming booking...")
    lua_code = f'confirm_booking_via_ui("{customer_did}", "{booking_id}"); return true'
    call("provider", "eval", {"code": lua_code})
    ok("Booking confirmed!")

    return True


def main():
    import argparse
    parser = argparse.ArgumentParser(description="Booking Demo Automation")
    parser.add_argument("--skip-setup", action="store_true",
                        help="Skip ai_interface startup (use existing)")
    parser.add_argument("--keep-alive", action="store_true",
                        help="Keep running after demo")
    args = parser.parse_args()

    # Check binaries
    if not args.skip_setup:
        if not AI_INTERFACE_BINARY.exists():
            print(f"Error: {AI_INTERFACE_BINARY} not found")
            print("Run: cargo build -p slint_shell -p kunki -p ai_interface")
            sys.exit(1)

    if not SAMPLE_APP_PATH.exists():
        print(f"Error: Sample app not found at {SAMPLE_APP_PATH}")
        sys.exit(1)

    ai_proc = None

    try:
        # Start AI interface
        if not args.skip_setup:
            ai_proc = start_ai_interface()
        else:
            header("CONNECTING TO EXISTING AI INTERFACE")
            if not wait_for_socket(AI_SOCKET, timeout=5):
                fail("AI interface not running")
                print("\nStart it with:")
                print(f"  ./target/debug/ai_interface --name {SESSION_NAME} --instances provider,customer --node --show-ui")
                sys.exit(1)
            ok("Connected to AI interface")

        # Setup provider
        provider_info = setup_provider()

        # Get viewer link
        viewer_link = get_viewer_link(provider_info["space_id"], provider_info["node_id"])

        # Setup customer
        setup_customer(viewer_link)

        # Verify customer apps
        customer_info = verify_customer_apps()

        # Customer makes booking
        booking = customer_make_booking()

        # Provider confirms booking (optional, may need more sync time)
        if booking:
            provider_confirm_booking()

        # Summary
        header("DEMO COMPLETE")
        print(f"  Session: {SESSION_NAME}")
        print(f"  Socket: {AI_SOCKET}")
        print()
        print(f"  Provider space: {provider_info['space_id']}")
        print(f"  Provider page: {provider_info['page_id']}")
        print(f"  Provider apps: {provider_info['apps']}")
        print()
        print(f"  Customer apps: {customer_info['apps']}")
        print()
        print("  To interact manually:")
        print(f"    ./scripts/ai {SESSION_NAME} '{{\"target\":\"provider\",\"action\":\"list_spaces\"}}'")
        print(f"    ./scripts/ai {SESSION_NAME} '{{\"target\":\"customer\",\"action\":\"eval\",\"params\":{{\"code\":\"return get_my_bookings()\"}}}}'")
        print()
        print("  To attach to tmux session:")
        print(f"    tmux attach -t {SESSION_NAME}")
        print()

        if args.keep_alive:
            print("  Demo kept alive. Press Ctrl+C to exit.\n")
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
        if ai_proc and not args.keep_alive:
            info("AI interface left running in tmux")
            info(f"Kill with: tmux kill-session -t {SESSION_NAME}")


if __name__ == "__main__":
    main()
