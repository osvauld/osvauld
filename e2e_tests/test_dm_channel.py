#!/usr/bin/env python3
"""
DM (Explicit Grant) Channel E2E Test with Capture.

Tests the explicit-grant dynamic layer flow:
  1. Wait for presence to sync (all peers see each other online)
  2. The canonical DM owner (smaller DID) creates the DM layer
  3. Node receives DM data, writes to the other peer's __sync_meta
  4. The other peer discovers via __sync_meta, sends LayerSubscribe
  5. The other peer receives the DM message
  6. Carol should NOT receive the DM data (isolation)

Usage:
    python e2e_tests/test_dm_channel.py --keep
    python e2e_tests/test_dm_channel.py --keep --capture-logs
"""

import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "group-chat"

args = AppTestScenario.parse_args("DM Channel E2E")

with AppTestScenario(
    name="dm_channel",
    app_path=str(APP_PATH),
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

    include_logs = "--capture-logs" in sys.argv

    # Get DIDs for all peers
    alice_did = alice.eval("return scribe:my_did()")
    bob_did = bob.eval("return scribe:my_did()")
    carol_did = carol.eval("return scribe:my_did()")

    print(f"\n=== DM Channel E2E (3 peers) ===")
    print(f"  Alice DID: {alice_did}")
    print(f"  Bob DID:   {bob_did}")
    print(f"  Carol DID: {carol_did}")

    # Use page owner as DM creator for stable E2E behavior.
    # In current policy wiring, viewer-initiated DM creation may not have
    # manage_layer_access capability to grant the other participant.
    creator, receiver = alice, bob
    creator_name, receiver_name = "alice", "bob"
    creator_did, receiver_did = alice_did, bob_did

    print(f"  DM creator: {creator_name} (page owner)")

    # Step 0: Wait for presence to sync — all peers must see each other online
    print(f"\n[0] Waiting for presence to sync...")
    try:
        alice.wait_for(
            lambda: (
                alice.eval("return get_online_count()")
                and alice.eval("return get_online_count()") >= 2
            ),
            timeout=15,
            desc="alice sees 2+ online peers",
        )
        bob.wait_for(
            lambda: (
                bob.eval("return get_online_count()")
                and bob.eval("return get_online_count()") >= 2
            ),
            timeout=15,
            desc="bob sees 2+ online peers",
        )
    except TimeoutError as e:
        print(f"    WARN: {e}")

    alice_online = alice.eval("return get_online_count()")
    bob_online = bob.eval("return get_online_count()")
    carol_online = carol.eval("return get_online_count()")
    print(
        f"    Online counts: alice={alice_online} bob={bob_online} carol={carol_online}"
    )

    # Start capture
    captures_dir = s.capture_start("dm_test", include_logs=include_logs)
    print(f"  Captures: {captures_dir}/")

    # Step 1: Canonical owner creates a DM with the other peer (not carol)
    print(f"\n[1] {creator_name} creates DM with {receiver_name}...")
    creator.eval(f'create_dm("{receiver_did}", "{receiver_name}")')
    time.sleep(1)

    creator_active_dm = creator.eval("return get_active_dm()")
    print(f"    {creator_name} active DM: {creator_active_dm}")

    # Step 2: Creator sends a message in the DM
    print(f"[2] {creator_name} sends message in DM...")
    creator.eval('send_dm_message("hello, this is a private DM")')

    # Step 3: Wait for sync to node + explicit grant propagation
    print(f"[3] Waiting 8s for sync to node + __sync_meta fanout to {receiver_name}...")
    time.sleep(8)

    # Step 4: Check receiver's state
    print(f"[4] Checking {receiver_name}'s DM state...")
    receiver_active_dm = receiver.eval("return get_active_dm()")
    receiver_dm_count = receiver.eval("return get_dm_message_count()")
    print(f"    {receiver_name} active DM: {receiver_active_dm}")
    print(f"    {receiver_name} DM message count: {receiver_dm_count}")

    # Try to have receiver open the DM
    print(f"[5] {receiver_name} tries to open DM with {creator_name}...")
    receiver_open_result = receiver.eval(
        f'return open_dm("{creator_did}", "{creator_name}")'
    )
    print(f"    {receiver_name} open_dm result: {receiver_open_result}")
    time.sleep(2)

    receiver_active_dm = receiver.eval("return get_active_dm()")
    receiver_dm_count = receiver.eval("return get_dm_message_count()")
    print(f"    {receiver_name} active DM after open: {receiver_active_dm}")
    print(f"    {receiver_name} DM message count after open: {receiver_dm_count}")

    # Step 5b: Check DM display names — both sides should see the other's name.
    # `get_active_dm_name()` was never exported via api.export(); the display name
    # is stored in UI state by dms.switch_dm() as ui:set("active_dm_name", ...).
    # We read it with ui:get("active_dm_name") which is always available in eval.
    print(f"\n[5b] Checking DM display names (both sides should see other's name)...")
    creator_dm_name = creator.eval(
        'local ok, v = pcall(function() return ui:get("active_dm_name") end); '
        "if ok then return v else return nil end"
    )
    receiver_dm_name = receiver.eval(
        'local ok, v = pcall(function() return ui:get("active_dm_name") end); '
        "if ok then return v else return nil end"
    )
    print(f"    {creator_name} sees DM named: '{creator_dm_name}'")
    print(f"    {receiver_name} sees DM named: '{receiver_dm_name}'")

    # Creator should see receiver's name; receiver should see creator's name.
    # We check that neither side sees an opaque DID-like string (8+ hex chars with no spaces).
    # A valid display name is the peer's username (e.g., "alice", "bob").
    creator_name_ok = creator_dm_name and creator_dm_name == receiver_name
    receiver_name_ok = receiver_dm_name and receiver_dm_name == creator_name
    print(
        f"    {creator_name} name check: {'PASS' if creator_name_ok else 'FAIL'} "
        f"(expected '{receiver_name}', got '{creator_dm_name}')"
    )
    print(
        f"    {receiver_name} name check: {'PASS' if receiver_name_ok else 'FAIL'} "
        f"(expected '{creator_name}', got '{receiver_dm_name}')"
    )

    # Step 6: Check carol's state (should have NOTHING)
    print(f"[6] Checking carol's DM state (should be empty)...")
    carol_active_dm = carol.eval("return get_active_dm()")
    carol_dm_count = carol.eval("return get_dm_message_count()")
    print(f"    Carol active DM: {carol_active_dm}")
    print(f"    Carol DM message count: {carol_dm_count}")

    # Carol tries to open the DM (should fail)
    carol_open_result = carol.eval(f'return open_dm("{creator_did}", "{creator_name}")')
    print(f"    Carol open_dm result: {carol_open_result}")

    # Step 7: Creator sends another message
    print(f"[7] {creator_name} sends second message...")
    creator.eval('send_dm_message("second DM message")')
    time.sleep(3)

    # Final counts
    creator_dm_count = creator.eval("return get_dm_message_count()")
    receiver_dm_count = receiver.eval("return get_dm_message_count()")
    carol_dm_count = carol.eval("return get_dm_message_count()")
    print(f"\n  Final DM message counts:")
    print(f"    {creator_name} (creator): {creator_dm_count}")
    print(f"    {receiver_name} (receiver): {receiver_dm_count}")
    print(f"    carol: {carol_dm_count}")

    # Stop capture
    merged = s.capture_end("dm_test", merge=True)

    # ================================================================
    # Capture Analysis
    # ================================================================
    print(f"\n{'=' * 60}")
    print(f"=== Capture Analysis ===")
    print(f"{'=' * 60}")

    if not merged or not merged.exists():
        print("  No capture data!")
        sys.exit(1)

    events = [json.loads(line) for line in open(merged)]
    print(f"  Total events: {len(events)}")

    # Group by instance
    by_instance = {}
    for e in events:
        inst = e.get("instance", "?")
        by_instance.setdefault(inst, []).append(e)
    for inst, evts in sorted(by_instance.items()):
        print(f"    {inst}: {len(evts)} events")

    # --- DM layer creation ---
    print(f"\n--- Layer creation (dms/) ---")
    creates = [
        e
        for e in events
        if e.get("type") == "layer_auth" and "dms/" in e.get("layer", "")
    ]
    if not creates:
        creates = [
            e
            for e in events
            if "dms/" in str(e.get("layer", ""))
            and e.get("type") in ("apply_update", "layer_auth")
        ]
    for e in creates:
        print(
            f"  [{e.get('instance')}] type={e.get('type')} layer={e.get('layer_short', e.get('layer', ''))} "
            f"did={e.get('short_did', '')} action={e.get('action', e.get('result', ''))}"
        )

    # --- __sync_meta events (focus on receiver's and carol's) ---
    print(f"\n--- __sync_meta events ---")
    sync_meta = [e for e in events if "__sync_meta" in str(e.get("layer", ""))]
    sync_meta_summary = {}
    for e in sync_meta:
        inst = e.get("instance", "?")
        layer = e.get("layer_short", e.get("layer", ""))
        typ = e.get("type", "?")
        key = f"[{inst}] {typ} {layer}"
        sync_meta_summary[key] = sync_meta_summary.get(key, 0) + 1
    for key, count in sorted(sync_meta_summary.items()):
        print(f"  {key}: {count}")

    # Highlight: was receiver's __sync_meta ever written to?
    receiver_sm = [
        e for e in sync_meta if receiver_did[-12:] in str(e.get("layer", ""))
    ]
    carol_sm = [e for e in sync_meta if carol_did[-12:] in str(e.get("layer", ""))]
    print(f"\n  {receiver_name}'s __sync_meta events: {len(receiver_sm)}")
    print(f"  Carol's __sync_meta events: {len(carol_sm)}")

    # --- LayerSubscribe repetition check (regression detector) ---
    print(f"\n--- LayerSubscribe repetition check ---")
    ls_events = [
        e
        for e in events
        if e.get("type") == "layer_subscribe_protocol" and e.get("direction") == "send"
    ]
    ls_by_layer = {}
    for e in ls_events:
        key = (e.get("instance", "?"), e.get("layer", "?"))
        ls_by_layer[key] = ls_by_layer.get(key, 0) + 1
    repeated = {k: v for k, v in ls_by_layer.items() if v > 1}
    if repeated:
        for (inst, layer), count in sorted(repeated.items()):
            print(f"  WARNING [{inst}] LayerSubscribe sent {count}x for {layer[-40:]}")
    else:
        print(f"  OK: no repeated LayerSubscribes for the same layer")

    # --- apply_update events for DM layers ---
    print(f"\n--- apply_update for dms/ layers ---")
    applies = [
        e
        for e in events
        if e.get("type") == "apply_update" and "dms/" in str(e.get("layer", ""))
    ]
    for e in applies:
        print(
            f"  [{e.get('instance')}] layer={e.get('layer_short', e.get('layer', ''))} "
            f"from={e.get('short_did', 'local')} result={e.get('result', '')}"
        )

    # --- permission_check for dms/ ---
    print(f"\n--- permission_check for dms/ ---")
    perms = [
        e
        for e in events
        if e.get("type") == "permission_check" and "dms/" in str(e.get("layer", ""))
    ]
    for e in perms:
        print(
            f"  [{e.get('instance')}] layer={e.get('layer_short', e.get('layer', ''))} "
            f"peer={e.get('short_did', '')} result={e.get('result', '')} via={e.get('granted_by', '')}"
        )

    # --- layer_auth events ---
    print(f"\n--- layer_auth events ---")
    auth = [e for e in events if e.get("type") == "layer_auth"]
    for e in auth:
        print(
            f"  [{e.get('instance')}] layer={e.get('layer_short', e.get('layer', ''))} "
            f"did={e.get('short_did', '')} action={e.get('action', '')}"
        )

    # --- broadcast_decision for dms/ ---
    print(f"\n--- broadcast_decision for dms/ ---")
    broadcasts = [
        e
        for e in events
        if e.get("type") == "broadcast_decision" and "dms/" in str(e.get("layer", ""))
    ]
    for e in broadcasts:
        print(
            f"  [{e.get('instance')}] layer={e.get('layer_short', e.get('layer', ''))} "
            f"peer={e.get('short_did', '')} decision={e.get('decision', '')} reason={e.get('reason', '')}"
        )

    # --- subscriber_state events ---
    print(f"\n--- subscriber_state events ---")
    sub_states = [e for e in events if e.get("type") == "subscriber_state"]
    for e in sub_states:
        print(
            f"  [{e.get('instance')}] peer={e.get('short_did', '')} "
            f"layers={e.get('authorized_layers_short', [])} "
            f"total={e.get('total_layers', '')}"
        )

    # --- All events from receiver ---
    print(f"\n--- All {receiver_name} events ---")
    receiver_evts = by_instance.get(receiver_name, [])
    if receiver_evts:
        for e in receiver_evts:
            print(
                f"  type={e.get('type')} layer={e.get('layer_short', e.get('layer', ''))} "
                f"action={e.get('action', e.get('result', ''))} "
                f"did={e.get('short_did', '')}"
            )
    else:
        print("  (none)")

    # --- All events from carol ---
    print(f"\n--- All carol events ---")
    carol_evts = by_instance.get("carol", [])
    if carol_evts:
        for e in carol_evts:
            print(
                f"  type={e.get('type')} layer={e.get('layer_short', e.get('layer', ''))} "
                f"action={e.get('action', e.get('result', ''))} "
                f"did={e.get('short_did', '')}"
            )
    else:
        print("  (none)")

    # --- Log events mentioning DM or explicit or sync_meta (if --capture-logs) ---
    if include_logs:
        print(f"\n--- Log events (dm/explicit/sync_meta/LayerSubscribe/authority) ---")
        log_evts = [
            e
            for e in events
            if e.get("type") == "log"
            and any(
                kw in str(e.get("msg", "")).lower()
                for kw in (
                    "dms/",
                    "explicit",
                    "sync_meta",
                    "layersubscribe",
                    "layer_subscribe",
                    "layer_access",
                    "authority",
                    "add_layer",
                    "manage_layer",
                )
            )
        ]
        for e in log_evts[:80]:  # cap at 80
            print(
                f"  [{e.get('instance')}] {e.get('level', '')} {e.get('target', '')}: "
                f"{e.get('msg', '')[:150]}"
            )

    # Result
    print(f"\n{'=' * 60}")
    receiver_ok = receiver_dm_count and receiver_dm_count >= 1
    carol_isolated = not carol_dm_count or carol_dm_count == 0
    names_ok = creator_name_ok and receiver_name_ok
    if (
        creator_dm_count
        and creator_dm_count >= 2
        and receiver_ok
        and carol_isolated
        and names_ok
    ):
        print(f"  PASS: DM synced to {receiver_name}, carol isolated, names correct")
        print(
            f"    {creator_name}={creator_dm_count} {receiver_name}={receiver_dm_count} carol={carol_dm_count}"
        )
        print(
            f"    {creator_name} sees '{creator_dm_name}', {receiver_name} sees '{receiver_dm_name}'"
        )
    else:
        print(
            f"  FAIL: {creator_name}={creator_dm_count} {receiver_name}={receiver_dm_count} carol={carol_dm_count}"
        )
        if not receiver_ok:
            print(f"    {receiver_name} should have >= 1 messages")
        if not carol_isolated:
            print(f"    Carol should have 0 messages (isolation broken!)")
        if not names_ok:
            print(f"    DM display names incorrect:")
            if not creator_name_ok:
                print(
                    f"      {creator_name} sees '{creator_dm_name}', expected '{receiver_name}'"
                )
            if not receiver_name_ok:
                print(
                    f"      {receiver_name} sees '{receiver_dm_name}', expected '{creator_name}'"
                )
        print(f"  Captures: {merged}")
    print(f"{'=' * 60}")
