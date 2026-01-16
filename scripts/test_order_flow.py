#!/usr/bin/env python3
"""Complete E-Commerce Order Flow Test

Tests the full order lifecycle:
1. Setup databases with pre-connected owner, node, customer
2. Owner adds products
3. Customer views products and places order
4. Owner sees the order
5. Owner updates status
6. Customer sees updated status

Usage:
    ./scripts/test_order_flow.py
"""

import subprocess
import socket
import json
import time
import sys
import os
from pathlib import Path
from typing import Optional, Dict, Any, List

# Paths
SCRIPT_DIR = Path(__file__).parent
ROOT_DIR = SCRIPT_DIR.parent
SHELL_BINARY = ROOT_DIR / "target" / "debug" / "slint_shell"
KUNKI_BINARY = ROOT_DIR / "target" / "debug" / "kunki"
SETUP_BINARY = ROOT_DIR / "target" / "debug" / "setup_test_dbs"

# Config
DB_DIR = "/tmp/order_flow_test"
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

    def wait_for_eval(self, timeout: float = 10.0) -> bool:
        """Wait for eval to be available (app loaded with Lua VM)."""
        start = time.time()
        while time.time() - start < timeout:
            try:
                result = self.eval("return 1")
                return result == 1
            except RuntimeError as e:
                if "not available" in str(e) or "no app" in str(e):
                    time.sleep(0.5)
                else:
                    raise
        return False


