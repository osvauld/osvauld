#!/usr/bin/env python3
"""
E-Commerce Sync Integration Test

Tests the full e-commerce P2P flow:
1. Owner adds product -> syncs to customer
2. Customer creates order -> submits order
3. Node runs derivation -> orders_summary created
4. Owner sees order -> confirms order
5. Status change syncs to customer

Usage:
    python scripts/test_ecommerce_sync.py              # Run test, cleanup on success
    python scripts/test_ecommerce_sync.py --keep       # Keep session alive after test
    python scripts/test_ecommerce_sync.py --debug      # Keep session on failure for debugging

After test, attach to tmux:
    tmux attach -t ecomm_test
"""

import sys
import time
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager


SHOP_APP = Path(__file__).parent.parent / "sample_apps" / "my-shop"
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


def main():
    print("=" * 60)
    print("  E-Commerce Sync Integration Test")
    print("=" * 60)

    # 1. Start all instances in tmux
    print("\n[1/10] Starting instances (node + owner + customer)...")
    tm = TmuxManager(
        session_name="ecomm_test",
        base_dir=Path("/tmp/ecomm_test"),
    )
    tm.add_node("node")
    tm.add_shell("owner")
    tm.add_shell("customer")
    tm.start()

    node = tm.get_client("node")
    owner = tm.get_client("owner")
    customer = tm.get_client("customer")
    print("      All instances ready")

    try:
        # 2. Owner setup
        print("\n[2/10] Owner: signup, create space...")
        owner.signup_or_login("owner")
        space = owner.create_space_with_pages(str(SHOP_APP))
        space_id = space["id"]
        page_id = space["pages"][0]["page_id"]
        print(f"      Space: {space_id[:8]}..., Page: {page_id[:8]}...")

        # 3. Connect and publish
        print("\n[3/10] Owner: connect to node, publish...")
        owner.connect_to_node(node)
        time.sleep(2)
        owner.publish_to_node(space_id)
        time.sleep(2)
        print("      Published")

        # 4. Customer setup
        print("\n[4/10] Customer: signup, subscribe...")
        customer.signup_or_login("customer")
        owner.add_viewer(customer, space_id)

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
        owner.open_app(page_id, "Shop Owner")
        time.sleep(2)
        customer.open_app(customer_page_id, "Shop Customer")
        time.sleep(2)
        print("      Both apps opened")

        # 6. Owner adds product - PAUSE FOR DEBUGGING
        print("\n[6/11] Owner: adding product...")
        owner.eval('add_product_via_ui("Test Widget", 99, "A test product", 50)')
        time.sleep(3)
        owner_products = owner.eval("return get_products_count()")
        print(f"      Owner has {owner_products} product(s)")
        assert owner_products >= 1, "Owner should have at least 1 product"

        # Wait for product to sync to customer
        print("      Waiting for sync to customer...")
        def check_customer_products():
            count = customer.eval("return get_products_count()")
            return count >= 1
        wait_for(check_customer_products, timeout=15, desc="customer products sync")
        customer_products = customer.eval("return get_products_count()")
        print(f"      Customer has {customer_products} product(s) - SYNC OK")


        # 7. Customer creates order - PAUSE FOR DEBUGGING
        print("\n[7/11] Customer: selecting product and creating draft...")
        product_id = customer.eval("return get_first_product_id()")
        print(f"      Product ID: {product_id[:16]}...")

        # Check initial state
        orders_before = customer.eval("return get_orders_count()")
        drafts_before = customer.eval("return get_drafts_count()")
        print(f"      Before: orders={orders_before}, drafts={drafts_before}")

        # Select product
        print("      Selecting product...")
        customer.eval(f'select_product("{product_id}")')
        print("      Product selected")

        # Fill form fields
        print("      Filling form fields...")
        customer.eval('on_field_changed("order_quantity", "2")')
        customer.eval('on_field_changed("order_notes", "Test order")')
        customer.eval('on_field_changed("order_address", "123 Test St")')
        print("      Form fields set")

        # Create draft (via modal submit)
        print("      Creating draft order...")
        customer.eval('on_modal_action("order", "submit")')

        drafts_after = customer.eval("return get_drafts_count()")
        order1_id = customer.eval("return get_last_order_id()")
        print(f"      Created draft: {order1_id}, drafts={drafts_after}")


        # Submit the order
        print("      Submitting order...")
        customer.eval(f'submit_order("{order1_id}")')
        orders_after = customer.eval("return get_orders_count()")
        print(f"      After submit: orders={orders_after}")

        customer_orders = customer.eval("return get_orders_count()")
        print(f"      Customer has {customer_orders} order(s)")
        assert customer_orders >= 1, "Customer should have at least 1 order"

        last_order_id = order1_id
        print(f"      Last Order ID: {last_order_id}")

        # Get customer DID early
        customer_did = customer.eval("return permit:my_did()")
        print(f"      Customer DID: {customer_did[:30]}...")

        # 8. Wait for derivation and owner to see order
        print("\n[8/11] Waiting for derivation and sync to owner...")
        time.sleep(3)  # Give node time to process and run derivation

        def check_owner_orders():
            count = owner.eval("return get_orders_count()")
            if count >= 1:
                status = owner.eval(f'return get_order_status("{last_order_id}")')
                return status is not None
            return False
        wait_for(check_owner_orders, timeout=30, desc="owner sees order")

        owner_status = owner.eval(f'return get_order_status("{last_order_id}")')
        print(f"      Owner sees order with status: {owner_status} - DERIVATION OK")

        # 9. Owner confirms order
        print("\n[9/11] Owner: confirming order...")
        owner.eval(f'update_order_status("{customer_did}", "{last_order_id}", "confirmed")')
        time.sleep(1)

        order_status_owner = owner.eval(f'return get_order_status("{last_order_id}")')
        print(f"      Owner sees status: {order_status_owner}")
        assert order_status_owner == "confirmed", f"Expected 'confirmed', got '{order_status_owner}'"

        # 10. Check status syncs to customer
        print("\n[10/11] Checking status sync to customer...")
        def check_customer_status():
            orders = customer.eval("return get_all_orders()")
            if orders:
                for order in orders:
                    if order.get("id") == last_order_id:
                        return order.get("status") == "confirmed"
            return False
        wait_for(check_customer_status, timeout=20, desc="customer status sync")

        orders = customer.eval("return get_all_orders()")
        final_status = None
        for order in orders:
            if order.get("id") == last_order_id:
                final_status = order.get("status")
                break
        print(f"      Customer sees status: {final_status} - STATUS SYNC OK")

        # 11. Final verification
        print("\n[11/11] Final verification...")
        for order in orders:
            if order.get("id") == last_order_id:
                print(f"      Order: {order.get('id')}")
                print(f"      Status: {order.get('status')}")
                print(f"      Items: {order.get('items')} x {order.get('quantity')}")
                print(f"      Total: ${order.get('total')}")

        # Summary
        print("\n" + "=" * 60)
        print("  [SUCCESS] Full E-Commerce Flow!")
        print("=" * 60)
        print("    [OK] Product sync: owner -> customer")
        print("    [OK] Order created and submitted")
        print("    [OK] Derivation: orders -> orders_summary")
        print("    [OK] Owner confirmed order")
        print("    [OK] Status sync: owner -> customer")
        print("=" * 60)

        return 0

    except Exception as e:
        print(f"\n[FAIL] {e}")
        import traceback
        traceback.print_exc()

        # Debug: print state on failure
        try:
            print("\n--- Debug Info ---")
            state = owner.eval('return debug_state()')
            print(f"Owner state: {state}")
        except:
            pass

        if KEEP_SESSION or DEBUG_ON_FAIL:
            print("\n" + "=" * 60)
            print("  Session kept alive for debugging")
            print("=" * 60)
            print(f"\n  tmux attach -t ecomm_test")
            print(f"\n  Sockets:")
            print(f"    Node:     /tmp/ecomm_test/node/node.sock")
            print(f"    Owner:    /tmp/ecomm_test/owner/owner.sock")
            print(f"    Customer: /tmp/ecomm_test/customer/customer.sock")
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
            print(f"\n  tmux attach -t ecomm_test")
            print(f"\n  Sockets:")
            print(f"    Node:     /tmp/ecomm_test/node/node.sock")
            print(f"    Owner:    /tmp/ecomm_test/owner/owner.sock")
            print(f"    Customer: /tmp/ecomm_test/customer/customer.sock")
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
    parser = argparse.ArgumentParser(description="E-Commerce Sync Integration Test")
    parser.add_argument("--keep", action="store_true", help="Keep tmux session alive after test")
    parser.add_argument("--debug", action="store_true", help="Keep session on failure for debugging")
    args = parser.parse_args()

    KEEP_SESSION = args.keep
    DEBUG_ON_FAIL = args.debug

    sys.exit(main())
