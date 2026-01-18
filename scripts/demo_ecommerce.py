#!/usr/bin/env python3
"""E-Commerce Demo with Real P2P

Demonstrates the full e-commerce flow:
1. Owner adds products
2. Products sync to node and customer
3. Customer places order
4. Order syncs back to owner

Usage:
    ./scripts/demo_ecommerce.py [--db-dir /tmp/ecom_demo]
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
SETUP_BINARY = ROOT_DIR / "target" / "debug" / "setup_test_dbs"

# Config
DB_DIR = "/tmp/ecom_demo"
PASSPHRASE = "test123"
SYNC_WAIT = 5  # seconds to wait for sync


def send_command(sock_path, cmd, timeout=15):
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


def call(sock_path, method, params=None):
    """Call debug socket method."""
    cmd = {"method": method, "id": 1}
    if params:
        cmd["params"] = params
    response = send_command(sock_path, cmd)
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
        # Don't capture stdout/stderr - let it show in terminal
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
    parser.add_argument("--skip-setup", action="store_true", help="Skip setup_test_dbs")
    args = parser.parse_args()

    db_dir = args.db_dir

    # Check binaries
    for binary in [SHELL_BINARY, KUNKI_BINARY, SETUP_BINARY]:
        if not binary.exists():
            print(f"Error: {binary} not found")
            print("Run: cargo build -p slint_shell -p kunki && cargo build -p integration_tests --bin setup_test_dbs")
            sys.exit(1)

    pm = ProcessManager()
    kunki_sock = f"{db_dir}/kunki.sock"
    owner_sock = f"{db_dir}/owner.sock"
    customer_sock = f"{db_dir}/customer.sock"

    try:
        # ============================================================
        # SETUP
        # ============================================================
        if not args.skip_setup:
            header("SETTING UP TEST DATABASES")

            # Clean up
            subprocess.run(["rm", "-rf", db_dir], check=True)
            os.makedirs(db_dir, exist_ok=True)

            step("Running setup_test_dbs...")
            result = subprocess.run(
                [str(SETUP_BINARY), "--db-dir", db_dir, "--passphrase", PASSPHRASE],
                timeout=120,
            )
            if result.returncode != 0:
                fail("setup_test_dbs failed")
                sys.exit(1)
            ok("Databases created with owner↔node↔customer connections")
        else:
            info("Skipping setup (--skip-setup)")
            # Clean up old sockets
            for sock in [kunki_sock, owner_sock, customer_sock]:
                if os.path.exists(sock):
                    os.remove(sock)

        # ============================================================
        # START KUNKI
        # ============================================================
        header("STARTING KUNKI (NODE)")

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

        # Wait for iroh pkarr discovery to propagate
        step("Waiting for iroh discovery to propagate...")
        time.sleep(5)

        # ============================================================
        # START OWNER
        # ============================================================
        header("STARTING OWNER SHELL")

        pm.start("owner", [
            str(SHELL_BINARY),
            "-d", "shop_owner",
            "--debug-socket", owner_sock,
        ], env={"STHALAM_DATA_DIR": db_dir})

        if not wait_for_socket(owner_sock):
            fail("Owner shell failed to start")
            sys.exit(1)
        ok("Owner shell started")

        # Login (triggers auto-reconnect to kunki)
        step("Logging in owner...")
        call(owner_sock, "login", {"passphrase": PASSPHRASE})
        ok("Owner logged in (auto-reconnect triggered)")

        # Wait for auto-reconnect
        step("Waiting for P2P connection...")
        time.sleep(3)

        # Check connection status
        nodes = call(owner_sock, "list_nodes").get("nodes", [])
        node_id = None
        if nodes:
            node = nodes[0]
            node_id = node.get("node_id")
            info(f"Node: {node.get('name')} - connected: {node.get('connected')}")
        else:
            info("No nodes found")

        # ============================================================
        # OWNER: OPEN APP AND ADD PRODUCTS
        # ============================================================
        header("OWNER: ADDING PRODUCTS")

        # Get space and page
        spaces = call(owner_sock, "list_spaces").get("spaces", [])
        if not spaces:
            fail("No spaces found")
            sys.exit(1)
        space_id = spaces[0]["id"]
        info(f"Space: {spaces[0]['name']}")

        pages = call(owner_sock, "list_pages", {"space_id": space_id}).get("pages", [])
        if not pages:
            fail("No pages found")
            sys.exit(1)
        page_id = pages[0]["id"]

        # Open Shop Owner app
        step("Opening Shop Owner app...")
        call(owner_sock, "open_app", {"page_id": page_id, "app_name": "Shop Owner"})

        if not wait_for_eval(owner_sock):
            fail("Owner app failed to load")
            sys.exit(1)
        ok("Shop Owner app loaded")

        # Add products via UI automation (same code path as human)
        step("Adding products via UI...")
        call(owner_sock, "eval", {"code": 'add_product_via_ui("Widget", 29.99, "A useful widget", 100)'})
        call(owner_sock, "eval", {"code": 'add_product_via_ui("Gadget", 49.99, "A cool gadget", 50)'})
        call(owner_sock, "eval", {"code": 'add_product_via_ui("Gizmo", 19.99, "A handy gizmo", 200)'})

        count = call(owner_sock, "eval", {"code": "return get_products_count()"})
        ok(f"Owner has {count} products")

        # ============================================================
        # WAIT FOR SYNC AND GET VIEWER LINK
        # ============================================================
        header("WAITING FOR SYNC")
        step(f"Waiting {SYNC_WAIT}s for products to sync...")
        time.sleep(SYNC_WAIT)

        # Get viewer link for customer to connect
        viewer_link = None
        if node_id:
            step("Getting viewer link for customer...")
            try:
                link_result = call(owner_sock, "get_shareable_link", {"space_id": space_id, "node_id": node_id})
                viewer_link = link_result.get("connection_string")
                if viewer_link:
                    ok(f"Got viewer link: {viewer_link[:50]}...")
            except Exception as e:
                info(f"Could not get viewer link: {e}")

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

        # Login
        step("Logging in customer...")
        call(customer_sock, "login", {"passphrase": PASSPHRASE})
        ok("Customer logged in")

        # Wait for P2P to initialize
        time.sleep(2)

        # Connect customer to node via viewer link
        if viewer_link:
            step("Connecting customer to node via viewer link...")
            try:
                call(customer_sock, "add_website", {"connection_string": viewer_link})
                ok("Customer connected to node")
                time.sleep(3)  # Wait for connection to establish
            except Exception as e:
                info(f"Customer connection: {e}")

        # ============================================================
        # CUSTOMER: OPEN APP AND CHECK PRODUCTS
        # ============================================================
        header("CUSTOMER: CHECKING PRODUCTS")

        # Get customer's space
        cust_spaces = call(customer_sock, "list_spaces").get("spaces", [])
        if not cust_spaces:
            fail("Customer has no spaces")
            sys.exit(1)
        cust_space_id = cust_spaces[0]["id"]

        cust_pages = call(customer_sock, "list_pages", {"space_id": cust_space_id}).get("pages", [])
        cust_page_id = cust_pages[0]["id"]

        # Open Customer app
        step("Opening Customer app...")
        call(customer_sock, "open_app", {"page_id": cust_page_id, "app_name": "Shop Customer"})

        if not wait_for_eval(customer_sock):
            fail("Customer app failed to load")
            sys.exit(1)
        ok("Customer app loaded")

        # Check products
        cust_products = call(customer_sock, "eval", {"code": "return get_products_count()"})
        info(f"Customer sees {cust_products} products")

        if cust_products >= 3:
            ok("Products synced to customer!")
        else:
            info("Products may still be syncing...")
            time.sleep(SYNC_WAIT)
            cust_products = call(customer_sock, "eval", {"code": "return get_products_count()"})
            info(f"Customer now sees {cust_products} products")

        # ============================================================
        # CUSTOMER: PLACE ORDER
        # ============================================================
        header("CUSTOMER: PLACING ORDER")

        # Get product list
        products = call(customer_sock, "eval", {"code": "return get_products()"})
        if products and len(products) > 0:
            product = products[0]
            info(f"Ordering: {product.get('name')} @ ${product.get('price')}")

            # Place order via UI automation (same code path as human)
            step("Placing order via UI...")
            call(customer_sock, "eval", {
                "code": f'place_order_via_ui("{product.get("id", "")}", 2, "Demo order", "123 Demo St")'
            })
            ok("Order placed via UI")

            order_id = call(customer_sock, "eval", {"code": "return get_last_order_id()"})
            info(f"Order ID: {order_id}")

            # ============================================================
            # WAIT FOR ORDER SYNC
            # ============================================================
            header("WAITING FOR ORDER SYNC")
            step(f"Waiting {SYNC_WAIT}s for order to sync...")
            time.sleep(SYNC_WAIT)

            # ============================================================
            # OWNER: CHECK FOR ORDER
            # ============================================================
            header("OWNER: CHECKING FOR ORDER")

            owner_orders = call(owner_sock, "eval", {"code": "return get_orders_count()"})
            info(f"Owner sees {owner_orders} orders")

            if owner_orders >= 1:
                ok("Order synced to owner!")
            else:
                info("Order may still be syncing (needs derivation on node)")
        else:
            info("No products available to order")

        # ============================================================
        # SUMMARY
        # ============================================================
        header("DEMO COMPLETE")

        print("Summary:")
        print(f"  Owner products:    {call(owner_sock, 'eval', {'code': 'return get_products_count()'})}")
        print(f"  Customer products: {call(customer_sock, 'eval', {'code': 'return get_products_count()'})}")
        print(f"  Customer orders:   {call(customer_sock, 'eval', {'code': 'return get_orders_count()'})}")
        print(f"  Owner orders:      {call(owner_sock, 'eval', {'code': 'return get_orders_count()'})}")
        print()
        print("UI windows are open - interact with them!")
        print("Press Ctrl+C to stop all processes...")

        # Keep running
        while True:
            time.sleep(1)

    except KeyboardInterrupt:
        print("\nShutting down...")
    except Exception as e:
        fail(str(e))
        import traceback
        traceback.print_exc()
        return False
    finally:
        pm.stop_all()

    return True


if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
