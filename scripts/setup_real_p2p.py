#!/usr/bin/env python3
"""Setup Real P2P Connections

Uses existing databases from setup_test_dbs and establishes real QUIC connections:
1. Start kunki, get its real connection string
2. Owner adds kunki as node, publishes space
3. Get viewer link from kunki
4. Customer connects via viewer link

Usage:
    ./scripts/setup_real_p2p.py [--db-dir /tmp/order_flow_test]
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
DC = SCRIPT_DIR / "dc"

# Config
DB_DIR = os.environ.get("DB_DIR", "/tmp/order_flow_test")
PASSPHRASE = "test123"

# Timeouts
STARTUP_TIMEOUT = 10.0
COMMAND_TIMEOUT = 15.0


def send_command(sock_path, cmd, timeout=COMMAND_TIMEOUT):
    """Send a JSON command to debug socket."""
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
    """Call a debug socket method."""
    cmd = {"method": method, "id": 1}
    if params:
        cmd["params"] = params
    response = send_command(sock_path, cmd)
    if "error" in response:
        raise RuntimeError(f"{method}: {response['error']}")
    return response.get("result")


def wait_for_socket(sock_path, timeout=STARTUP_TIMEOUT):
    """Wait for socket to be available."""
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


def wait_for_p2p(sock_path, timeout=15.0):
    """Wait for P2P to be ready."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            status = call(sock_path, "p2p_status")
            if status.get("p2p_ready"):
                return True
        except:
            pass
        time.sleep(0.5)
    return False


def print_header(text):
    print(f"\n{'='*60}")
    print(f"  {text}")
    print(f"{'='*60}\n")


def print_step(text):
    print(f"  -> {text}")


def print_ok(text):
    print(f"  [OK] {text}")


def print_fail(text):
    print(f"  [FAIL] {text}")


def print_info(text):
    print(f"  [i] {text}")


class ProcessManager:
    def __init__(self):
        self.processes = {}

    def start(self, name, cmd, env=None):
        full_env = os.environ.copy()
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


