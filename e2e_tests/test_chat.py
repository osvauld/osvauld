#!/usr/bin/env python3
"""
Group Chat E2E Test — bidirectional message sync.

Tests:
- Alice sends message -> syncs to Bob and Carol
- Bob replies -> syncs back to Alice and Carol

Usage:
    python e2e_tests/test_chat.py
    python e2e_tests/test_chat.py --keep
    python e2e_tests/test_chat.py --debug
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

args = AppTestScenario.parse_args("Group Chat E2E Test")

with AppTestScenario(
    name="chat_test",
    app_path=str(DEMOS_APP),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
        "carol": {"role": "viewer", "app": "Group Chat"},
    },
    **args,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")
    carol = s.peer("carol")

    # Alice sends a message
    print("[1/4] Alice sends message...")
    alice.eval('send_message("Hello from Alice!")')
    alice.wait_for(
        lambda: alice.eval("return get_message_count()") >= 1,
        desc="alice message count",
    )
    print(f"  Alice has {alice.eval('return get_message_count()')} message(s)")

    # Wait for sync to Bob and Carol
    print("[2/4] Waiting for sync to Bob and Carol...")
    bob.wait_for(
        lambda: bob.eval("return get_message_count()") >= 1,
        desc="Bob messages sync",
    )
    carol.wait_for(
        lambda: carol.eval("return get_message_count()") >= 1,
        desc="Carol messages sync",
    )
    print(f"  Bob has {bob.eval('return get_message_count()')} message(s) - SYNC OK")
    print(f"  Carol has {carol.eval('return get_message_count()')} message(s) - SYNC OK")

    # Bob replies
    print("[3/4] Bob sends reply...")
    bob.eval('send_message("Hello back from Bob!")')

    # Wait for sync to Alice and Carol
    print("[4/4] Waiting for sync to Alice and Carol...")
    alice.wait_for(
        lambda: alice.eval("return get_message_count()") >= 2,
        desc="Alice reply sync",
    )
    carol.wait_for(
        lambda: carol.eval("return get_message_count()") >= 2,
        desc="Carol reply sync",
    )
    print(f"  Alice has {alice.eval('return get_message_count()')} message(s) - BIDIRECTIONAL SYNC OK")
    print(f"  Carol has {carol.eval('return get_message_count()')} message(s) - THREE-WAY SYNC OK")

    print(f"\n{'=' * 60}")
    print("  [SUCCESS] Group Chat E2E Test Passed!")
    print(f"{'=' * 60}")
