#!/usr/bin/env python3
"""Test Canvas App Cursor Sync via AI Interface

Tests live cursor sync between two users via QUIC datagrams.
Assumes ai_interface is running with:
    ./ai_interface --name cursor --instances provider,consumer --node --show-ui

Usage:
    ./scripts/test_cursor_sync.py
"""

import socket
import json
import time
import sys

AI_SOCKET = "/tmp/sthalam/cursor/ai.sock"
SAMPLE_APP_PATH = "/home/abe/osvauld/sample_apps/canvas-app"
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
    result = response.get("result", {})
    # Handle nested result structure
    if isinstance(result, dict) and "result" in result:
        result = result["result"]
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


def wait_for_eval(target, timeout=20):
    """Wait for eval to be available (app loaded)."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            result = call(target, "eval", {"code": "return 1"})
            # Handle nested result
            val = result.get("result") if isinstance(result, dict) else result
            if val == 1:
                return True
        except:
            pass
        time.sleep(0.5)
    return False


def main():
    header("CHECKING AI INTERFACE")

    try:
        status = send({"action": "status", "id": 0})
        info(f"Connected to cursor sync session")
        info(f"Instances: {list(status.keys())}")
    except Exception as e:
        fail(f"Cannot connect to AI interface: {e}")
        print("\nMake sure ai_interface is running:")
        print("  ./target/debug/ai_interface --name cursor --instances provider,consumer --node --show-ui")
        sys.exit(1)

    # ============================================================
    # SETUP PROVIDER
    # ============================================================
    header("SETTING UP PROVIDER")

    step("Signing up provider...")
    try:
        call("provider", "sign_up", {"username": "canvas_owner", "passphrase": PASSPHRASE})
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
    nodes_result = call("provider", "list_nodes")
    nodes = nodes_result.get("result", {}).get("nodes") or nodes_result.get("nodes", [])
    if not nodes:
        fail("No nodes found")
        sys.exit(1)
    node_id = nodes[0]["node_id"]
    info(f"Connected to node: {node_id[:16]}...")

    # Create space
    step("Creating Canvas space...")
    call("provider", "create_space", {"name": "Canvas Collaboration", "template_path": SAMPLE_APP_PATH})

    # Wait for creation and get the actual space ID from list_spaces
    time.sleep(1)
    spaces_result = call("provider", "list_spaces")
    spaces = spaces_result.get("result", {}).get("spaces") or spaces_result.get("spaces", [])
    # Find space by name
    space_id = None
    for s in spaces:
        if s.get("name") == "Canvas Collaboration":
            space_id = s.get("id")
            break
    if not space_id and spaces:
        space_id = spaces[-1].get("id")
    if not space_id:
        fail("No spaces found")
        sys.exit(1)
    ok(f"Space created: {space_id[:16]}...")

    # Import canvas page
    step("Importing canvas page...")
    page = call("provider", "import_page", {
        "space_id": space_id,
        "page_dir": SAMPLE_APP_PATH
    })
    page_id = page.get("result", {}).get("page_id") or page.get("page_id")
    apps = page.get("result", {}).get("apps") or page.get("apps")
    ok(f"Page imported: {page_id}")
    ok(f"Apps: {apps}")

    # Publish space
    step("Publishing space to node...")
    call("provider", "publish_space", {"space_id": space_id, "node_id": node_id}, timeout=60)
    ok("Space published")

    # Get shareable link
    step("Getting shareable link...")
    link_result = call("provider", "get_shareable_link", {"space_id": space_id, "node_id": node_id}, timeout=30)
    viewer_link = link_result.get("result", {}).get("connection_string") or link_result.get("connection_string")
    if not viewer_link:
        fail(f"No shareable link returned: {link_result}")
        sys.exit(1)
    ok(f"Got link: {viewer_link[:60]}...")

    # ============================================================
    # SETUP CONSUMER
    # ============================================================
    header("SETTING UP CONSUMER")

    step("Signing up consumer...")
    try:
        call("consumer", "sign_up", {"username": "canvas_viewer", "passphrase": PASSPHRASE})
        ok("Consumer signed up")
    except RuntimeError as e:
        if "already" in str(e).lower():
            info("Consumer already signed up")
        else:
            raise

    step("Logging in consumer...")
    call("consumer", "login", {"passphrase": PASSPHRASE})
    ok("Consumer logged in")

    # Wait for P2P
    step("Waiting for P2P discovery (10s)...")
    time.sleep(10)

    # Connect via viewer link
    step("Connecting consumer to node via viewer link...")
    for attempt in range(3):
        try:
            call("consumer", "add_website", {"connection_string": viewer_link}, timeout=90)
            ok("Consumer connected to node")
            break
        except RuntimeError as e:
            if attempt < 2:
                info(f"Attempt {attempt + 1} failed, retrying in 5s...")
                time.sleep(5)
            else:
                raise

    # Wait for sync
    step("Waiting 5s for consumer to receive data...")
    time.sleep(5)

    # Verify consumer has the space
    consumer_spaces = call("consumer", "list_spaces")
    c_spaces = consumer_spaces.get("result", {}).get("spaces") or consumer_spaces.get("spaces", [])
    if not c_spaces:
        fail("Consumer didn't receive space")
        sys.exit(1)
    ok(f"Consumer has {len(c_spaces)} space(s)")

    # Get consumer's space ID
    consumer_space_id = c_spaces[0].get("id")

    # ============================================================
    # OPEN CANVAS APP
    # ============================================================
    header("OPENING CANVAS APP")

    # Open app on provider (using page_id from import_page)
    step("Opening canvas on provider...")
    call("provider", "open_app", {"page_id": page_id, "app_name": "Canvas"})

    # Wait for app to load
    step("Waiting for provider app to load...")
    if not wait_for_eval("provider", timeout=30):
        fail("Provider app didn't load")
        sys.exit(1)
    ok("Provider canvas loaded")

    # Get consumer's page
    consumer_pages = call("consumer", "list_pages", {"space_id": consumer_space_id})
    cp_list = consumer_pages.get("result", {}).get("pages") or consumer_pages.get("pages", [])
    consumer_page_id = cp_list[0].get("id") if cp_list else page_id

    # Open app on consumer
    step("Opening canvas on consumer...")
    call("consumer", "open_app", {"page_id": consumer_page_id, "app_name": "Canvas"})

    # Wait for app to load
    step("Waiting for consumer app to load...")
    if not wait_for_eval("consumer", timeout=30):
        fail("Consumer app didn't load")
        sys.exit(1)
    ok("Consumer canvas loaded")

    # ============================================================
    # TEST CURSOR SYNC
    # ============================================================
    header("TESTING CURSOR SYNC")

    step("Simulating provider pointer move...")
    # Simulate pointer movement on provider
    call("provider", "eval", {"code": "on_pointer_event('move', 200, 150); return 'moved'"})
    time.sleep(0.1)
    call("provider", "eval", {"code": "on_pointer_event('move', 300, 200); return 'moved'"})
    time.sleep(0.1)
    call("provider", "eval", {"code": "on_pointer_event('move', 400, 250); return 'moved'"})
    ok("Sent cursor movements")

    # Wait for datagrams to propagate
    step("Waiting for cursor sync...")
    time.sleep(2)

    # Check if consumer received cursors
    step("Checking consumer for remote cursors...")
    result = call("consumer", "eval", {"code": """
        local count = 0
        for _ in pairs(remote_cursors) do
            count = count + 1
        end
        return count
    """})
    cursor_count = result.get("result", 0)

    if cursor_count > 0:
        ok(f"Consumer sees {cursor_count} remote cursor(s)!")

        # Get cursor details
        details = call("consumer", "eval", {"code": """
            local cursors = {}
            for did, cursor in pairs(remote_cursors) do
                table.insert(cursors, {
                    did = did,
                    x = cursor.x,
                    y = cursor.y,
                    color = cursor.color
                })
            end
            return cursors
        """})
        cursor_data = details.get("result", [])
        for c in cursor_data:
            info(f"  Cursor: x={c.get('x')}, y={c.get('y')}, color={c.get('color')}")

        header("SUCCESS!")
        print("  Cursor sync is working!")
        print("  Provider's cursor position is visible to consumer.")
    else:
        fail("No remote cursors received")
        info("This could be due to:")
        info("  - Datagram transmission not working")
        info("  - on_remote_cursor callback not being called")
        info("  - Timing issues (cursors already stale)")

        # Debug: check if provider's cursor was sent
        step("Debug: Checking provider's last cursor send...")
        prov_result = call("provider", "eval", {"code": "return last_cursor_send"})
        info(f"Provider last_cursor_send: {prov_result.get('result')}")

    header("TEST COMPLETE")


if __name__ == "__main__":
    main()
