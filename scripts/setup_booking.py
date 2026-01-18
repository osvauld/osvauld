#!/usr/bin/env python3
"""Booking App Setup and Test

Sets up the booking service with proper node-based flow:
1. Provider creates space with booking app
2. Publishes to kunki node
3. Customer connects through node (gets correct permits)
4. Verifies customer only sees Service Customer app

Usage:
    ./scripts/setup_booking.py [--db-dir /tmp/booking_test]
    ./scripts/setup_booking.py --skip-setup  # Use existing DBs
"""

import subprocess
import socket
import json
import time
import sys
import os
from pathlib import Path

# Paths
SCRIPT_DIR = Path(__file__).parent
ROOT_DIR = SCRIPT_DIR.parent
SHELL_BINARY = ROOT_DIR / "target" / "debug" / "slint_shell"
KUNKI_BINARY = ROOT_DIR / "target" / "debug" / "kunki"
SAMPLE_APP_PATH = ROOT_DIR / "sample_apps" / "my-booking"

# Config
DB_DIR = "/tmp/booking_test"
PASSPHRASE = "test123"
SYNC_WAIT = 5  # seconds to wait for sync


def send_command(sock_path, cmd, timeout=30):
    """Send JSON command to debug socket."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        s.connect(sock_path)
        s.settimeout(timeout)
        s.send((json.dumps(cmd) + '\n').encode())
        response = s.recv(65536).decode()
        return json.loads(response)
    finally:
        s.close()


def call(sock_path, method, params=None, timeout=30):
    """Call debug socket method."""
    cmd = {"method": method, "id": 1}
    if params:
        cmd["params"] = params
    response = send_command(sock_path, cmd, timeout=timeout)
    if "error" in response:
        raise RuntimeError(f"{method}: {response['error']}")
    return response.get("result")


def wait_for_socket(sock_path, timeout=15):
    """Wait for socket to be ready."""
    start = time.time()
    while time.time() - start < timeout:
        if os.path.exists(sock_path):
            try:
                call(sock_path, "ping")
                return True
            except:
                pass
        time.sleep(0.3)
    return False


def wait_for_eval(sock_path, timeout=20):
    """Wait for eval to be available (app loaded)."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            result = call(sock_path, "eval", {"code": "return 1"})
            if result == 1:
                return True
        except:
            pass
        time.sleep(0.5)
    return False


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


class ProcessManager:
    def __init__(self):
        self.processes = {}

    def start(self, name, cmd, env=None):
        """Start process WITHOUT capturing stdout (visible output)."""
        full_env = os.environ.copy()
        if env:
            full_env.update(env)
        proc = subprocess.Popen(cmd, env=full_env)
        self.processes[name] = proc
        return proc

    def stop_all(self):
        for name, proc in self.processes.items():
            proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()
        self.processes.clear()


