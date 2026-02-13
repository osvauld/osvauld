#!/usr/bin/env python3
"""
Query presence state from running instances.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "scripts"))

from osvauld.client import ControlClient

# Query Alice
print("=== Querying Alice ===")
alice = ControlClient("/tmp/presence_test/alice/alice.sock")

try:
    # Check online_users UI property
    print("\n1. online_users UI property:")
    result = alice.eval('return ui:get("online_users")')
    print(f"   Result: {result}")
    print(f"   Type: {type(result)}")
    print(f"   Length: {len(result) if isinstance(result, list) else 'N/A'}")

    # Check presence layer directly
    print("\n2. Presence layer keys:")
    keys = alice.eval(
        'local layer = scribe:map(scribe:page_id() .. "/presence"); return layer:keys()'
    )
    print(f"   Keys: {keys}")

    # Get each presence entry
    if keys and len(keys) > 0:
        print("\n3. Presence entries:")
        for key in keys:
            entry = alice.eval(
                f'local layer = scribe:map(scribe:page_id() .. "/presence"); return layer:get("{key}")'
            )
            print(f"   {key}: {entry}")

    # Check my_did
    print("\n4. Alice's DID:")
    did = alice.eval("return scribe:my_did()")
    print(f"   DID: {did}")

except Exception as e:
    print(f"Error: {e}")
    import traceback

    traceback.print_exc()

# Query Bob
print("\n" + "=" * 50)
print("=== Querying Bob ===")
bob = ControlClient("/tmp/presence_test/bob/bob.sock")

try:
    # Check online_users UI property
    print("\n1. online_users UI property:")
    result = bob.eval('return ui:get("online_users")')
    print(f"   Result: {result}")
    print(f"   Type: {type(result)}")
    print(f"   Length: {len(result) if isinstance(result, list) else 'N/A'}")

    # Check presence layer directly
    print("\n2. Presence layer keys:")
    keys = bob.eval(
        'local layer = scribe:map(scribe:page_id() .. "/presence"); return layer:keys()'
    )
    print(f"   Keys: {keys}")

    # Get each presence entry
    if keys and len(keys) > 0:
        print("\n3. Presence entries:")
        for key in keys:
            entry = bob.eval(
                f'local layer = scribe:map(scribe:page_id() .. "/presence"); return layer:get("{key}")'
            )
            print(f"   {key}: {entry}")

    # Check my_did
    print("\n4. Bob's DID:")
    did = bob.eval("return scribe:my_did()")
    print(f"   DID: {did}")

except Exception as e:
    print(f"Error: {e}")
    import traceback

    traceback.print_exc()

print("\n" + "=" * 50)
print("=== Analysis ===")
print("If online_users is empty but presence layer has entries,")
print("the binding may not be working correctly.")
