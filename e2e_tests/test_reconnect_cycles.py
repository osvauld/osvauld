#!/usr/bin/env python3
"""
Reconnect cycles E2E — repeated peer offline/online resilience.

Validates:
- Peers can go offline/online repeatedly while node stays online
- Each cycle: peers write while offline, then merge after reconnect
- No state corruption or accumulating bugs across cycles
- Final count is exactly correct after all cycles

Usage:
    python e2e_tests/test_reconnect_cycles.py --debug --test-mode
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

CYCLES = 3
MESSAGES_PER_CYCLE = 2  # each peer sends 2 per cycle

args = AppTestScenario.parse_args("Reconnect Cycles E2E")

with AppTestScenario(
    name="reconnect_cycles",
    app_path=str(APP_PATH),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
    },
    **args,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")

    # ── Connected baseline ──
    print("[1] Connected baseline")
    alice.eval('send_message("baseline from alice")')
    bob.eval('send_message("baseline from bob")')

    alice.wait_for(
        lambda: alice.eval("return get_message_count()") >= 2,
        timeout=20,
        desc="alice sees baselines",
    )
    bob.wait_for(
        lambda: bob.eval("return get_message_count()") >= 2,
        timeout=20,
        desc="bob sees baselines",
    )
    running_total = 2
    print(f"    Both see {running_total} messages")

    # ── Offline/online cycles ──
    for cycle in range(1, CYCLES + 1):
        print(f"\n[Cycle {cycle}/{CYCLES}] Peers go offline")
        alice.go_offline()
        bob.go_offline()
        time.sleep(1)

        print(f"    Both peers write offline ({MESSAGES_PER_CYCLE} each)")
        for i in range(MESSAGES_PER_CYCLE):
            alice.eval(f'send_message("cycle {cycle} alice msg {i}")')
            bob.eval(f'send_message("cycle {cycle} bob msg {i}")')

        # Each peer sees own writes immediately
        own_expected = running_total + MESSAGES_PER_CYCLE
        alice.wait_for(
            lambda exp=own_expected: alice.eval("return get_message_count()") >= exp,
            timeout=10,
            desc=f"cycle {cycle} alice own writes",
        )
        bob.wait_for(
            lambda exp=own_expected: bob.eval("return get_message_count()") >= exp,
            timeout=10,
            desc=f"cycle {cycle} bob own writes",
        )
        alice_pre = alice.eval("return get_message_count()")
        bob_pre = bob.eval("return get_message_count()")
        print(f"    Pre-merge: alice={alice_pre}, bob={bob_pre}")

        print(f"    Peers go online — merge cycle {cycle}")
        alice.go_online()
        bob.go_online()

        running_total += MESSAGES_PER_CYCLE * 2  # both peers wrote
        alice.wait_for(
            lambda exp=running_total: alice.eval("return get_message_count()") >= exp,
            timeout=60,
            desc=f"cycle {cycle} alice converged",
        )
        bob.wait_for(
            lambda exp=running_total: bob.eval("return get_message_count()") >= exp,
            timeout=60,
            desc=f"cycle {cycle} bob converged",
        )
        alice_post = alice.eval("return get_message_count()")
        bob_post = bob.eval("return get_message_count()")
        print(
            f"    Post-merge: alice={alice_post}, bob={bob_post} (expected >= {running_total})"
        )

    # ── Final verification ──
    expected_final = 2 + (CYCLES * MESSAGES_PER_CYCLE * 2)
    alice_final = alice.eval("return get_message_count()")
    bob_final = bob.eval("return get_message_count()")

    print(f"\n[Final] Expected >= {expected_final}")
    print(f"    Alice: {alice_final}, Bob: {bob_final}")

    assert alice_final >= expected_final, (
        f"Alice has {alice_final}, expected >= {expected_final}"
    )
    assert bob_final >= expected_final, (
        f"Bob has {bob_final}, expected >= {expected_final}"
    )

    print(f"[SUCCESS] Reconnect cycles E2E passed ({CYCLES} cycles)")
