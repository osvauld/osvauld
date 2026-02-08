#!/usr/bin/env python3
"""
Booking Sync Integration Test

Tests the full booking P2P flow:
1. Provider sets schedule -> syncs to customer
2. Customer books a slot -> booking syncs to provider
3. Provider confirms booking -> status syncs to customer

Usage:
    python scripts/test_booking_sync.py              # Run test, cleanup on success
    python scripts/test_booking_sync.py --keep       # Keep session alive after test
    python scripts/test_booking_sync.py --debug      # Keep session on failure for debugging

After test, attach to tmux:
    tmux attach -t booking_test
"""

import sys
import time
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager


BOOKING_APP = Path(__file__).parent.parent / "sample_apps" / "my-booking"
KEEP_SESSION = False  # Set by args
DEBUG_ON_FAIL = False  # Set by args


def wait_for(condition_fn, timeout=30, interval=1, desc="condition"):
    """Wait for a condition to be true."""
    for i in range(int(timeout / interval)):
        try:
            result = condition_fn()
            if result:
                return result
        except Exception:
            pass
        time.sleep(interval)
    raise TimeoutError(f"Timeout waiting for {desc}")


def wait_for_app_ready(client, timeout=30):
    """Wait for app to be ready for eval."""
    for i in range(timeout):
        try:
            result = client.eval("return 1")
            if result == 1:
                return True
        except Exception:
            pass
        time.sleep(1)
    raise TimeoutError("App not ready for eval")