def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--db-dir", default=DB_DIR)
    args = parser.parse_args()

    db_dir = args.db_dir

    # Check binaries
    for binary in [SHELL_BINARY, KUNKI_BINARY]:
        if not binary.exists():
            print(f"Error: {binary} not found")
            print("Run: cargo build -p slint_shell -p kunki")
            sys.exit(1)

    # Verify databases exist
    for db_name in ["shop_owner", "kunki", "customer"]:
        db_path = Path(db_dir) / f"{db_name}.db"
        if not db_path.exists():
            print(f"Error: {db_path} not found")
            print(f"Run setup_test_dbs first: cargo run -p integration_tests --bin setup_test_dbs -- --db-dir {db_dir}")
            sys.exit(1)

    pm = ProcessManager()
    kunki_sock = f"{db_dir}/kunki.sock"
    owner_sock = f"{db_dir}/owner.sock"
    customer_sock = f"{db_dir}/customer.sock"

    # Clean up old sockets
    for sock in [kunki_sock, owner_sock, customer_sock]:
        if os.path.exists(sock):
            os.remove(sock)

    try:
        # ============================================================
        # START KUNKI
        # ============================================================
        print_header("STARTING KUNKI")

        pm.start("kunki", [
            str(KUNKI_BINARY),
            "-d", f"{db_dir}/kunki",
            "start", "-p", PASSPHRASE,
            "--debug-socket", kunki_sock,
        ])

        if not wait_for_socket(kunki_sock, timeout=15):
            raise RuntimeError("Kunki failed to start")
        print_ok("Kunki started")

        # Get kunki's real connection string
        print_step("Getting kunki connection string...")
        time.sleep(2)  # Wait for connection string to be generated
        result = call(kunki_sock, "get_connection_string")
        conn_string = result.get("connection_string")
        if not conn_string:
            raise RuntimeError("Failed to get kunki connection string")
        print_ok(f"Got connection string: {conn_string[:50]}...")

        # ============================================================
        # START OWNER
        # ============================================================
        print_header("STARTING OWNER")

        pm.start("owner", [
            str(SHELL_BINARY),
            "-d", "shop_owner",
            "--debug-socket", owner_sock,
        ], env={"STHALAM_DATA_DIR": db_dir})

        if not wait_for_socket(owner_sock):
            raise RuntimeError("Owner failed to start")
        print_ok("Owner shell started")

        # Login
        call(owner_sock, "login", {"passphrase": PASSPHRASE})
        print_ok("Owner logged in")

        # Wait for P2P
        if not wait_for_p2p(owner_sock):
            print_fail("Owner P2P not ready")
        else:
            print_ok("Owner P2P ready")

        # Get space ID
        spaces = call(owner_sock, "list_spaces").get("spaces", [])
        if not spaces:
            raise RuntimeError("No spaces found")
        space_id = spaces[0]["id"]
        print_info(f"Space: {spaces[0]['name']} ({space_id[:20]}...)")

        # Check stored nodes - relationship should exist from setup_test_dbs
        nodes = call(owner_sock, "list_nodes").get("nodes", [])
        if nodes:
            node_id = nodes[0]["id"]
            is_connected = nodes[0].get("is_connected", False)
            print_info(f"Node: {node_id[:20]}... (connected: {is_connected})")

            if not is_connected:
                # Node exists but not connected - update relay URL and reconnect
                print_step("Updating node address with real relay URL...")
                try:
                    result = call(owner_sock, "update_node_address", {"connection_string": conn_string})
                    print_ok(f"Updated node address: relay={result.get('relay_url', 'none')[:30] if result.get('relay_url') else 'none'}...")
                except RuntimeError as e:
                    print_info(f"Update address: {e}")

                # Wait for reconnection
                print_step("Waiting for reconnection...")
                for i in range(15):
                    time.sleep(2)
                    nodes = call(owner_sock, "list_nodes").get("nodes", [])
                    if nodes and nodes[0].get("is_connected"):
                        print_ok("Connected to node!")
                        break
                    print_info(f"  Still waiting... ({i+1}/15)")
                else:
                    print_info("Connection not established yet, continuing anyway...")
        else:
            # No node stored - need to add it
            print_step("No node found, adding kunki as sovereign node...")
            result = call(owner_sock, "add_node", {"connection_string": conn_string})
            print_ok(f"Added node: {result}")
            node_id = result.get("node_id")

        # Publish space to node
        print_step("Publishing space to node...")
        try:
            result = call(owner_sock, "publish_space", {"space_id": space_id, "node_id": node_id})
            print_ok(f"Published: {result}")
        except RuntimeError as e:
            if "already published" in str(e).lower():
                print_info("Space already published, continuing...")
            else:
                print_info(f"Publish result: {e}")

        # Wait for sync
        time.sleep(3)

        # Get viewer link from kunki
        print_step("Getting viewer link from kunki...")
        # We need to call this on kunki with the space ID
        # Let me check if kunki has a method for this
        try:
            result = call(kunki_sock, "get_viewer_link", {"space_id": space_id})
            viewer_link = result.get("viewer_link") or result.get("connection_string")
            print_ok(f"Viewer link: {viewer_link[:50] if viewer_link else 'None'}...")
        except Exception as e:
            print_info(f"Kunki viewer link not available: {e}")
            # Try via owner
            try:
                result = call(owner_sock, "get_shareable_link", {"space_id": space_id, "node_id": node_id})
                viewer_link = result.get("viewer_link") or result.get("link")
                print_ok(f"Viewer link (via owner): {viewer_link[:50] if viewer_link else 'None'}...")
            except Exception as e2:
                print_fail(f"Could not get viewer link: {e2}")
                viewer_link = None

        # ============================================================
        # START CUSTOMER
        # ============================================================
        print_header("STARTING CUSTOMER")

        pm.start("customer", [
            str(SHELL_BINARY),
            "-d", "customer",
            "--debug-socket", customer_sock,
        ], env={"STHALAM_DATA_DIR": db_dir})

        if not wait_for_socket(customer_sock):
            raise RuntimeError("Customer failed to start")
        print_ok("Customer shell started")

        # Login
        call(customer_sock, "login", {"passphrase": PASSPHRASE})
        print_ok("Customer logged in")

        # Wait for P2P
        if not wait_for_p2p(customer_sock):
            print_fail("Customer P2P not ready")
        else:
            print_ok("Customer P2P ready")

        # Connect customer via viewer link
        if viewer_link:
            print_step("Connecting customer via viewer link...")
            try:
                result = call(customer_sock, "add_website", {"connection_string": viewer_link})
                print_ok(f"Customer connected: {result}")
            except RuntimeError as e:
                if "already" in str(e).lower():
                    print_info("Customer already connected, continuing...")
                else:
                    print_info(f"Connect result: {e}")

        # Wait for sync
        print_step("Waiting for sync...")
        time.sleep(5)

        # ============================================================
        # SUMMARY
        # ============================================================
        print_header("SETUP COMPLETE")

        print("Sockets ready:")
        print(f"  Kunki:    {kunki_sock}")
        print(f"  Owner:    {owner_sock}")
        print(f"  Customer: {customer_sock}")
        print()
        print("Test commands:")
        print(f"  ./scripts/dc {owner_sock} spaces")
        print(f"  ./scripts/dc {owner_sock} pages <space_id>")
        print(f"  ./scripts/dc {owner_sock} open_app <page_id> 'Shop Owner'")
        print(f"  ./scripts/dc {owner_sock} eval 'return get_products_count()'")
        print()
        print("Press Ctrl+C to stop all processes...")

        # Keep running
        while True:
            time.sleep(1)

    except KeyboardInterrupt:
        print("\nShutting down...")
    except Exception as e:
        print_fail(str(e))
        import traceback
        traceback.print_exc()
        return False
    finally:
        pm.stop_all()

    return True


if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
