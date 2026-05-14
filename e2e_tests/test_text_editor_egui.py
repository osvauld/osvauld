#!/usr/bin/env python3
"""
text-editor-egui smoke — single peer.

We're validating the UI binding surface + Lua app, not CRDT sync (which the
Slint text-editor test already covers). Running one peer keeps iteration
fast: open the app, drive operations via the control-socket `eval` bridge,
assert on the resulting tree state.

This variant seeds a realistic Notion-style document (mixed kinds, plain
prose, a todo list, a code block, a numbered list) so the UI gets exercised
the way a user actually loads it — not just one row per kind in isolation.
Useful for eyeballing layout/typography while the test runs with --keep.

Prerequisite:
    cargo build -p sthalam --features egui

Usage:
    python e2e_tests/test_text_editor_egui.py
    python e2e_tests/test_text_editor_egui.py --keep
    python e2e_tests/test_text_editor_egui.py --debug
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "text-editor-egui"

# A representative doc: title, intro paragraph, sub-heading, prose, a todo
# checklist, a numbered plan, a quote, a code snippet, and a closing line.
# Each entry is (kind, text). `done=True` only meaningful for `todo`.
DOC = [
    ("h1",   "Launch checklist — Osvauld text editor"),
    ("p",    "Quick notes from today's session. The egui renderer now supports the full Notion block kinds, with hover-revealed grips and a clean dark surface."),
    ("h2",   "What shipped"),
    ("p",    "We finished the per-kind styling pass and wired hot reload through the control socket. Reload happens without restarting either peer."),
    ("list", "Borderless multiline text edits"),
    ("list", "Hover-revealed grip in a left gutter"),
    ("list", "Drag-and-drop reorder with a ghost preview"),
    ("list", "Top toolbar that converts the focused block"),
    ("h2",   "Open items"),
    ("todo", "Slash menu for inline block creation"),
    ("todo", "Inline formatting (bold / italic / code)"),
    ("todo", "Nested blocks with indent"),
    ("todo", "Tables"),
    ("h3",   "Tomorrow"),
    ("nlist", "Wire the slash menu prototype"),
    ("nlist", "Add an indent/outdent affordance"),
    ("nlist", "Bench frame time with a 500-block doc"),
    ("quote", "Make it work, make it right, make it fast — in that order."),
    ("code",  "fn main() {\n    println!(\"hello, osvauld\");\n}"),
    ("divider", ""),
    ("p",    "End of doc. Drag a block by its grip to reorder."),
]

args = AppTestScenario.parse_args("text-editor-egui smoke")

with AppTestScenario(
    name="text_editor_egui",
    app_path=str(APP_PATH),
    peers={
        "alice": {"role": "owner", "app": "Text Editor Egui"},
    },
    **args,
) as s:
    alice = s.peer("alice")

    print("[1/5] Seed converges to at least one block...")
    alice.wait_for(
        lambda: alice.eval("return get_block_count()") >= 1,
        desc="alice sees seeded block",
    )

    print(f"[2/5] Seed a {len(DOC)}-block realistic document...")
    initial_count = alice.eval("return get_block_count()")
    seeded_ids = []
    for kind, text in DOC:
        # `text` is escaped via repr() so multiline (code block) and quotes
        # survive the round-trip through eval.
        bid = alice.eval(f"return add_block_kind({kind!r}, {text!r})")
        assert isinstance(bid, str) and bid, f"add_block_kind({kind!r}) returned {bid!r}"
        seeded_ids.append(bid)

    expected_count = initial_count + len(DOC)
    actual_count = alice.eval("return get_block_count()")
    assert actual_count == expected_count, (
        f"expected {expected_count} blocks after seeding, got {actual_count}"
    )

    print("[3/5] Round-trip every seeded block (kind + text)...")
    # Build id -> (kind, text) lookup from current tree.
    by_id = {}
    for i in range(1, actual_count + 1):
        bid = alice.eval(f"return get_block_id({i})")
        kind = alice.eval(f"return get_block_kind({i})")
        text = alice.eval(f"return get_block_text({i})")
        by_id[bid] = (kind, text)

    for (expected_kind, expected_text), bid in zip(DOC, seeded_ids):
        observed = by_id.get(bid)
        assert observed is not None, f"seeded block {bid} not found in tree"
        obs_kind, obs_text = observed
        assert obs_kind == expected_kind, (
            f"kind mismatch for {bid}: expected {expected_kind!r}, got {obs_kind!r}"
        )
        assert obs_text == expected_text, (
            f"text mismatch for {bid}: expected {expected_text!r}, got {obs_text!r}"
        )

    print("[4/5] Toggle each todo's done state...")
    todo_ids = [bid for (kind, _), bid in zip(DOC, seeded_ids) if kind == "todo"]
    assert todo_ids, "expected at least one todo in DOC"

    def idx_of(target_id):
        count = alice.eval("return get_block_count()")
        for i in range(1, count + 1):
            if alice.eval(f"return get_block_id({i})") == target_id:
                return i
        return None

    for tid in todo_ids:
        ti = idx_of(tid)
        assert ti is not None
        alice.eval(f"set_block_done({ti}, true)")
        assert alice.eval(f"return get_block_done({ti})") is True

    print("[5/5] Reorder: move the quote above the first list item...")
    quote_id = next(bid for (kind, _), bid in zip(DOC, seeded_ids) if kind == "quote")
    first_list_id = next(bid for (kind, _), bid in zip(DOC, seeded_ids) if kind == "list")
    first_list_idx = idx_of(first_list_id)
    alice.eval(f"move_block({idx_of(quote_id)}, {first_list_idx})")
    # After move, the quote should sit at first_list_idx (or first_list_idx - 1
    # depending on whether move is interpreted as "insert before" vs "swap"
    # — accept either, just assert the quote precedes the first list now).
    count_after = alice.eval("return get_block_count()")
    quote_pos = idx_of(quote_id)
    list_pos = idx_of(first_list_id)
    assert quote_pos is not None and list_pos is not None
    assert quote_pos < list_pos, (
        f"expected quote ({quote_pos}) above first list ({list_pos}) after move"
    )

    print(f"\n{'=' * 60}")
    print(f"  [SUCCESS] text-editor-egui smoke passed ({actual_count} blocks)")
    print(f"{'=' * 60}")
