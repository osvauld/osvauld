#!/usr/bin/env python3
"""
RichText Hello E2E Test — smoke test for @osvauld/widgets RichTextEdit.

Validates milestone 1 of the widgets crate end-to-end:
- Sample app imports `RichTextEdit` from `@osvauld/widgets`
- Slint compilation resolves the library path
- Widget renders, text round-trips through the doc LoroMap layer
- Edit on owner syncs to viewer

Usage:
    python e2e_tests/test_richtext_hello.py
    python e2e_tests/test_richtext_hello.py --keep
    python e2e_tests/test_richtext_hello.py --debug
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "richtext-hello"

args = AppTestScenario.parse_args("RichText Hello E2E Test")

with AppTestScenario(
    name="richtext_hello_test",
    app_path=str(APP_PATH),
    peers={
        "alice": {"role": "owner", "app": "RichText Hello"},
        "bob": {"role": "viewer", "app": "RichText Hello"},
    },
    **args,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")

    # Sanity check — fresh doc starts empty.
    print("[1/4] Verifying empty initial state...")
    assert alice.eval("return get_body()") == "", "alice doc not empty on init"
    assert bob.eval("return get_body()") == "", "bob doc not empty on init"

    # Alice writes through the app's save path (same code on_field_changed uses).
    print("[2/4] Alice writes 'hello world' to doc.body...")
    alice.eval('set_body("hello world")')
    alice.wait_for(
        lambda: alice.eval("return get_body()") == "hello world",
        desc="alice local write",
    )

    # Wait for sync to bob — verify both the layer and the bound Slint property.
    print("[3/4] Waiting for sync to bob (layer + bound property)...")
    bob.wait_for(
        lambda: bob.eval("return get_body()") == "hello world",
        desc="bob doc sync",
    )
    bob.wait_for(
        lambda: bob.eval("return get_doc_text_property()") == "hello world",
        desc="bob bind_text reactive update",
    )

    # Alice overwrites with a longer string — exercise CRDT update path
    # and verify reactive bind picks up the change without manual refresh.
    print("[4/4] Alice overwrites with longer text...")
    alice.eval('set_body("the quick brown fox jumps over the lazy dog")')
    bob.wait_for(
        lambda: bob.eval("return get_body()")
        == "the quick brown fox jumps over the lazy dog",
        desc="bob doc resync",
    )
    bob.wait_for(
        lambda: bob.eval("return get_doc_text_property()")
        == "the quick brown fox jumps over the lazy dog",
        desc="bob bind_text reactive resync",
    )

    print(f"\n{'=' * 60}")
    print("  [SUCCESS] RichText Hello E2E Test Passed!")
    print(f"{'=' * 60}")
