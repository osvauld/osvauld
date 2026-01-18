#!/usr/bin/env python3
"""Test Booking Flow - Verify blocked slots sync to customer

Tests the complete flow:
1. Provider blocks a time slot
2. Data syncs to node (derivation runs)
3. Customer sees the slot as blocked

Prerequisites:
    # Setup databases first:
    rm -rf /tmp/sthalam/booking && mkdir -p /tmp/sthalam/booking
    cp /tmp/booking_test/*.db /tmp/sthalam/booking/

    # Or run setup_booking_db:
    ./target/debug/setup_booking_db --db-dir /tmp/sthalam/booking

    # Start ai_interface:
    ./target/debug/ai_interface --name booking --instances provider,customer --node

Usage:
    python scripts/test_booking_flow.py
"""

import socket
import json
import time
import sys
from datetime import datetime, timedelta


AI_SOCKET = "/tmp/sthalam/booking/ai.sock"
PASSPHRASE = "test123"
SYNC_WAIT = 3  # seconds to wait for sync


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


def lua_eval(target, code, timeout=30):
    """Evaluate Lua code in target instance."""
    result = call(target, "eval", {"code": code}, timeout=timeout)
    return result.get("result")


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


def wait_for_app(target, timeout=20):
    """Wait for app to be loaded (eval available)."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            result = lua_eval(target, "return 1")
            if result == 1:
                return True
        except:
            pass
        time.sleep(0.5)
    return False


def main():
    header("Booking Flow Test")

    # Generate a unique date for this test run
    test_date = (datetime.now() + timedelta(days=7)).strftime("%Y-%m-%d")
    test_time = "14:00"
    test_end = "15:00"
    test_reason = f"Test block {datetime.now().strftime('%H:%M:%S')}"

    print(f"  Test date: {test_date}")
    print(f"  Test time: {test_time}-{test_end}")
    print()

    # Step 1: Check connection
    step("Checking AI interface connection...")
    try:
        # Use meta action format for status
        status = send({"action": "status", "id": 1})
        result = status.get("result", {})
        ok(f"Connected: provider={result.get('provider', 'unknown')}, customer={result.get('customer', 'unknown')}")
    except Exception as e:
        fail(f"Cannot connect to AI interface: {e}")
        print("\n  Make sure ai_interface is running:")
        print("  ./target/debug/ai_interface --name booking --instances provider,customer --node")
        return 1

    # Step 2: Login provider
    header("Provider Setup")
    step("Logging in provider...")
    result = call("provider", "login", {"passphrase": PASSPHRASE})
    if result.get("result", {}).get("success"):
        ok("Provider logged in")
    else:
        fail(f"Provider login failed: {result}")
        return 1

    # Step 3: Get space and page IDs
    step("Getting space info...")
    spaces = call("provider", "list_spaces")
    if not spaces.get("result", {}).get("spaces"):
        fail("No spaces found")
        return 1
    space = spaces["result"]["spaces"][0]
    space_id = space["id"]
    ok(f"Space: {space['name']} ({space_id[:8]}...)")

    step("Getting page info...")
    pages = call("provider", "list_pages", {"space_id": space_id})
    if not pages.get("result", {}).get("pages"):
        fail("No pages found")
        return 1
    page = pages["result"]["pages"][0]
    page_id = page["id"]
    ok(f"Page: {page['name']} ({page_id[:8]}...)")

    # Step 4: Open Service Provider app
    step("Opening Service Provider app...")
    call("provider", "open_app", {"page_id": page_id, "app_name": "Service Provider"})
    if not wait_for_app("provider"):
        fail("App failed to load")
        return 1
    ok("Service Provider app loaded")

    # Step 5: Get initial blocked count
    step("Getting initial blocked times...")
    initial_blocked = lua_eval("provider", "return get_blocked()")
    initial_count = len(initial_blocked) if initial_blocked else 0
    ok(f"Initial blocked count: {initial_count}")

    # Step 6: Block a new slot
    header("Block Time Slot")
    step(f"Blocking {test_date} {test_time}-{test_end}...")
    block_result = lua_eval("provider",
        f'return block_time_via_ui("{test_date}", "{test_time}", "{test_end}", "{test_reason}")')
    if block_result:
        ok("Block command sent")
    else:
        fail("Block command failed")
        return 1

    # Step 7: Verify blocked layer updated
    step("Verifying blocked layer...")
    time.sleep(1)  # Give it a moment
    new_blocked = lua_eval("provider", "return get_blocked()")
    new_count = len(new_blocked) if new_blocked else 0
    if new_count > initial_count:
        ok(f"Blocked count: {initial_count} -> {new_count}")
    else:
        fail(f"Blocked count did not increase: {new_count}")
        return 1

    # Find our blocked entry
    our_block = None
    for b in new_blocked:
        if b.get("date") == test_date and b.get("start_time") == test_time:
            our_block = b
            break
    if our_block:
        ok(f"Found our block: {our_block.get('id', 'no-id')[:12]}...")
    else:
        fail("Could not find our blocked entry")
        return 1

    # Step 8: Wait for sync to node
    header("Sync to Node")
    step(f"Waiting {SYNC_WAIT}s for sync to node...")
    time.sleep(SYNC_WAIT)
    ok("Sync wait complete")

    # Step 9: Check derived calendar on provider
    step("Checking derived calendar (provider view)...")
    calendar_code = f'''
        local cal = loro:get_layer(permit:page_id() .. "/derived/calendar", "map")
        if cal then
            local key = "{test_date}_{test_time}"
            local slot = cal:get(key)
            return slot
        end
        return nil
    '''
    derived_slot = lua_eval("provider", calendar_code)
    if derived_slot and derived_slot.get("booked"):
        ok(f"Derived calendar has slot: booked={derived_slot.get('booked')}")
    else:
        fail(f"Derived calendar missing slot: {derived_slot}")
        return 1

    # Step 10: Login customer
    header("Customer Verification")
    step("Logging in customer...")
    result = call("customer", "login", {"passphrase": PASSPHRASE})
    if result.get("result", {}).get("success"):
        ok("Customer logged in")
    else:
        fail(f"Customer login failed: {result}")
        return 1

    # Step 11: Open Service Customer app
    step("Opening Service Customer app...")
    call("customer", "open_app", {"page_id": page_id, "app_name": "Service Customer"})
    if not wait_for_app("customer"):
        fail("Customer app failed to load")
        return 1
    ok("Service Customer app loaded")

    # Step 12: Check if slot is blocked for customer
    step(f"Checking if {test_date} {test_time} is blocked for customer...")
    is_blocked = lua_eval("customer", f'return is_slot_booked("{test_date}", "{test_time}")')
    if is_blocked:
        ok(f"Customer sees slot as BLOCKED")
    else:
        fail(f"Customer does NOT see slot as blocked")
        return 1

    # Summary
    header("TEST PASSED")
    print(f"  Provider blocked: {test_date} {test_time}-{test_end}")
    print(f"  Derivation ran on node")
    print(f"  Customer sees slot as blocked")
    print()

    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("\n\nInterrupted")
        sys.exit(1)
    except Exception as e:
        print(f"\n  [ERROR] {e}")
        sys.exit(1)
