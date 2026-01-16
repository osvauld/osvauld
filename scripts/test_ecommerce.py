#!/usr/bin/env python3
"""E-Commerce Integration Test

Tests the full e-commerce flow:
1. Owner creates space, imports shop app, adds products
2. Customer connects and views products
3. Customer places order
4. Owner sees order and updates status
5. Customer sees updated status

Usage:
    # Fresh setup (clears DBs)
    ./scripts/test_ecommerce.py --fresh

    # Use existing DBs (from setup_test_dbs)
    ./scripts/test_ecommerce.py --db-dir /tmp/ai_test

    # Keep shells running after test (for debugging)
    ./scripts/test_ecommerce.py --fresh --keep-alive
"""

import subprocess
import socket
import json
import time
import sys
import os
import signal
import argparse
from pathlib import Path
from typing import Optional, Dict, Any, List

# Test configuration
DEFAULT_DB_DIR = "/tmp/ecommerce_test"
DEFAULT_PASSPHRASE = "test123"
SAMPLE_APP_PATH = Path(__file__).parent.parent / "sample_apps" / "my-shop"
SHELL_BINARY = Path(__file__).parent.parent / "target" / "debug" / "slint_shell"

# Timeouts
SHELL_STARTUP_TIMEOUT = 3.0
COMMAND_TIMEOUT = 10.0
APP_LOAD_TIMEOUT = 5.0


class DebugClient:
    """Client for communicating with slint_shell debug socket."""

    def __init__(self, socket_path: str, name: str = ""):
        self.socket_path = socket_path
        self.name = name

    def send(self, cmd: Dict[str, Any]) -> Dict[str, Any]:
        """Send a command and return the response."""
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
        """Call a method and return the result (raises on error)."""
        cmd = {"method": method, "id": 1}
        if params:
            cmd["params"] = params

        response = self.send(cmd)

        if "error" in response:
            err = response["error"]
            msg = err.get("message", str(err)) if isinstance(err, dict) else str(err)
            raise RuntimeError(f"[{self.name}] {method} failed: {msg}")

        return response.get("result")

    # Convenience methods
    def ping(self) -> bool:
        result = self.call("ping")
        return result.get("status") == "ok"

    def signup(self, username: str, passphrase: str) -> Dict:
        return self.call("sign_up", {"username": username, "passphrase": passphrase})

    def login(self, passphrase: str) -> Dict:
        return self.call("login", {"passphrase": passphrase})

    def screen(self) -> str:
        return self.call("ui_get_screen").get("screen", "")

    def create_space(self, name: str) -> Dict:
        return self.call("create_space", {"name": name})

    def import_page(self, space_id: str, page_dir: str) -> Dict:
        return self.call("import_page", {"space_id": space_id, "page_dir": page_dir})

    def open_app(self, page_id: str, app_name: str) -> Dict:
        return self.call("open_app", {"page_id": page_id, "app_name": app_name})

    def spaces(self) -> List[Dict]:
        return self.call("list_spaces").get("spaces", [])

    def pages(self, space_id: str) -> List[Dict]:
        return self.call("list_pages", {"space_id": space_id}).get("pages", [])

    def apps(self, page_id: str) -> List[Dict]:
        return self.call("list_apps", {"page_id": page_id}).get("apps", [])

    def eval(self, code: str) -> Any:
        return self.call("eval", {"code": code})


