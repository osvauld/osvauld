#!/usr/bin/env python3
"""
Daily shards offline-first E2E — 3 peers with time progression.

Validates:
- Connected phase: all 3 peers exchange messages on day 0
- Charlie goes offline while Alice/Bob advance time and write across day shards
- Node goes down, Charlie restarts offline and writes locally into same-day shard
- Node restarts, all auto-reconnect
- Charlie catches up: sees all online writes + his offline writes merge into same shard

Usage:
    python e2e_tests/test_chat_daily_shards_offline_3peers.py --test-mode --debug
"""

import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

# 2026-02-20 00:00:00 UTC
BASE_TIME = 1_740_009_600
DAY_SECONDS = 86_400
HOUR_SECONDS = 3_600
STEP_SECONDS = DAY_SECONDS + HOUR_SECONDS

args = AppTestScenario.parse_args("Daily Shards Offline 3-Peer E2E")

with AppTestScenario(
    name="daily_shards_offline_3p",
    app_path=str(APP_PATH),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
        "charlie": {"role": "viewer", "app": "Group Chat"},
    },
    **args,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")
    charlie = s.peer("charlie")

    # ── Phase 1: Set deterministic time ──
    print("[1/7] Set deterministic base time on all peers")
    alice.set_time(BASE_TIME)
    bob.set_time(BASE_TIME)
    charlie.set_time(BASE_TIME)
    print(f"    Base: {BASE_TIME} (2026-02-20 00:00:00 UTC)")

    # ── Phase 2: Connected baseline — all 3 peers send on day 0 ──
    print("[2/7] Connected baseline — all peers send on day 0")
    alice.eval('send_message("day 0 from alice")')
    bob.eval('send_message("day 0 from bob")')
    charlie.eval('send_message("day 0 from charlie")')

    for name, peer in [("alice", alice), ("bob", bob), ("charlie", charlie)]:
        peer.wait_for(
            lambda p=peer: p.eval("return get_message_count()") >= 3,
            timeout=20,
            desc=f"{name} sees all 3 day-0 messages",
        )
    print("    All peers see 3 messages")

    # ── Phase 3: Take charlie offline ──
    print("[3/7] Take charlie offline")
    s.stop_peer("charlie")
    time.sleep(1)

    # ── Phase 4: Alice/Bob write across day 1-3 shards ──
    print("[4/7] Alice/Bob write across day 1-3 while charlie is offline")
    for day in range(1, 4):
        alice.advance_time(STEP_SECONDS)
        bob.advance_time(STEP_SECONDS)
        sender = alice if day % 2 == 1 else bob
        sender_name = "alice" if day % 2 == 1 else "bob"
        sender.eval(f'send_message("day {day} from {sender_name}")')

    online_count = 6  # 3 baseline + 3 days
    alice.wait_for(
        lambda: alice.eval("return get_message_count()") >= online_count,
        timeout=30,
        desc="alice has all online messages",
    )
    bob.wait_for(
        lambda: bob.eval("return get_message_count()") >= online_count,
        timeout=30,
        desc="bob has all online messages",
    )
    print(f"    Alice/Bob each have {online_count} messages across 4 day shards")

    # ── Phase 5: Stop node, restart charlie offline, write locally ──
    print("[5/7] Stop node, restart charlie offline, write locally")
    current_time = BASE_TIME + (3 * STEP_SECONDS)
    s.stop_node()
    time.sleep(1)

    charlie = s.start_peer("charlie", reconnect=False)
    charlie.set_time(current_time)

    charlie.eval('send_message("charlie offline msg 1")')
    charlie.eval('send_message("charlie offline msg 2")')

    charlie.wait_for(
        lambda: charlie.eval("return get_message_count()") >= 2,
        timeout=20,
        desc="charlie sees own offline writes",
    )
    charlie_pre = charlie.eval("return get_message_count()")
    print(f"    Charlie wrote 2 offline messages (sees {charlie_pre} total)")

    # ── Phase 6: Restart node, wait for auto-reconnect + merge ──
    print("[6/7] Restart node — auto-reconnect and merge")
    s.start_node()

    expected_total = online_count + 2  # 6 online + 2 charlie offline = 8

    charlie.wait_for(
        lambda: charlie.eval("return get_message_count()") >= expected_total,
        timeout=60,
        desc="charlie catches up after merge",
    )
    alice.wait_for(
        lambda: alice.eval("return get_message_count()") >= expected_total,
        timeout=60,
        desc="alice receives charlie offline writes",
    )
    bob.wait_for(
        lambda: bob.eval("return get_message_count()") >= expected_total,
        timeout=60,
        desc="bob receives charlie offline writes",
    )
    print(f"    All peers converged to {expected_total} messages")

    # ── Phase 7: Verify same-day shard merge ──
    print("[7/7] Verify same-day shard contains both online and offline writes")
    day3_period = datetime.fromtimestamp(current_time, timezone.utc).strftime(
        "%Y-%m-%d"
    )

    charlie.wait_for(
        lambda: charlie.eval(
            """
local period = "{period}"
local layers = scribe:list_layers("channels/general/messages/" .. period)
if not layers or #layers == 0 then return false end
local layer = scribe:map(scribe:page_id() .. "/" .. layers[1])
local found_online = false
local found_offline = false
for _, key in ipairs(layer:keys() or {}) do
    local msg = layer:get(key)
    if msg and msg.text == "day 3 from alice" then
        found_online = true
    end
    if msg and msg.text == "charlie offline msg 1" then
        found_offline = true
    end
end
return found_online and found_offline
""".replace("{period}", day3_period)
        ),
        timeout=30,
        desc="day-3 shard has both online + offline writes",
    )

    final = {
        "alice": alice.eval("return get_message_count()"),
        "bob": bob.eval("return get_message_count()"),
        "charlie": charlie.eval("return get_message_count()"),
    }
    print(f"    Final counts: {final}")
    print("[SUCCESS] Daily shards offline-first 3-peer merge E2E passed")
