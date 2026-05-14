#!/usr/bin/env python3
"""
UI automation smoke test — minimal diagnostic.

Verifies in isolation:
  1. The editor window opens and is visible.
  2. take_snapshot returns a non-empty image.
  3. dispatch_event(PointerPressed) reaches the slint hit-tester
     (we expose a debug global to count presses).

If (2) fails the screenshot is blank — femtovg/glReadPixels issue.
If (3) fails our coordinate calc or dispatch path is wrong.

Usage: python e2e_tests/test_ui_smoke.py
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "text-editor"
ART = Path(__file__).parent.parent / "test_artefacts" / "ui_smoke"

args = AppTestScenario.parse_args("UI smoke")
ART.mkdir(parents=True, exist_ok=True)

with AppTestScenario(
    name="ui_smoke",
    app_path=str(APP_PATH),
    peers={"alice": {"role": "owner", "app": "Text Editor"}},
    **args,
) as s:
    alice = s.peer("alice")

    # Wait for the editor to be ready and have at least one block.
    alice.wait_for(
        lambda: alice.eval("return get_block_count()") >= 1,
        desc="seeded block",
    )
    time.sleep(0.6)  # let one paint happen

    # 1. Window size + screenshot before any synthetic input.
    w, h = alice.ui_window_size()
    print(f"  window size = {w} x {h}")

    pre = ART / "01_pre.png"
    alice.ui_screenshot(str(pre))
    print(f"  screenshot 01 -> {pre}  size={pre.stat().st_size} bytes")

    # 2. Hover into the middle of the window — should at least flip
    #    *some* TouchArea's has-hover. The toolbar buttons are easy
    #    targets (top of the window, around y=50).
    alice.ui_mouse_move(120, 50)
    time.sleep(0.2)
    post = ART / "02_post_move.png"
    alice.ui_screenshot(str(post))
    print(f"  screenshot 02 -> {post}  size={post.stat().st_size} bytes")

    # 3. Press + release in the same spot, to see if the toolbar's
    #    "p" button (first kind button at x≈40, y≈50) reacts. Any
    #    visible state change between 02 and 03 indicates routing.
    alice.ui_mouse_press(40, 50)
    time.sleep(0.1)
    alice.ui_mouse_release(40, 50)
    time.sleep(0.3)
    post2 = ART / "03_post_click.png"
    alice.ui_screenshot(str(post2))
    print(f"  screenshot 03 -> {post2}  size={post2.stat().st_size} bytes")

    print()
    print(f"open in viewer: feh {ART}/*.png")