def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--db-dir", default=DB_DIR)
    parser.add_argument("--skip-setup", action="store_true", help="Skip fresh setup")
    parser.add_argument("--keep-alive", action="store_true", help="Keep shells running")
    args = parser.parse_args()

    db_dir = args.db_dir

    # Check binaries
    for binary in [SHELL_BINARY, KUNKI_BINARY]:
        if not binary.exists():
            print(f"Error: {binary} not found")
            print("Run: cargo build -p slint_shell -p kunki")
            sys.exit(1)

    if not SAMPLE_APP_PATH.exists():
        print(f"Error: Sample app not found at {SAMPLE_APP_PATH}")
        sys.exit(1)

    pm = ProcessManager()
    kunki_sock = f"{db_dir}/kunki.sock"
    provider_sock = f"{db_dir}/provider.sock"
    customer_sock = f"{db_dir}/customer.sock"

    try:
        # ============================================================
        # SETUP
        # ============================================================
        if not args.skip_setup:
            header("SETTING UP FRESH DATABASES")
            subprocess.run(["rm", "-rf", db_dir], check=True)
            os.makedirs(db_dir, exist_ok=True)
            ok(f"Created {db_dir}")
        else:
            info("Skipping setup (--skip-setup)")
            for sock in [kunki_sock, provider_sock, customer_sock]:
                if os.path.exists(sock):
                    os.remove(sock)

        # ============================================================
        # START KUNKI (NODE)
        # ============================================================
        header("STARTING KUNKI (NODE)")

        if not args.skip_setup:
            # Initialize kunki first
            step("Initializing kunki...")
            result = subprocess.run([
                str(KUNKI_BINARY),
                "-d", f"{db_dir}/kunki",
                "init", "-p", PASSPHRASE, "-u", "kunki"
            ], capture_output=True, text=True)
            if result.returncode != 0:
                fail(f"Kunki init failed: {result.stderr}")
                sys.exit(1)
            ok("Kunki initialized")

        pm.start("kunki", [
            str(KUNKI_BINARY),
            "-d", f"{db_dir}/kunki",
            "start", "-p", PASSPHRASE,
            "--debug-socket", kunki_sock,
        ])

        if not wait_for_socket(kunki_sock, timeout=15):
            fail("Kunki failed to start")
            sys.exit(1)
        ok("Kunki started and listening")

        # Wait for iroh discovery
        step("Waiting for iroh discovery to propagate (15s)...")
        time.sleep(15)

        # Get kunki's connection string
        kunki_conn = call(kunki_sock, "get_connection_string")
        info(f"Kunki connection string ready")

        # ============================================================
        # START PROVIDER (OWNER)
        # ============================================================
        header("STARTING PROVIDER SHELL")

        pm.start("provider", [
            str(SHELL_BINARY),
            "-d", "provider",
            "--debug-socket", provider_sock,
        ], env={"STHALAM_DATA_DIR": db_dir})

        if not wait_for_socket(provider_sock):
            fail("Provider shell failed to start")
            sys.exit(1)
        ok("Provider shell started")

        if not args.skip_setup:
            # Signup provider
            step("Signing up provider...")
            call(provider_sock, "sign_up", {"username": "provider", "passphrase": PASSPHRASE})
            ok("Provider signed up")

        # Login
        step("Logging in provider...")
        call(provider_sock, "login", {"passphrase": PASSPHRASE})
        ok("Provider logged in")

        # Wait for P2P
        time.sleep(2)

        if not args.skip_setup:
            # Add kunki as node
            step("Adding kunki node...")
            call(provider_sock, "add_node", {"connection_string": kunki_conn["connection_string"]})
            ok("Kunki node added")

            # Wait for connection
            time.sleep(3)

            # Get node_id from list_nodes
            nodes = call(provider_sock, "list_nodes").get("nodes", [])
            if not nodes:
                fail("No nodes found after adding kunki")
                sys.exit(1)
            node_id = nodes[0]["node_id"]
            info(f"Connected to node: {node_id[:16]}...")

            # Create space
            step("Creating Booking Service space...")
            space = call(provider_sock, "create_space", {"name": "Booking Service", "template_path": str(SAMPLE_APP_PATH)})
            space_id = space["id"]
            ok(f"Space created: {space_id}")

            # Import booking app
            step("Importing my-booking page...")
            page = call(provider_sock, "import_page", {
                "space_id": space_id,
                "page_dir": str(SAMPLE_APP_PATH)
            })
            page_id = page["page_id"]
            ok(f"Page imported: {page_id}")
            ok(f"Apps: {page['apps']}")

            # Publish to node
            step("Publishing space to node...")
            call(provider_sock, "publish_space", {"space_id": space_id, "node_id": node_id})
            ok("Space published to node")

            # Wait for sync
            step(f"Waiting {SYNC_WAIT}s for sync...")
            time.sleep(SYNC_WAIT)
        else:
            # Get existing space/page/node
            spaces = call(provider_sock, "list_spaces").get("spaces", [])
            space_id = spaces[0]["id"]
            pages = call(provider_sock, "list_pages", {"space_id": space_id}).get("pages", [])
            page_id = pages[0]["id"]
            nodes = call(provider_sock, "list_nodes").get("nodes", [])
            node_id = nodes[0]["node_id"] if nodes else None
            info(f"Using existing space: {space_id}, page: {page_id}, node: {node_id[:16] if node_id else 'none'}...")

        # Open Service Provider app
        step("Opening Service Provider app...")
        call(provider_sock, "open_app", {"page_id": page_id, "app_name": "Service Provider"})

        if not wait_for_eval(provider_sock):
            fail("Provider app failed to load")
            sys.exit(1)
        ok("Service Provider app loaded")

        # Add some schedule via UI
        step("Setting up Monday schedule...")
        call(provider_sock, "eval", {"code": 'set_schedule_via_ui("monday", "09:00", "17:00", 60)'})
        ok("Monday schedule set")

        # Check provider apps
        provider_apps = call(provider_sock, "list_apps", {"page_id": page_id}).get("apps", [])
        app_names = [a["name"] for a in provider_apps]
        info(f"Provider sees apps: {app_names}")

        # ============================================================
        # GET VIEWER LINK VIA PROVIDER
        # ============================================================
        header("GETTING VIEWER LINK VIA PROVIDER")

        # Get viewer link via provider's UI (which gets it from the connected node)
        # The node generates the viewer permit with correct layer restrictions
        step("Getting shareable link via provider...")
        link_result = call(provider_sock, "get_shareable_link", {"space_id": space_id, "node_id": node_id})
        viewer_link = link_result.get("connection_string")
        if viewer_link:
            ok(f"Got viewer link: {viewer_link[:60]}...")
        else:
            fail("Failed to get viewer link")
            sys.exit(1)

        # ============================================================
        # START CUSTOMER
        # ============================================================
        header("STARTING CUSTOMER SHELL")

        pm.start("customer", [
            str(SHELL_BINARY),
            "-d", "customer",
            "--debug-socket", customer_sock,
        ], env={"STHALAM_DATA_DIR": db_dir})

        if not wait_for_socket(customer_sock):
            fail("Customer shell failed to start")
            sys.exit(1)
        ok("Customer shell started")

        if not args.skip_setup:
            step("Signing up customer...")
            call(customer_sock, "sign_up", {"username": "customer", "passphrase": PASSPHRASE})
            ok("Customer signed up")

        step("Logging in customer...")
        call(customer_sock, "login", {"passphrase": PASSPHRASE})
        ok("Customer logged in")

        # Wait for P2P discovery
        step("Waiting for P2P discovery (5s)...")
        time.sleep(5)

        # Connect via viewer link (through node) - retry a few times
        step("Connecting customer to node via viewer link...")
        max_retries = 3
        for attempt in range(max_retries):
            try:
                call(customer_sock, "add_website", {"connection_string": viewer_link}, timeout=90)
                ok("Customer connected to node")
                break
            except RuntimeError as e:
                if attempt < max_retries - 1:
                    info(f"Connection attempt {attempt + 1} failed, retrying in 5s...")
                    time.sleep(5)
                else:
                    raise e

        # Wait for sync
        step(f"Waiting {SYNC_WAIT}s for customer to receive data...")
        time.sleep(SYNC_WAIT)

        # ============================================================
        # VERIFY CUSTOMER APPS - THE KEY CHECK!
        # ============================================================
        header("VERIFYING CUSTOMER APPS")

        cust_spaces = call(customer_sock, "list_spaces").get("spaces", [])
        if not cust_spaces:
            fail("Customer has no spaces")
            sys.exit(1)
        cust_space_id = cust_spaces[0]["id"]
        ok(f"Customer has space: {cust_spaces[0]['name']}")

        cust_pages = call(customer_sock, "list_pages", {"space_id": cust_space_id}).get("pages", [])
        cust_page_id = cust_pages[0]["id"]

        # THIS IS THE KEY CHECK!
        cust_apps = call(customer_sock, "list_apps", {"page_id": cust_page_id}).get("apps", [])
        cust_app_names = [a["name"] for a in cust_apps]

        print()
        print(f"  Provider apps: {app_names}")
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
        call(customer_sock, "open_app", {"page_id": cust_page_id, "app_name": "Service Customer"})

        if not wait_for_eval(customer_sock):
            fail("Customer app failed to load")
            sys.exit(1)
        ok("Service Customer app loaded")

        # ============================================================
        # SUMMARY
        # ============================================================
        header("SETUP COMPLETE")

        print(f"  Provider socket: {provider_sock}")
        print(f"  Customer socket: {customer_sock}")
        print(f"  Kunki socket:    {kunki_sock}")
        print()
        print(f"  Space ID: {space_id}")
        print(f"  Page ID:  {page_id}")
        print()
        print("  Provider apps:", app_names)
        print("  Customer apps:", cust_app_names)
        print()

        if args.keep_alive:
            print("  Shells kept alive. Press Ctrl+C to stop.\n")
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
        if not args.keep_alive:
            pm.stop_all()


if __name__ == "__main__":
    main()
