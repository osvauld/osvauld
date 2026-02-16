#!/usr/bin/env python3
"""
E-Commerce E2E Test — product sync, ordering, derivation, status updates.

Tests the full flow:
1. Owner adds product -> syncs to customer
2. Customer creates and submits order
3. Node runs derivation -> orders_summary created
4. Owner sees order -> confirms it
5. Status change syncs back to customer

Usage:
    python e2e_tests/test_ecommerce.py
    python e2e_tests/test_ecommerce.py --keep
    python e2e_tests/test_ecommerce.py --debug
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

SHOP_APP = Path(__file__).parent.parent / "sample_apps" / "my-shop"

args = AppTestScenario.parse_args("E-Commerce E2E Test")

with AppTestScenario(
    name="ecomm_test",
    app_path=str(SHOP_APP),
    peers={
        "owner": {"role": "owner", "app": "Shop Owner"},
        "customer": {"role": "viewer", "app": "Shop Customer"},
    },
    **args,
) as s:
    owner = s.peer("owner")
    customer = s.peer("customer")

    # 1. Owner adds product
    print("[1/6] Owner: adding product...")
    owner.eval('add_product_via_ui("Test Widget", 99, "A test product", 50)')
    time.sleep(3)
    owner_products = owner.eval("return get_products_count()")
    print(f"  Owner has {owner_products} product(s)")
    assert owner_products >= 1, "Owner should have at least 1 product"

    # Wait for sync to customer
    print("  Waiting for sync to customer...")
    customer.wait_for(
        lambda: customer.eval("return get_products_count()") >= 1,
        desc="customer products sync",
    )
    print(f"  Customer has {customer.eval('return get_products_count()')} product(s) - SYNC OK")

    # 2. Customer creates order
    print("\n[2/6] Customer: creating order...")
    product_id = customer.eval("return get_first_product_id()")
    print(f"  Product ID: {product_id[:16]}...")

    customer.eval(f'select_product("{product_id}")')
    customer.eval('on_field_changed("order_quantity", "2")')
    customer.eval('on_field_changed("order_notes", "Test order")')
    customer.eval('on_field_changed("order_address", "123 Test St")')
    customer.eval('on_modal_action("order", "submit")')

    order_id = customer.eval("return get_last_order_id()")
    print(f"  Draft created: {order_id}")

    # Submit order
    customer.eval(f'submit_order("{order_id}")')
    customer_orders = customer.eval("return get_orders_count()")
    print(f"  Customer has {customer_orders} order(s)")
    assert customer_orders >= 1, "Customer should have at least 1 order"

    # Get customer DID for status update
    customer_did = customer.eval("return permit:my_did()")

    # 3. Wait for derivation and owner to see order
    print("\n[3/6] Waiting for derivation and sync to owner...")
    time.sleep(3)
    owner.wait_for(
        lambda: (
            owner.eval("return get_orders_count()") >= 1
            and owner.eval(f'return get_order_status("{order_id}")') is not None
        ),
        timeout=30,
        desc="owner sees order",
    )
    owner_status = owner.eval(f'return get_order_status("{order_id}")')
    print(f"  Owner sees order with status: {owner_status} - DERIVATION OK")

    # 4. Owner confirms order
    print("\n[4/6] Owner: confirming order...")
    owner.eval(f'update_order_status("{customer_did}", "{order_id}", "confirmed")')
    time.sleep(1)

    order_status_owner = owner.eval(f'return get_order_status("{order_id}")')
    print(f"  Owner sees status: {order_status_owner}")
    assert order_status_owner == "confirmed", f"Expected 'confirmed', got '{order_status_owner}'"

    # 5. Check status syncs to customer
    print("\n[5/6] Checking status sync to customer...")
    def check_customer_status():
        orders = customer.eval("return get_all_orders()")
        if orders:
            for order in orders:
                if order.get("id") == order_id:
                    return order.get("status") == "confirmed"
        return False

    customer.wait_for(check_customer_status, timeout=20, desc="customer status sync")

    orders = customer.eval("return get_all_orders()")
    final_status = None
    for order in orders:
        if order.get("id") == order_id:
            final_status = order.get("status")
            break
    print(f"  Customer sees status: {final_status} - STATUS SYNC OK")

    # 6. Summary
    print(f"\n[6/6] Final verification...")
    for order in orders:
        if order.get("id") == order_id:
            print(f"  Order: {order.get('id')}")
            print(f"  Status: {order.get('status')}")
            print(f"  Items: {order.get('items')} x {order.get('quantity')}")
            print(f"  Total: ${order.get('total')}")

    print(f"\n{'=' * 60}")
    print("  [SUCCESS] Full E-Commerce Flow!")
    print(f"{'=' * 60}")
    print("    [OK] Product sync: owner -> customer")
    print("    [OK] Order created and submitted")
    print("    [OK] Derivation: orders -> orders_summary")
    print("    [OK] Owner confirmed order")
    print("    [OK] Status sync: owner -> customer")
    print(f"{'=' * 60}")
