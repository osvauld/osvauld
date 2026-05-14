#!/usr/bin/env python3
"""
Drag-list-egui demo runner — boots one peer through the full osvauld stack
(sthalam shell + kunki node + scribe upload + page open) and hands control
back to the user to interact with the egui window.

Not a real assertion test yet: the UI-automation path is Slint-specific, so
we can't dispatch synthetic pointer events into the egui window from
outside the process. Phase 6 of `docs/plans/renderer_egui.md` is the right
place for that. For now this script is "publish the app + open the page +
keep the session alive so I can click around."

Prerequisite: `cargo build -p sthalam --features egui` (the dispatch arm
that routes manifest.renderer == "egui" to renderer_egui is feature-gated).

Usage:
    python e2e_tests/test_drag_list_egui.py --keep
    python e2e_tests/test_drag_list_egui.py --keep --debug
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

APP_PATH = Path(__file__).parent.parent / "sample_apps" / "drag-list-egui"

args = AppTestScenario.parse_args("Drag List (egui) demo runner")

with AppTestScenario(
    name="drag_list_egui",
    app_path=str(APP_PATH),
    peers={
        "alice": {"role": "owner", "app": "Drag List (egui)"},
    },
    **args,
) as s:
    alice = s.peer("alice")

    print()
    print("=" * 60)
    print("  drag-list-egui is up.")
    print()
    print("  • The egui window should be visible (alice's instance).")
    print("  • Click + drag any card to reorder.")
    print("  • egui paints the ghost at the cursor for free.")
    print()
    print("  Press Ctrl+C to tear down the session.")
    print("=" * 60)
    print()

    # AppTestScenario consumes `--keep` from parse_args; when set, the
    # context manager blocks until Ctrl+C. Nothing else to do here.
