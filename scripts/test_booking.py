#!/usr/bin/env python3
"""Test Booking App via AI Interface

Sends commands to ai_interface socket to set up and test the booking flow.
Assumes ai_interface is already running with:
    ./ai_interface --name booking --instances provider,customer --node --show-ui

Usage:
    ./scripts/test_booking.py
"""

import socket
import json
import time
import sys

AI_SOCKET = "/tmp/sthalam/booking/ai.sock"
SAMPLE_APP_PATH = "/home/abe/osvauld/sample_apps/my-booking"
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
    if "error" in response and response["error"]:
        raise RuntimeError(f"{target}.{action}: {response['error']}")
    return response.get("result", {})


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


def wait_for_eval(target, timeout=20):
    """Wait for eval to be available (app loaded)."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            result = call(target, "eval", {"code": "return 1"})
            if result.get("result") == 1:
                return True
        except:
            pass
        time.sleep(0.5)
    return False


def main():
    header("CHECKING AI INTERFACE")

    try:
        status = send({"action": "status", "id": 0})
        info(f"Connected to booking session")
        info(f"Instances: {list(status.keys())}")
    except Exception as e:
        fail(f"Cannot connect to AI interface: {e}")
        print("\nMake sure ai_interface is running:")
        print("  ./target/debug/ai_interface --name booking --instances provider,customer --node --show-ui")
        sys.exit(1)

    # ============================================================
    # SETUP PROVIDER
    # ============================================================
    header("SETTING UP PROVIDER")

    step("Signing up provider...")
    try:
        call("provider", "sign_up", {"username": "provider", "passphrase": PASSPHRASE})
        ok("Provider signed up")
    except RuntimeError as e:
        if "already" in str(e).lower():
            info("Provider already signed up")
        else:
            raise

    step("Logging in provider...")
    call("provider", "login", {"passphrase": PASSPHRASE})
    ok("Provider logged in")

    # Wait for P2P
    time.sleep(2)

    # Get kunki connection string and add as node
    step("Getting kunki connection string...")
    kunki_conn = call("kunki", "get_connection_string")
    conn_str = kunki_conn.get("result", {}).get("connection_string")
    if not conn_str:
        conn_str = kunki_conn.get("connection_string")
    ok(f"Got kunki connection: {conn_str[:50]}...")

    step("Adding kunki as node...")
    call("provider", "add_node", {"connection_string": conn_str}, timeout=60)
    ok("Kunki node added")

    # Wait for connection
    time.sleep(3)

    # Get node_id
    nodes = call("provider", "list_nodes").get("result", {}).get("nodes", [])
    if not nodes:
        nodes = call("provider", "list_nodes").get("nodes", [])
    if not nodes:
        fail("No nodes found")
        sys.exit(1)
    node_id = nodes[0]["node_id"]
    info(f"Connected to node: {node_id[:16]}...")

    # Create space
    step("Creating Booking Service space...")
    space = call("provider", "create_space", {"name": "Booking Service", "template_path": SAMPLE_APP_PATH})
    space_id = space.get("result", {}).get("id") or space.get("id")
    ok(f"Space created: {space_id}")

    # Import booking page
    step("Importing my-booking page...")
    page = call("provider", "import_page", {
        "space_id": space_id,
        "page_dir": SAMPLE_APP_PATH
    })
    page_id = page.get("result", {}).get("page_id") or page.get("page_id")
    apps = page.get("result", {}).get("apps") or page.get("apps")
    ok(f"Page imported: {page_id}")
    ok(f"Apps: {apps}")

    # Publish to node
    step("Publishing space to node...")
    call("provider", "publish_space", {"space_id": space_id, "node_id": node_id})
    ok("Space published to node")

    # Wait for sync
    step("Waiting 5s for sync...")
    time.sleep(5)

    # Open Service Provider app
    step("Opening Service Provider app...")
    call("provider", "open_app", {"page_id": page_id, "app_name": "Service Provider"})

    if not wait_for_eval("provider"):
        fail("Provider app failed to load")
        sys.exit(1)
    ok("Service Provider app loaded")

    # Set up schedule
    step("Setting up Monday schedule...")
    call("provider", "eval", {"code": 'set_schedule_via_ui("monday", "09:00", "17:00", 60)'})
    ok("Monday schedule set")

    # Check provider apps
    provider_apps = call("provider", "list_apps", {"page_id": page_id})
    provider_app_names = [a["name"] for a in (provider_apps.get("result", {}).get("apps") or provider_apps.get("apps", []))]
    info(f"Provider sees apps: {provider_app_names}")

    # Get viewer link
    header("GETTING VIEWER LINK")
    step("Getting shareable link via provider...")
    link_result = call("provider", "get_shareable_link", {"space_id": space_id, "node_id": node_id})
    viewer_link = link_result.get("result", {}).get("connection_string") or link_result.get("connection_string")
    if viewer_link:
        ok(f"Got viewer link: {viewer_link[:60]}...")
    else:
        fail("Failed to get viewer link")
        sys.exit(1)

    # ============================================================
    # SETUP CUSTOMER
    # ============================================================
    header("SETTING UP CUSTOMER")

    step("Signing up customer...")
    try:
        call("customer", "sign_up", {"username": "customer", "passphrase": PASSPHRASE})
        ok("Customer signed up")
    except RuntimeError as e:
        if "already" in str(e).lower():
            info("Customer already signed up")
        else:
            raise

    step("Logging in customer...")
    call("customer", "login", {"passphrase": PASSPHRASE})
    ok("Customer logged in")

    # Wait for P2P
    step("Waiting for P2P discovery (10s)...")
    time.sleep(10)

    # Connect via viewer link
    step("Connecting customer to node via viewer link...")
    for attempt in range(3):
        try:
            call("customer", "add_website", {"connection_string": viewer_link}, timeout=90)
            ok("Customer connected to node")
            break
        except RuntimeError as e:
            if attempt < 2:
                info(f"Attempt {attempt + 1} failed, retrying in 5s...")
                time.sleep(5)
            else:
                raise

    # Wait for sync
    step("Waiting 5s for customer to receive data...")
    time.sleep(5)

    # ============================================================
    # VERIFY CUSTOMER APPS
    # ============================================================
    header("VERIFYING CUSTOMER APPS")

    cust_spaces = call("customer", "list_spaces")
    spaces = cust_spaces.get("result", {}).get("spaces") or cust_spaces.get("spaces", [])
    if not spaces:
        fail("Customer has no spaces")
        sys.exit(1)
    cust_space_id = spaces[0]["id"]
    ok(f"Customer has space: {spaces[0]['name']}")

    cust_pages = call("customer", "list_pages", {"space_id": cust_space_id})
    pages_list = cust_pages.get("result", {}).get("pages") or cust_pages.get("pages", [])
    cust_page_id = pages_list[0]["id"]

    # THE KEY CHECK!
    cust_apps = call("customer", "list_apps", {"page_id": cust_page_id})
    cust_app_names = [a["name"] for a in (cust_apps.get("result", {}).get("apps") or cust_apps.get("apps", []))]

    print()
    print(f"  Provider apps: {provider_app_names}")
    print(f"  Customer apps: {cust_app_names}")
    print()

    # Verify customer only sees Service Customer
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

    # ============================================================
    # SUMMARY
    # ============================================================
    header("TEST COMPLETE")

    print(f"  Space ID: {space_id}")
    print(f"  Page ID:  {page_id}")
    print()
    print("  Provider apps:", provider_app_names)
    print("  Customer apps:", cust_app_names)
    print()
    print("  Both apps are now running in tmux session 'booking'")
    print("  Use: tmux attach -t booking")
    print()


if __name__ == "__main__":
    main()
