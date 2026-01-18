#!/usr/bin/env python3
"""Full E-Commerce Demo with Visible UI

Sets up test databases with all connections pre-established,
then runs owner, customer, and kunki with visible UIs.

Usage:
    ./scripts/run_full_demo.py

This will:
1. Run setup_test_dbs to create pre-connected databases
2. Start kunki (relay node)
3. Start owner shell with UI
4. Start customer shell with UI
5. Both can interact with synced data

Press Ctrl+C to stop all instances.
"""

import subprocess
import socket
import json
import time
import sys
import os
import signal
from pathlib import Path
from typing import Optional, Dict, Any, List

# Paths
SCRIPT_DIR = Path(__file__).parent
ROOT_DIR = SCRIPT_DIR.parent
SHELL_BINARY = ROOT_DIR / "target" / "debug" / "slint_shell"
KUNKI_BINARY = ROOT_DIR / "target" / "debug" / "kunki"
SETUP_BINARY = ROOT_DIR / "target" / "debug" / "setup_test_dbs"
SAMPLE_APP_PATH = ROOT_DIR / "sample_apps" / "my-shop"

# Config
DB_DIR = "/tmp/full_demo"
PASSPHRASE = "test123"

# Timeouts
STARTUP_TIMEOUT = 8.0
COMMAND_TIMEOUT = 15.0
APP_LOAD_TIMEOUT = 3.0
SYNC_TIMEOUT = 5.0


class DebugClient:
    """Client for debug socket communication."""

    def __init__(self, socket_path: str, name: str = ""):
        self.socket_path = socket_path
        self.name = name

    def wait_ready(self, timeout: float = STARTUP_TIMEOUT) -> bool:
        """Wait for socket to be ready."""
        start = time.time()
        while time.time() - start < timeout:
            if os.path.exists(self.socket_path):
                try:
                    if self.ping():
                        return True
                except:
                    pass
            time.sleep(0.2)
        return False

    def send(self, cmd: Dict[str, Any]) -> Dict[str, Any]:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            sock.connect(self.socket_path)
            sock.settimeout(COMMAND_TIMEOUT)
            sock.send((json.dumps(cmd) + '\n').encode())
            response = sock.recv(65536).decode()
            return json.loads(response)
        finally:
            sock.close()

    def call(self, method: str, params: Optional[Dict] = None) -> Any:
        cmd = {"method": method, "id": 1}
        if params:
            cmd["params"] = params
        response = self.send(cmd)
        if "error" in response:
            err = response["error"]
            msg = err.get("message", str(err)) if isinstance(err, dict) else str(err)
            raise RuntimeError(f"[{self.name}] {method}: {msg}")
        return response.get("result")

    def ping(self) -> bool:
        return self.call("ping").get("status") == "ok"

    def login(self, passphrase: str):
        return self.call("login", {"passphrase": passphrase})

    def screen(self) -> str:
        return self.call("ui_get_screen").get("screen", "")

    def spaces(self):
        return self.call("list_spaces").get("spaces", [])

    def pages(self, space_id: str):
        return self.call("list_pages", {"space_id": space_id}).get("pages", [])

    def apps(self, page_id: str):
        return self.call("list_apps", {"page_id": page_id}).get("apps", [])

    def open_app(self, page_id: str, app_name: str):
        return self.call("open_app", {"page_id": page_id, "app_name": app_name})

    def eval(self, code: str):
        return self.call("eval", {"code": code})

    def p2p_status(self) -> Dict:
        return self.call("p2p_status")

    def wait_for_p2p(self, timeout: float = 10.0) -> bool:
        """Wait for P2P to be initialized (courier ready)."""
        start = time.time()
        while time.time() - start < timeout:
            try:
                status = self.p2p_status()
                if status.get("p2p_ready"):
                    return True
            except:
                pass
            time.sleep(0.5)
        return False