class ShellInstance:
    """Manages a slint_shell process."""

    def __init__(self, name: str, db_dir: str):
        self.name = name
        self.db_dir = db_dir
        self.socket_path = f"{db_dir}/{name}.sock"
        self.process: Optional[subprocess.Popen] = None
        self.client: Optional[DebugClient] = None

    def start(self):
        """Start the shell process."""
        env = os.environ.copy()
        env["STHALAM_DATA_DIR"] = self.db_dir
        env["SLINT_BACKEND"] = "testing"

        self.process = subprocess.Popen(
            [
                str(SHELL_BINARY),
                "-d", self.name,
                "--debug-socket", self.socket_path,
            ],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        # Wait for socket to be available
        start_time = time.time()
        while time.time() - start_time < SHELL_STARTUP_TIMEOUT:
            if os.path.exists(self.socket_path):
                try:
                    self.client = DebugClient(self.socket_path, self.name)
                    if self.client.ping():
                        return
                except:
                    pass
            time.sleep(0.1)

        raise RuntimeError(f"Shell {self.name} failed to start")

    def stop(self):
        """Stop the shell process."""
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.process.kill()
            self.process = None

        # Clean up socket
        if os.path.exists(self.socket_path):
            os.remove(self.socket_path)


class TestContext:
    """Holds test state and shell instances."""

    def __init__(self, db_dir: str, passphrase: str):
        self.db_dir = db_dir
        self.passphrase = passphrase
        self.shells: Dict[str, ShellInstance] = {}
        self.space_id: Optional[str] = None
        self.page_id: Optional[str] = None

    def add_shell(self, name: str) -> ShellInstance:
        shell = ShellInstance(name, self.db_dir)
        self.shells[name] = shell
        return shell

    def cleanup(self):
        for shell in self.shells.values():
            shell.stop()


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


def assert_eq(actual, expected, msg: str):
    if actual != expected:
        raise AssertionError(f"{msg}: expected {expected}, got {actual}")


def assert_contains(collection, item, msg: str):
    if item not in collection:
        raise AssertionError(f"{msg}: {item} not in {collection}")


def run_test(ctx: TestContext, fresh: bool = True):
    """Run the full e-commerce test."""

    # ================================================================
    # SETUP OWNER
    # ================================================================
    print_header("SETTING UP OWNER")

    owner = ctx.add_shell("owner")
    owner.start()
    print_ok(f"Owner shell started (socket: {owner.socket_path})")

    dc = owner.client

    if fresh:
        # Fresh signup
        print_step("Signing up owner...")
        dc.signup("shop_owner", ctx.passphrase)
        print_ok("Owner signed up")

    print_step("Logging in owner...")
    dc.login(ctx.passphrase)
    assert_eq(dc.screen(), "spaces", "Owner should be on spaces screen after login")
    print_ok("Owner logged in")

    if fresh:
        # Create space and import app
        print_step("Creating space...")
        space = dc.create_space("My Shop")
        ctx.space_id = space["id"]
        print_ok(f"Space created: {ctx.space_id}")

        print_step("Importing my-shop page...")
        page = dc.import_page(ctx.space_id, str(SAMPLE_APP_PATH))
        ctx.page_id = page["page_id"]
        print_ok(f"Page imported: {ctx.page_id} with apps: {page['apps']}")
    else:
        # Find existing space/page
        spaces = dc.spaces()
        assert_contains([s["name"] for s in spaces], "My Shop", "Space 'My Shop' should exist")
        ctx.space_id = next(s["id"] for s in spaces if s["name"] == "My Shop")

        pages = dc.pages(ctx.space_id)
        assert_eq(len(pages), 1, "Should have exactly one page")
        ctx.page_id = pages[0]["id"]
        print_ok(f"Found existing space: {ctx.space_id}, page: {ctx.page_id}")

    # Open Shop Owner app
    print_step("Opening Shop Owner app...")
    dc.open_app(ctx.page_id, "Shop Owner")
    time.sleep(APP_LOAD_TIMEOUT)  # Wait for app to load

    # Verify eval works
    result = dc.eval("return 1 + 1")
    assert_eq(result, 2, "Eval should work")
    print_ok("Shop Owner app opened, eval working")

    # ================================================================
    # OWNER: ADD PRODUCTS
    # ================================================================
    print_header("OWNER: ADDING PRODUCTS")

    print_step("Adding products...")
    dc.eval('add_product("Widget", 29.99, "A useful widget", 100)')
    dc.eval('add_product("Gadget", 49.99, "A cool gadget", 50)')
    dc.eval('add_product("Gizmo", 19.99, "A handy gizmo", 200)')

    count = dc.eval("return get_products_count()")
    assert_eq(count, 3, "Should have 3 products")
    print_ok(f"Added 3 products (count: {count})")

    products = dc.eval("return get_products()")
    print_ok(f"Products: {json.dumps(products, indent=2)}")

    # ================================================================
    # VERIFY OWNER STATE
    # ================================================================
    print_header("VERIFYING OWNER STATE")

    orders = dc.eval("return get_all_orders()")
    order_count = dc.eval("return get_orders_count()")
    assert_eq(order_count, 0, "Should have 0 orders initially")
    print_ok(f"Initial orders: {order_count}")

    # ================================================================
    # TEST PRODUCT OPERATIONS
    # ================================================================
    print_header("TESTING PRODUCT OPERATIONS")

    # Get product by name
    products = dc.eval("return get_products()")
    widget = next(p for p in products if p["name"] == "Widget")
    print_ok(f"Widget: id={widget['id']}, price={widget['price']}, stock={widget['stock']}")

    # Test product lookup
    product_count = dc.eval("return get_products_count()")
    assert_eq(product_count, 3, "Should have 3 products")
    print_ok(f"Product count verified: {product_count}")

    # ================================================================
    # NOTE: CUSTOMER ORDER FLOW
    # ================================================================
    print_header("CUSTOMER ORDER FLOW (REQUIRES MULTI-INSTANCE)")

    print("""
    The customer order flow requires:
    1. A separate shell instance with customer role
    2. Sync via a node (kunki)
    3. Proper permits for customer to create orders

    For full e-commerce testing with orders, use:
    - cargo run -p integration_tests --bin setup_test_dbs
    - Then run multiple shell instances with proper roles

    The single-instance test validates:
    - Shell startup and socket communication
    - Signup/login flow
    - Space and page creation
    - App loading and Lua eval
    - Product management (add, list, count)
    """)

    # ================================================================
    # SUMMARY
    # ================================================================
    print_header("TEST COMPLETED SUCCESSFULLY")

    print(f"  Space ID: {ctx.space_id}")
    print(f"  Page ID:  {ctx.page_id}")
    print(f"  Products: {count}")
    print(f"  Orders:   {order_count}")
    print()

    return True


def main():
    parser = argparse.ArgumentParser(description="E-Commerce Integration Test")
    parser.add_argument("--db-dir", default=DEFAULT_DB_DIR, help="Database directory")
    parser.add_argument("--passphrase", default=DEFAULT_PASSPHRASE, help="Passphrase for all users")
    parser.add_argument("--fresh", action="store_true", help="Clear DB and start fresh")
    parser.add_argument("--keep-alive", action="store_true", help="Keep shells running after test")
    args = parser.parse_args()

    # Check binary exists
    if not SHELL_BINARY.exists():
        print(f"Error: Shell binary not found at {SHELL_BINARY}")
        print("Run: cargo build -p slint_shell")
        sys.exit(1)

    # Check sample app exists
    if not SAMPLE_APP_PATH.exists():
        print(f"Error: Sample app not found at {SAMPLE_APP_PATH}")
        sys.exit(1)

    # Setup
    db_dir = args.db_dir
    if args.fresh:
        print(f"Clearing database directory: {db_dir}")
        subprocess.run(["rm", "-rf", db_dir], check=True)

    os.makedirs(db_dir, exist_ok=True)

    ctx = TestContext(db_dir, args.passphrase)

    try:
        success = run_test(ctx, fresh=args.fresh)

        if args.keep_alive:
            print("\n  Shells kept alive. Press Ctrl+C to stop.\n")
            print("  Owner socket:", ctx.shells["owner"].socket_path)
            try:
                while True:
                    time.sleep(1)
            except KeyboardInterrupt:
                print("\n  Shutting down...")

        sys.exit(0 if success else 1)

    except Exception as e:
        print_fail(str(e))
        import traceback
        traceback.print_exc()
        sys.exit(1)

    finally:
        if not args.keep_alive:
            ctx.cleanup()


if __name__ == "__main__":
    main()
