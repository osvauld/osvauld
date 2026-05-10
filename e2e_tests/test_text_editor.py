#!/usr/bin/env python3
"""
Text Editor E2E Test — first end-to-end exercise of the LoroTree layer type.

Validates Step 0 (LoroTree wiring) and Step A (the text-editor app) together:
- App imports `RichTextEdit` from `@osvauld/widgets`
- `scribe:bind_tree("blocks", DOC)` sets up reactive sync between tree
  layer and the Slint VecModel
- Tree ops (create / set_prop / delete) propagate from alice to bob and
  bob's `blocks` model converges
- Block ids stay stable across peers (so editor row identity holds)

Usage:
    python e2e_tests/test_text_editor.py
    python e2e_tests/test_text_editor.py --keep
    python e2e_tests/test_text_editor.py --debug
"""

import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "text-editor"

args = AppTestScenario.parse_args("Text Editor E2E Test")

with AppTestScenario(
    name="text_editor_test",
    app_path=str(APP_PATH),
    peers={
        "alice": {"role": "owner", "app": "Text Editor"},
        "bob": {"role": "viewer", "app": "Text Editor"},
    },
    **args,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")

    # On init each peer seeds an empty paragraph if the doc is empty.
    # After sync converges they should share at least one block, with
    # block ids stable on both sides.
    print("[1/5] Waiting for initial seed to converge...")
    bob.wait_for(
        lambda: bob.eval("return get_block_count()") >= 1,
        desc="bob sees seeded block",
    )

    # Alice appends a second block with text.
    print("[2/5] Alice adds 'hello world' as a new block...")
    new_id = alice.eval('return add_block("hello world")')
    assert isinstance(new_id, str) and new_id, f"alice add_block returned {new_id!r}"
    alice.wait_for(
        lambda: any(
            alice.eval(f"return get_block_text({i})") == "hello world"
            for i in range(1, alice.eval("return get_block_count()") + 1)
        ),
        desc="alice has hello world",
    )

    # Bob should converge to the same content (text-equality is enough — block
    # ids should be identical across peers because LoroTree node ids are
    # globally unique, but the test doesn't depend on positional order).
    print("[3/5] Waiting for sync to bob...")
    bob.wait_for(
        lambda: any(
            bob.eval(f"return get_block_text({i})") == "hello world"
            for i in range(1, bob.eval("return get_block_count()") + 1)
        ),
        desc="bob sees hello world block",
    )

    # Alice mutates the second block's text. set_prop generates a tree op
    # under the hood — we want bob's existing row to update in place.
    print("[4/5] Alice rewrites the new block...")
    # Locate the 'hello world' row by index from alice's perspective.
    count_a = alice.eval("return get_block_count()")
    target_index = None
    for i in range(1, count_a + 1):
        if alice.eval(f"return get_block_text({i})") == "hello world":
            target_index = i
            break
    assert target_index is not None
    alice.eval(f'set_block_text({target_index}, "the quick brown fox")')
    bob.wait_for(
        lambda: any(
            bob.eval(f"return get_block_text({i})") == "the quick brown fox"
            for i in range(1, bob.eval("return get_block_count()") + 1)
        ),
        desc="bob sees rewritten text",
    )

    # And verify the older 'hello world' is gone (was the same node).
    print("[5/5] Confirming bob no longer has 'hello world'...")
    has_old = any(
        bob.eval(f"return get_block_text({i})") == "hello world"
        for i in range(1, bob.eval("return get_block_count()") + 1)
    )
    assert not has_old, "bob still sees stale 'hello world' after rewrite"

    # ── Concurrent writes: char-level CRDT merge on the same block ──
    #
    # This is the regression check for Step 3 (per-block LoroText with
    # creator-materialised container). Both peers fire `insert` ops at
    # opposite ends of the same block within the broadcast window, so
    # neither has applied the other's update yet — the writes are
    # CRDT-concurrent. Loro's text CRDT must converge both peers to the
    # *same* string and that string must contain *all* characters from
    # both contributions. Pre-Step-3 (lazy `meta.insert_container`), one
    # peer's container would be orphaned by LoroMap LWW and their
    # characters would silently disappear; this test fails loudly there.
    print("[6/8] Setting up a fresh block for concurrent edits...")
    concurrent_id = alice.eval('return add_block("")')
    assert isinstance(concurrent_id, str) and concurrent_id

    # Resolve the row index of the empty block on each peer — block
    # ordering across peers is stable because LoroTree node ids match,
    # but the *index* each peer assigns can shift if either has more or
    # fewer locally-seeded blocks. Look up by id.
    def index_for_id(peer, target_id):
        count = peer.eval("return get_block_count()")
        for i in range(1, count + 1):
            if peer.eval(f"return get_block_id({i})") == target_id:
                return i
        return None

    bob.wait_for(
        lambda: index_for_id(bob, concurrent_id) is not None,
        desc="bob sees the new empty block",
    )

    alice_idx = index_for_id(alice, concurrent_id)
    bob_idx = index_for_id(bob, concurrent_id)
    assert alice_idx and bob_idx

    # Fire both inserts in parallel. Alice prepends "ABC" at position 0,
    # bob appends "xyz" — also at position 0 of his local snapshot, since
    # the block is empty on both sides at this moment. Loro's text CRDT
    # treats them as concurrent inserts and produces a deterministic
    # interleaving (both orderings — "ABCxyz" / "xyzABC" — are valid;
    # whichever Loro's tiebreak picks is what *both* peers must converge
    # to).
    print("[7/8] Firing concurrent inserts from alice and bob...")
    with ThreadPoolExecutor(max_workers=2) as ex:
        fa = ex.submit(alice.eval, f'insert_at_block({alice_idx}, 0, "ABC")')
        fb = ex.submit(bob.eval, f'insert_at_block({bob_idx}, 0, "xyz")')
        fa.result()
        fb.result()

    print("[8/8] Waiting for both peers to converge on a merged string...")
    # Convergence assertion: both peers see the *same* text, and that text
    # is one of the two valid CRDT interleavings of "ABC" + "xyz". We
    # don't pin which interleaving — that's a Loro internal detail — only
    # that no character is dropped and both peers agree.
    def merged_text(peer):
        idx = index_for_id(peer, concurrent_id)
        return peer.eval(f"return get_block_text({idx})") if idx else None

    valid_merges = {"ABCxyz", "xyzABC"}
    alice.wait_for(
        lambda: merged_text(alice) in valid_merges,
        desc="alice merged to one of the valid interleavings",
    )
    bob.wait_for(
        lambda: merged_text(bob) in valid_merges,
        desc="bob merged to one of the valid interleavings",
    )
    final_a = merged_text(alice)
    final_b = merged_text(bob)
    assert final_a == final_b, (
        f"peers diverged on concurrent edits: alice={final_a!r}, bob={final_b!r}"
    )
    assert "ABC" in final_a and "xyz" in final_a, (
        f"merged text dropped a contribution: {final_a!r}"
    )
    print(f"  → both peers converged to {final_a!r} (alice's + bob's chars present)")

    print(f"\n{'=' * 60}")
    print("  [SUCCESS] Text Editor E2E Test Passed!")
    print(f"{'=' * 60}")