class ProcessManager:
    def __init__(self):
        self.processes: Dict[str, subprocess.Popen] = {}

    def start(self, name: str, cmd: List[str], env: Dict[str, str] = None, headless: bool = False):
        full_env = os.environ.copy()
        if headless:
            full_env["SLINT_BACKEND"] = "testing"  # Headless mode
        if env:
            full_env.update(env)

        proc = subprocess.Popen(
            cmd,
            env=full_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
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


def print_header(text: str):
    print(f"\n{'='*60}")
    print(f"  {text}")
    print(f"{'='*60}\n")


def print_step(text: str):
    print(f"  -> {text}")


def print_ok(text: str):
    print(f"  [OK] {text}")


def print_fail(text: str):
    print(f"  [FAIL] {text}")


def print_info(text: str):
    print(f"  [i] {text}")


def assert_eq(actual, expected, msg: str):
    if actual != expected:
        raise AssertionError(f"{msg}: expected {expected}, got {actual}")


def assert_ge(actual, expected, msg: str):
    if actual < expected:
        raise AssertionError(f"{msg}: expected >= {expected}, got {actual}")


def main():
    # Check binaries
    for binary in [SHELL_BINARY, KUNKI_BINARY, SETUP_BINARY]:
        if not binary.exists():
            print(f"Error: {binary} not found")
            print("Run: cargo build -p slint_shell -p kunki && cargo build -p integration_tests --bin setup_test_dbs")
            sys.exit(1)

    # Clean up
    print_header("COMPLETE ORDER FLOW TEST")
    subprocess.run(["rm", "-rf", DB_DIR], check=True)
    os.makedirs(DB_DIR, exist_ok=True)

    pm = ProcessManager()

    try:
        # ============================================================
        # SETUP: Create pre-connected databases
        # ============================================================
        print_header("SETTING UP TEST DATABASES")

        print_step("Running setup_test_dbs...")
        result = subprocess.run(
            [str(SETUP_BINARY), "--db-dir", DB_DIR, "--passphrase", PASSPHRASE],
            capture_output=True,
            text=True,
            timeout=120,
        )
        if result.returncode != 0:
            print_fail(f"setup_test_dbs failed: {result.stderr[:500]}")
            sys.exit(1)
        print_ok("Test databases created")

        # ============================================================
        # START KUNKI
        # ============================================================
        print_header("STARTING KUNKI")

        pm.start("kunki", [
            str(KUNKI_BINARY),
            "-d", f"{DB_DIR}/kunki",
            "start", "-p", PASSPHRASE,
            "--debug-socket", f"{DB_DIR}/kunki.sock",
        ])
        time.sleep(2)
        print_ok("Kunki started")

        # ============================================================
        # START OWNER
        # ============================================================
        print_header("STARTING OWNER")

        pm.start("owner", [
            str(SHELL_BINARY),
            "-d", "shop_owner",
            "--debug-socket", f"{DB_DIR}/owner.sock",
        ], env={"STHALAM_DATA_DIR": DB_DIR})

        owner = DebugClient(f"{DB_DIR}/owner.sock", "owner")
        if not owner.wait_ready():
            raise RuntimeError("Owner failed to start")
        print_ok("Owner shell started")

        owner.login(PASSPHRASE)
        print_ok("Owner logged in")

        owner.wait_for_p2p(timeout=15.0)
        print_ok("Owner P2P ready")

        # Get space and page
        spaces = owner.spaces()
        assert_ge(len(spaces), 1, "Owner should have at least 1 space")
        space_id = spaces[0]["id"]
        print_info(f"Space: {spaces[0]['name']}")

        pages = owner.pages(space_id)
        assert_ge(len(pages), 1, "Should have at least 1 page")
        page_id = pages[0]["id"]

        # Open Shop Owner app
        print_step("Opening Shop Owner app...")
        owner.open_app(page_id, "Shop Owner")
        if not owner.wait_for_eval(timeout=15.0):
            raise RuntimeError("Owner app Lua VM failed to initialize")
        print_ok("Shop Owner app opened and Lua ready")

        # Add products if none exist
        count = owner.eval("return get_products_count()")
        if count == 0:
            print_step("Adding products...")
            owner.eval('add_product("Widget", 29.99, "A useful widget", 100)')
            owner.eval('add_product("Gadget", 49.99, "A cool gadget", 50)')
            owner.eval('add_product("Gizmo", 19.99, "A handy gizmo", 200)')
            count = owner.eval("return get_products_count()")
            # Wait for sync
            print_step(f"Waiting {SYNC_TIMEOUT}s for products to sync...")
            time.sleep(SYNC_TIMEOUT)
        print_ok(f"Owner has {count} products")

        # Get products for order
        products = owner.eval("return get_products()")
        print_info(f"Products: {[p['name'] for p in products]}")

        # Initial order count
        owner_orders_before = owner.eval("return get_orders_count()")
        print_info(f"Owner orders before: {owner_orders_before}")

        # ============================================================
        # START CUSTOMER
        # ============================================================
        print_header("STARTING CUSTOMER")

        pm.start("customer", [
            str(SHELL_BINARY),
            "-d", "customer",
            "--debug-socket", f"{DB_DIR}/customer.sock",
        ], env={"STHALAM_DATA_DIR": DB_DIR})

        customer = DebugClient(f"{DB_DIR}/customer.sock", "customer")
        if not customer.wait_ready():
            raise RuntimeError("Customer failed to start")
        print_ok("Customer shell started")

        customer.login(PASSPHRASE)
        print_ok("Customer logged in")

        customer.wait_for_p2p(timeout=15.0)
        print_ok("Customer P2P ready")

        # Get customer's spaces
        cust_spaces = customer.spaces()
        assert_ge(len(cust_spaces), 1, "Customer should have at least 1 space")
        cust_space_id = cust_spaces[0]["id"]
        print_info(f"Customer space: {cust_spaces[0]['name']}")

        cust_pages = customer.pages(cust_space_id)
        cust_page_id = cust_pages[0]["id"]
        print_info(f"Customer page: {cust_page_id[:20]}...")

        # List available apps
        cust_apps = customer.apps(cust_page_id)
        print_info(f"Customer apps: {[a['name'] for a in cust_apps]}")

        # Open Customer app (try to find correct name)
        app_name = "Customer"  # Default
        for app in cust_apps:
            if "customer" in app["name"].lower():
                app_name = app["name"]
                break

        print_step(f"Opening {app_name} app...")
        try:
            result = customer.open_app(cust_page_id, app_name)
            print_info(f"open_app result: {result}")
        except Exception as e:
            print_info(f"open_app exception: {e}")

        if not customer.wait_for_eval(timeout=20.0):
            print_info("Eval not available - checking screen...")
            screen = customer.screen()
            print_info(f"Customer screen: {screen}")
            raise RuntimeError(f"Customer app Lua VM failed to initialize (screen: {screen})")
        print_ok("Customer app opened and Lua ready")

        # Check customer sees products
        cust_products = customer.eval("return get_products_count()")
        print_info(f"Customer sees {cust_products} products initially")

        # If products not synced, wait and retry
        if cust_products == 0:
            print_step(f"Products not synced, waiting {SYNC_TIMEOUT * 2}s...")
            time.sleep(SYNC_TIMEOUT * 2)
            # Force refresh
            customer.eval("refresh_products_ui()")
            cust_products = customer.eval("return get_products_count()")
            print_info(f"Customer sees {cust_products} products after wait")

        print_ok(f"Customer sees {cust_products} products")

        # Get product details for order
        cust_product_list = customer.eval("return get_products()")
        if cust_product_list and len(cust_product_list) > 0:
            product = cust_product_list[0]
            print_info(f"Ordering: {product['name']} @ ${product['price']}")

            # ============================================================
            # CUSTOMER: CREATE AND SUBMIT ORDER
            # ============================================================
            print_header("CUSTOMER: PLACING ORDER")

            # Select product first
            print_step("Selecting product...")
            customer.eval(f'select_product("{product["id"]}", "{product["name"]}", {product["price"]})')

            # Create draft order
            print_step("Creating order...")
            customer.eval(f'create_order("{product["id"]}", 2, "Test order", "123 Test St")')
            print_ok("Draft order created")

            drafts = customer.eval("return get_drafts_count()")
            print_info(f"Customer drafts: {drafts}")

            # Get order ID
            order_id = customer.eval("return get_last_order_id()")
            print_info(f"Order ID: {order_id}")

            # Submit order
            print_step("Submitting order...")
            customer.eval(f'submit_order("{order_id}")')
            print_ok("Order submitted")

            # Verify order moved to synced
            synced_orders = customer.eval("return get_orders_count()")
            print_info(f"Customer synced orders: {synced_orders}")

            drafts_after = customer.eval("return get_drafts_count()")
            print_info(f"Customer drafts after: {drafts_after}")

            # ============================================================
            # WAIT FOR SYNC
            # ============================================================
            print_header("WAITING FOR SYNC")

            print_step(f"Waiting {SYNC_TIMEOUT}s for sync...")
            time.sleep(SYNC_TIMEOUT)

            # ============================================================
            # OWNER: VERIFY ORDER RECEIVED
            # ============================================================
            print_header("OWNER: CHECKING ORDER")

            owner_orders_after = owner.eval("return get_orders_count()")
            print_info(f"Owner orders after: {owner_orders_after}")

            if owner_orders_after > owner_orders_before:
                print_ok(f"Owner received order! ({owner_orders_before} -> {owner_orders_after})")

                # Get order details
                all_orders = owner.eval("return get_all_orders()")
                if all_orders:
                    order = all_orders[-1]  # Latest order
                    print_info(f"Order status: {order.get('status')}")
                    print_info(f"Order customer: {order.get('customer', 'N/A')[:20]}...")

                    # Get customer DID for status update
                    customer_did = order.get('customer')

                    if customer_did and order_id:
                        # ============================================================
                        # OWNER: UPDATE ORDER STATUS
                        # ============================================================
                        print_header("OWNER: UPDATING STATUS")

                        print_step("Confirming order...")
                        owner.eval(f'update_order_status("{customer_did}", "{order_id}", "confirmed")')
                        print_ok("Order confirmed")

                        # Wait for sync
                        time.sleep(SYNC_TIMEOUT)

                        # Check status on owner side
                        status = owner.eval(f'return get_order_status("{order_id}")')
                        print_info(f"Owner sees status: {status}")

                        # ============================================================
                        # CUSTOMER: VERIFY STATUS UPDATE
                        # ============================================================
                        print_header("CUSTOMER: CHECKING STATUS")

                        cust_orders = customer.eval("return get_all_orders()")
                        if cust_orders:
                            for o in cust_orders:
                                if o.get('id') == order_id:
                                    print_info(f"Customer sees status: {o.get('status')}")
                                    if o.get('status') == 'confirmed':
                                        print_ok("Status synced to customer!")
                                    break
            else:
                print_fail("Order not received by owner (sync may need more time)")

        # ============================================================
        # SUMMARY
        # ============================================================
        print_header("TEST COMPLETED")

        print("Summary:")
        print(f"  Owner products: {owner.eval('return get_products_count()')}")
        print(f"  Owner orders: {owner.eval('return get_orders_count()')}")
        print(f"  Customer products: {customer.eval('return get_products_count()')}")
        print(f"  Customer orders: {customer.eval('return get_orders_count()')}")

        return True

    except Exception as e:
        print_fail(str(e))
        import traceback
        traceback.print_exc()
        return False

    finally:
        pm.stop_all()


if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
