#!/usr/bin/env python3
"""
Diagnostic script to check presence state in group chat.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "scripts"))

from osvauld.scenario import AppTestScenario

DEMOS_APP = Path(__file__).parent / "sample_apps" / "osvauld-demos"

with AppTestScenario(
    name="presence_test",
    app_path=str(DEMOS_APP),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
    },
    keep=True,  # Keep instances running
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")

    # Wait for sync
    print("Waiting for message sync...")
    alice.eval('send_message("Hello from Alice!")')
    bob.wait_for(
        lambda: bob.eval("return get_message_count()") >= 1,
        desc="Bob messages sync",
        timeout=30.0,
    )
    print("Sync complete!")

    # Check presence state via Lua
    print("\n=== Checking Presence State ===\n")

    # Check online_users UI property
    print("Alice's online_users (UI property):")
    alice_users = alice.eval('return ui:get("online_users")')
    print(f"  Result: {alice_users}")

    print("\nBob's online_users (UI property):")
    bob_users = bob.eval('return ui:get("online_users")')
    print(f"  Result: {bob_users}")

    # Check presence layer directly
    print("\n=== Checking Presence Layer Directly ===\n")

    print("Alice's presence layer keys:")
    alice_keys = alice.eval(
        'local layer = scribe:map(scribe:page_id() .. "/presence"); return layer:keys()'
    )
    print(f"  Keys: {alice_keys}")

    print("\nBob's presence layer keys:")
    bob_keys = bob.eval(
        'local layer = scribe:map(scribe:page_id() .. "/presence"); return layer:keys()'
    )
    print(f"  Keys: {bob_keys}")

    # Check each entry in presence layer
    if alice_keys:
        for key in alice_keys:
            entry = alice.eval(
                f'local layer = scribe:map(scribe:page_id() .. "/presence"); return layer:get("{key}")'
            )
            print(f"  Alice - {key}: {entry}")

    if bob_keys:
        for key in bob_keys:
            entry = bob.eval(
                f'local layer = scribe:map(scribe:page_id() .. "/presence"); return layer:get("{key}")'
            )
            print(f"  Bob - {key}: {entry}")

    # Check if binding is working
    print("\n=== Checking Binding Setup ===\n")

    binding_check = alice.eval("""
        -- Check if the binding was set up correctly
        local page_id = scribe:page_id()
        local presence_layer = scribe:map(page_id .. "/presence")
        local keys = presence_layer:keys()
        return {
            page_id = page_id,
            presence_key_count = #keys,
            first_key = keys[1] or "none",
        }
    """)
    print(f"Alice binding check: {binding_check}")

    print("\n=== Done - Keeping instances running ===")
    print("Press Ctrl+C to exit and cleanup")

    # Keep running
    import time

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nShutting down...")