class ProcessManager:
    """Manages child processes."""

    def __init__(self):
        self.processes: Dict[str, subprocess.Popen] = {}

    def start(self, name: str, cmd: List[str], env: Dict[str, str] = None):
        full_env = os.environ.copy()
        if env:
            full_env.update(env)

        print(f"  Starting {name}...")
        proc = subprocess.Popen(
            cmd,
            env=full_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.processes[name] = proc
        return proc

    def stop_all(self):
        print("\n  Stopping all processes...")
        for name, proc in self.processes.items():
            print(f"    Stopping {name}...")
            proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()
        self.processes.clear()


def print_header(text: str):
    print(f"\n{'='*60}")
    print(f"  {text}")
    print(f"{'='*60}\n")


def print_step(text: str):
    print(f"  -> {text}")


def print_ok(text: str):
    print(f"  [OK] {text}")


def print_info(text: str):
    print(f"  [i] {text}")


def print_warn(text: str):
    print(f"  [!] {text}")


def main():
    # Check binaries
    if not SHELL_BINARY.exists():
        print(f"Error: {SHELL_BINARY} not found. Run: cargo build -p slint_shell")
        sys.exit(1)

    if not KUNKI_BINARY.exists():
        print(f"Error: {KUNKI_BINARY} not found. Run: cargo build -p kunki")
        sys.exit(1)

    if not SETUP_BINARY.exists():
        print(f"Error: {SETUP_BINARY} not found.")
        print("Run: cargo build -p integration_tests --bin setup_test_dbs")
        sys.exit(1)

    if not SAMPLE_APP_PATH.exists():
        print(f"Error: Sample app not found at {SAMPLE_APP_PATH}")
        sys.exit(1)

    # Clean up
    print_header("FULL E-COMMERCE DEMO WITH SYNC")
    print_step(f"Cleaning {DB_DIR}...")
    subprocess.run(["rm", "-rf", DB_DIR], check=True)
    os.makedirs(DB_DIR, exist_ok=True)

    pm = ProcessManager()

    try:
        # ============================================================
        # RUN SETUP_TEST_DBS
        # ============================================================
        print_header("SETTING UP TEST DATABASES")

        print_step("Running setup_test_dbs to create pre-connected databases...")
        result = subprocess.run(
            [str(SETUP_BINARY), "--db-dir", DB_DIR, "--passphrase", PASSPHRASE],
            capture_output=True,
            text=True,
            timeout=120,
        )
        if result.returncode != 0:
            print(f"  setup_test_dbs failed!")
            print(f"  stdout: {result.stdout[-1000:]}")
            print(f"  stderr: {result.stderr[-1000:]}")
            raise RuntimeError("Failed to setup test databases")
        print_ok("Test databases created with all connections established")

        # Parse output to get space/page IDs
        space_id = None
        page_id = None
        for line in result.stdout.split('\n'):
            if line.startswith("Space:"):
                parts = line.split('(')
                if len(parts) > 1:
                    space_id = parts[1].rstrip(')')
            elif line.startswith("Page:"):
                parts = line.split('(')
                if len(parts) > 1:
                    page_id = parts[1].rstrip(')')

        if space_id:
            print_info(f"Space ID: {space_id[:20]}...")
        if page_id:
            print_info(f"Page ID: {page_id[:20]}...")

        # ============================================================
        # START KUNKI (NODE)
        # ============================================================
        print_header("STARTING KUNKI NODE")

        pm.start("kunki", [
            str(KUNKI_BINARY),
            "-d", f"{DB_DIR}/kunki",
            "start",
            "-p", PASSPHRASE,
            "--debug-socket", f"{DB_DIR}/kunki.sock",
        ])
        time.sleep(2)  # Give kunki time to start
        print_ok("Kunki started")

        # ============================================================
        # START OWNER SHELL
        # ============================================================
        print_header("STARTING OWNER")

        pm.start("owner", [
            str(SHELL_BINARY),
            "-d", "shop_owner",
            "--debug-socket", f"{DB_DIR}/owner.sock",
        ], env={"STHALAM_DATA_DIR": DB_DIR})

        owner = DebugClient(f"{DB_DIR}/owner.sock", "owner")
        if not owner.wait_ready():
            raise RuntimeError("Owner shell failed to start")
        print_ok("Owner shell started")

        print_step("Logging in owner...")
        owner.login(PASSPHRASE)
        print_ok("Owner logged in")

        # Wait for P2P (needed for real-time sync)
        print_step("Waiting for P2P initialization...")
        if owner.wait_for_p2p(timeout=15.0):
            print_ok("Owner P2P ready")
        else:
            print_warn("Owner P2P not ready (pre-synced data still available)")

        # List spaces to verify setup
        owner_spaces = owner.spaces()
        print_ok(f"Owner has {len(owner_spaces)} space(s)")
        for s in owner_spaces:
            print_info(f"  - {s['name']} ({s['id'][:16]}...)")
            # Use the first space
            if not space_id:
                space_id = s['id']

        # Find page and open app
        if space_id:
            pages = owner.pages(space_id)
            if pages:
                page_id = pages[0]['id']
                print_ok(f"Found page: {page_id[:16]}...")

                print_step("Opening Shop Owner app...")
                owner.open_app(page_id, "Shop Owner")
                time.sleep(APP_LOAD_TIMEOUT)
                print_ok("Shop Owner app opened")

                # Check for products
                try:
                    count = owner.eval("return get_products_count()")
                    print_info(f"Current products: {count}")

                    if count == 0:
                        print_step("Adding initial products via UI...")
                        owner.eval('add_product_via_ui("Widget", 29.99, "A useful widget", 100)')
                        owner.eval('add_product_via_ui("Gadget", 49.99, "A cool gadget", 50)')
                        owner.eval('add_product_via_ui("Gizmo", 19.99, "A handy gizmo", 200)')
                        count = owner.eval("return get_products_count()")
                        print_ok(f"Added products (total: {count})")
                except Exception as e:
                    print_warn(f"Eval failed: {e}")

        # ============================================================
        # START CUSTOMER SHELL
        # ============================================================
        print_header("STARTING CUSTOMER")

        pm.start("customer", [
            str(SHELL_BINARY),
            "-d", "customer",
            "--debug-socket", f"{DB_DIR}/customer.sock",
        ], env={"STHALAM_DATA_DIR": DB_DIR})

        customer = DebugClient(f"{DB_DIR}/customer.sock", "customer")
        if not customer.wait_ready():
            raise RuntimeError("Customer shell failed to start")
        print_ok("Customer shell started")

        print_step("Logging in customer...")
        customer.login(PASSPHRASE)
        print_ok("Customer logged in")

        # Wait for P2P
        print_step("Waiting for P2P initialization...")
        if customer.wait_for_p2p(timeout=15.0):
            print_ok("Customer P2P ready")
        else:
            print_warn("Customer P2P not ready (pre-synced data still available)")

        # List customer spaces
        customer_spaces = customer.spaces()
        print_ok(f"Customer has {len(customer_spaces)} space(s)")
        for s in customer_spaces:
            print_info(f"  - {s['name']} ({s['id'][:16]}...)")

        # Open customer app if available
        if customer_spaces:
            cust_space_id = customer_spaces[0]['id']
            cust_pages = customer.pages(cust_space_id)
            if cust_pages:
                cust_page_id = cust_pages[0]['id']
                cust_apps = customer.apps(cust_page_id)
                print_info(f"Customer apps: {[a['name'] for a in cust_apps]}")

                # Open the Customer app if available
                customer_app_names = [a['name'] for a in cust_apps]
                if "Customer" in customer_app_names:
                    print_step("Opening Customer app...")
                    customer.open_app(cust_page_id, "Customer")
                    time.sleep(APP_LOAD_TIMEOUT)
                    print_ok("Customer app opened")

                    try:
                        cust_count = customer.eval("return get_products_count()")
                        print_info(f"Customer sees {cust_count} products")
                    except Exception as e:
                        print_warn(f"Customer eval failed: {e}")

        # ============================================================
        # DEMO READY
        # ============================================================
        print_header("DEMO READY!")

        print(f"""
  THREE WINDOWS SHOULD BE VISIBLE:

  OWNER WINDOW (shop_owner):
    - Shows Shop Owner app
    - Socket: {DB_DIR}/owner.sock

  CUSTOMER WINDOW (customer):
    - Shows Customer app with synced data
    - Socket: {DB_DIR}/customer.sock

  KUNKI (no UI):
    - Running as relay node
    - Socket: {DB_DIR}/kunki.sock

  TRY THESE COMMANDS:

  # Add more products (owner - via UI automation)
  ./scripts/dc {DB_DIR}/owner.sock eval 'add_product_via_ui("New Item", 99.99, "Brand new", 10)'

  # Check product count (owner)
  ./scripts/dc {DB_DIR}/owner.sock eval 'return get_products_count()'

  # Check product count (customer - should sync)
  ./scripts/dc {DB_DIR}/customer.sock eval 'return get_products_count()'

  # List spaces
  ./scripts/dc {DB_DIR}/owner.sock spaces
  ./scripts/dc {DB_DIR}/customer.sock spaces

  Press Ctrl+C to stop all instances.
""")

        # Keep running
        while True:
            time.sleep(1)

    except KeyboardInterrupt:
        print("\n  Interrupted by user")
    except Exception as e:
        print(f"\n  Error: {e}")
        import traceback
        traceback.print_exc()
    finally:
        pm.stop_all()


if __name__ == "__main__":
    main()
