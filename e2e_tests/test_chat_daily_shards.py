#!/usr/bin/env python3
"""
Group Chat daily shard E2E.

Validates that:
- messages written to today's shard sync to viewer
- a second day shard can be created and synced
- app-level message count aggregates across day shards
- time control via RPC (not Lua monkeypatch)
"""

import sys
from pathlib import Path
from datetime import datetime, timezone

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "group-chat"

# Base time: 2026-02-20 00:00:00 UTC
BASE_TIME = 1_740_009_600
DAY_SECONDS = 86_400
HOUR_SECONDS = 3_600
STEP_SECONDS = DAY_SECONDS + HOUR_SECONDS
MESSAGES_PER_DAY = 3

args = AppTestScenario.parse_args("Group Chat Daily Shards E2E")

with AppTestScenario(
    name="chat_daily_shards",
    app_path=str(APP_PATH),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
    },
    **args,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")

    print("[1/5] Set deterministic base time on both peers")
    alice.set_time(BASE_TIME)
    bob.set_time(BASE_TIME)
    print(f"    Base time: {BASE_TIME} (2026-02-20 00:00:00 UTC)")

    print("[2/5] Alice and Bob send multiple messages per day for 7 days")
    for day in range(7):
        for idx in range(MESSAGES_PER_DAY):
            sender = alice if (day + idx) % 2 == 0 else bob
            sender_name = "alice" if (day + idx) % 2 == 0 else "bob"
            sender.eval(f'send_message("day {day} msg {idx} from {sender_name}")')
        if day < 6:
            alice.advance_time(STEP_SECONDS)
            bob.advance_time(STEP_SECONDS)
    total_seeded = 7 * MESSAGES_PER_DAY
    print(f"    Sent {total_seeded} messages across 7 days")

    print("[3/5] Bob aggregates all seeded shards")
    bob.wait_for(
        lambda: bob.eval("return get_message_count()") >= total_seeded,
        timeout=30,
        desc="bob aggregated message count across 7 days",
    )
    count = bob.eval("return get_message_count()")
    print(f"    Bob sees {count} messages")

    print("[4/5] Verify day 6 message is in the correct shard")
    day6_period = datetime.fromtimestamp(
        BASE_TIME + (6 * STEP_SECONDS), timezone.utc
    ).strftime("%Y-%m-%d")
    bob.wait_for(
        lambda: bob.eval(
            """
local period = "{day6_period}"
local layers = scribe:list_layers("channels/general/messages/" .. period)
if not layers or #layers == 0 then
    return false
end
local layer = scribe:map(scribe:page_id() .. "/" .. layers[1])
local keys = layer:keys() or {}
for _, key in ipairs(keys) do
    local msg = layer:get(key)
    if msg and msg.text == "day 6 msg 2 from alice" then
        return true
    end
end
return false
""".replace("{day6_period}", day6_period)
        ),
        timeout=20,
        desc="bob has day 6 message in correct shard",
    )
    print("    Day 6 message verified in correct shard")

    print("[5/5] Advance one week and send message")
    alice.advance_time((7 * DAY_SECONDS) + HOUR_SECONDS)
    bob.advance_time((7 * DAY_SECONDS) + HOUR_SECONDS)
    bob.eval('send_message("week-ahead message from bob")')

    week_ahead_period = datetime.fromtimestamp(
        BASE_TIME + (6 * STEP_SECONDS) + (7 * DAY_SECONDS) + HOUR_SECONDS,
        timezone.utc,
    ).strftime("%Y-%m-%d")

    bob.wait_for(
        lambda: bob.eval("return get_message_count()") >= (total_seeded + 1),
        timeout=30,
        desc="bob aggregated week-ahead send",
    )

    bob.wait_for(
        lambda: bob.eval(
            """
local period = "{week_ahead_period}"
local layers = scribe:list_layers("channels/general/messages/" .. period)
if not layers or #layers == 0 then
    return false
end
local layer = scribe:map(scribe:page_id() .. "/" .. layers[1])
local keys = layer:keys() or {}
for _, key in ipairs(keys) do
    local msg = layer:get(key)
    if msg and msg.text == "week-ahead message from bob" then
        return true
    end
end
return false
""".replace("{week_ahead_period}", week_ahead_period)
        ),
        timeout=30,
        desc="bob has week-ahead message in day 7 shard",
    )

    final_count = bob.eval("return get_message_count()")
    print(f"    Final count: {final_count} messages")

    print("[SUCCESS] Daily shard week-forward E2E passed")