def main():
    print("=" * 60)
    print("  Booking Sync Integration Test")
    print("=" * 60)

    # 1. Start all instances in tmux
    print("\n[1/10] Starting instances (node + provider + customer)...")
    tm = TmuxManager(
        session_name="booking_test",
        base_dir=Path("/tmp/booking_test"),
    )
    tm.add_node("node")
    tm.add_shell("provider")
    tm.add_shell("customer")
    tm.start()

    node = tm.get_client("node")
    provider = tm.get_client("provider")
    customer = tm.get_client("customer")
    print("      All instances ready")

    try:
        # 2. Provider setup
        print("\n[2/10] Provider: signup, create space...")
        provider.signup_or_login("provider")
        space = provider.create_space_with_pages(str(BOOKING_APP))
        space_id = space["id"]
        page_id = space["pages"][0]["page_id"]
        print(f"      Space: {space_id[:8]}..., Page: {page_id[:8]}...")

        # 3. Connect and publish
        print("\n[3/10] Provider: connect to node, publish...")
        provider.connect_to_node(node)
        time.sleep(2)
        provider.publish_to_node(space_id)
        time.sleep(2)
        print("      Published")

        # 4. Customer setup
        print("\n[4/10] Customer: signup, subscribe...")
        customer.signup_or_login("customer")
        provider.add_viewer(customer, space_id)

        # Wait for customer to sync space
        customer_page_id = None
        for i in range(15):
            time.sleep(1)
            spaces = customer.list_spaces()
            if spaces:
                pages = customer.list_pages(spaces[0]["id"])
                if pages:
                    customer_page_id = pages[0]["id"]
                    print(f"      Customer synced in {i+1}s")
                    break
        if not customer_page_id:
            raise RuntimeError("Customer failed to sync space")

        # 5. Open apps
        print("\n[5/10] Opening apps...")
        provider.open_app(page_id, "Service Provider")
        wait_for_app_ready(provider)
        print("      Provider app ready")
        customer.open_app(customer_page_id, "Service Customer")
        wait_for_app_ready(customer)
        print("      Both apps opened")

        # 6. Provider sets schedule
        print("\n[6/10] Provider: setting schedule...")
        provider.eval('set_schedule_via_ui("monday", "09:00", "17:00", 30)')
        time.sleep(2)
        provider_schedule = provider.eval("return get_schedule()")
        print(f"      Provider schedule: {len(provider_schedule) if provider_schedule else 0} day(s)")
        assert provider_schedule and "monday" in provider_schedule, "Provider should have monday schedule"

        # 7. Verify schedule syncs to customer
        print("\n[7/10] Verifying schedule sync to customer...")
        def check_customer_schedule():
            schedule = customer.eval("return get_schedule()")
            return schedule and "monday" in schedule
        wait_for(check_customer_schedule, timeout=15, desc="customer schedule sync")
        customer_schedule = customer.eval("return get_schedule()")
        print(f"      Customer sees schedule: {len(customer_schedule) if customer_schedule else 0} day(s) - SYNC OK")

        # 8. Customer books a slot
        print("\n[8/10] Customer: booking slot...")
        # Book for next Monday (2026-01-27)
        customer.eval('book_slot_via_ui("2026-01-27", "10:00", "Consultation", "Test booking", "John Doe", "555-1234")')
        time.sleep(2)

        customer_bookings = customer.eval("return get_my_bookings()")
        print(f"      Customer has {len(customer_bookings) if customer_bookings else 0} booking(s)")
        assert customer_bookings and len(customer_bookings) >= 1, "Customer should have at least 1 booking"

        booking_id = customer_bookings[0].get("id")
        print(f"      Booking ID: {booking_id}")

        # Get customer DID for provider to confirm
        customer_did = customer.eval("return permit:my_did()")
        print(f"      Customer DID: {customer_did[:30]}...")

        # 9. Wait for derivation and provider to see booking
        print("\n[9/10] Waiting for booking to sync to provider...")
        time.sleep(3)  # Give node time to process

        def check_provider_bookings():
            count = provider.eval("return get_bookings_count()")
            return count >= 1
        wait_for(check_provider_bookings, timeout=30, desc="provider sees booking")

        provider_bookings = provider.eval("return get_all_bookings()")
        print(f"      Provider sees {len(provider_bookings) if provider_bookings else 0} booking(s) - DERIVATION OK")

        # Get the booking info from provider's perspective
        provider_booking = None
        if provider_bookings:
            for b in provider_bookings:
                if b.get("id") == booking_id:
                    provider_booking = b
                    break
        if not provider_booking and provider_bookings:
            provider_booking = provider_bookings[0]
            booking_id = provider_booking.get("id")
            customer_did = provider_booking.get("customer_did")

        print(f"      Booking status at provider: {provider_booking.get('status') if provider_booking else 'N/A'}")

        # 10. Provider confirms booking
        print("\n[10/10] Provider: confirming booking...")
        provider.eval(f'confirm_booking_via_ui("{customer_did}", "{booking_id}")')
        time.sleep(2)

        # Verify confirmation at provider
        provider_bookings_after = provider.eval("return get_all_bookings()")
        provider_status = None
        if provider_bookings_after:
            for b in provider_bookings_after:
                if b.get("id") == booking_id:
                    provider_status = b.get("status")
                    break
        print(f"      Provider sees status: {provider_status}")

        # Verify status syncs to customer
        def check_customer_status():
            bookings = customer.eval("return get_my_bookings()")
            if bookings:
                for b in bookings:
                    if b.get("id") == booking_id:
                        return b.get("status") == "confirmed"
            return False
        wait_for(check_customer_status, timeout=20, desc="customer status sync")

        customer_bookings_final = customer.eval("return get_my_bookings()")
        final_status = None
        if customer_bookings_final:
            for b in customer_bookings_final:
                if b.get("id") == booking_id:
                    final_status = b.get("status")
                    break
        print(f"      Customer sees status: {final_status} - STATUS SYNC OK")

        # Summary
        print("\n" + "=" * 60)
        print("  [SUCCESS] Full Booking Sync Flow!")
        print("=" * 60)
        print("    [OK] Schedule sync: provider -> customer")
        print("    [OK] Booking created by customer")
        print("    [OK] Derivation: booking synced to provider")
        print("    [OK] Provider confirmed booking")
        print("    [OK] Status sync: provider -> customer")
        print("=" * 60)

        return 0

    except Exception as e:
        print(f"\n[FAIL] {e}")
        import traceback
        traceback.print_exc()

        # Debug: print state on failure
        try:
            print("\n--- Debug Info ---")
            provider_schedule = provider.eval("return get_schedule()")
            print(f"Provider schedule: {provider_schedule}")
            customer_schedule = customer.eval("return get_schedule()")
            print(f"Customer schedule: {customer_schedule}")
            provider_bookings = provider.eval("return get_all_bookings()")
            print(f"Provider bookings: {provider_bookings}")
            customer_bookings = customer.eval("return get_my_bookings()")
            print(f"Customer bookings: {customer_bookings}")
        except:
            pass

        if KEEP_SESSION or DEBUG_ON_FAIL:
            print("\n" + "=" * 60)
            print("  Session kept alive for debugging")
            print("=" * 60)
            print(f"\n  tmux attach -t booking_test")
            print(f"\n  Sockets:")
            print(f"    Node:     /tmp/booking_test/node/node.sock")
            print(f"    Provider: /tmp/booking_test/provider/provider.sock")
            print(f"    Customer: /tmp/booking_test/customer/customer.sock")
            print("\n  Press Ctrl+C to stop and cleanup")
            try:
                import signal
                signal.pause()
            except KeyboardInterrupt:
                pass
            tm.stop()
        else:
            tm.stop()
        return 1

    finally:
        if KEEP_SESSION:
            print("\n" + "=" * 60)
            print("  Session kept alive (--keep flag)")
            print("=" * 60)
            print(f"\n  tmux attach -t booking_test")
            print(f"\n  Sockets:")
            print(f"    Node:     /tmp/booking_test/node/node.sock")
            print(f"    Provider: /tmp/booking_test/provider/provider.sock")
            print(f"    Customer: /tmp/booking_test/customer/customer.sock")
            print("\n  Press Ctrl+C to stop and cleanup")
            try:
                import signal
                signal.pause()
            except KeyboardInterrupt:
                pass
            tm.stop()
        elif not DEBUG_ON_FAIL:
            print("\nCleaning up...")
            tm.stop()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Booking Sync Integration Test")
    parser.add_argument("--keep", action="store_true", help="Keep tmux session alive after test")
    parser.add_argument("--debug", action="store_true", help="Keep session on failure for debugging")
    args = parser.parse_args()

    KEEP_SESSION = args.keep
    DEBUG_ON_FAIL = args.debug

    sys.exit(main())
