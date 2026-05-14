# Plans

Living planning docs that track in-flight or upcoming work. Each plan is
a single markdown file in this directory; this README is the index.

A plan lives here for two reasons:

1. **Continuity across sessions.** Work spans multiple Claude sessions
   and the conversation summary alone isn't enough. A written plan
   survives compaction, branch switches, and human breaks.
2. **Cross-cutting context.** When work touches several crates / tests /
   sample apps, the plan is the place future-you (or another agent)
   reads first to understand *why* the change spans where it spans.

What goes here:

- Multi-step engineering work (feature builds, refactors, migrations)
- Design directions still being validated against real apps
- Sequencing decisions ("do A before B because…")

What does **not** go here:

- One-shot bug fixes — those live in the commit message.
- Architecture stable enough to belong in `docs/ARCHITECTURE.md` or a
  similar reference doc — promote it out of `plans/` once it's stable.
- Per-PR scratchpads — use the PR description.

## Status legend

| Marker | Meaning |
|---|---|
| 🟢 active | Currently being worked on |
| 🟡 paused | Intentionally on hold; reason in the plan body |
| 🔵 next | Queued, not started |
| ⚫ done | Goal met; kept for context until promoted out |

## Index

| Status | Plan | Scope |
|---|---|---|
| 🟢 active | [widgets_roadmap.md](widgets_roadmap.md) | Block editor, canvas direction, interactivity primitives (Draggable / DropTarget / DragLayer), render layer (parley), kind schema generalisation. The umbrella for the editor + canvas frontier. |
| 🟢 active | [ui_automation.md](ui_automation.md) | From-scratch UI automation: drive the live winit-rendered Slint window from outside via control socket. Mouse dispatch, screenshot, GIF + per-frame state capture for animated-state assertions. Unblocks real drag-and-drop tests. |
| 🟢 active | [renderer_egui.md](renderer_egui.md) | Third renderer crate alongside slint/raylib. Phases 0–1 + drag bindings done; subprocess explored and reverted; phases G1–G5 (in-process via GLFW) landed 2026-05-13. |
| 🟡 paused | [app_ipc.md](app_ipc.md) | Capability-aware RPC + event channel between sthalam (daemon) and Lua app subprocesses. Parked after renderer_egui pivoted to in-process via GLFW (2026-05-13). Picked back up when the trust requirement forces it (running untrusted peer-shipped apps). |

## Conventions inside a plan file

Each plan should include, in roughly this order:

- **Status** — one of the markers above, plus a date.
- **Goal** — one paragraph. What "done" looks like.
- **Why now** — what motivated this work; what unblocks if we ship.
- **Design** — the chosen approach + the alternatives considered + why.
- **Sequencing** — phased steps with clear deliverables per phase.
- **Open questions** — things to resolve before / during.
- **Out of scope** — explicit non-goals.
- **Related** — links to other plans, design docs, sample apps it touches.

Keep the prose tight. A plan that nobody reads is worse than no plan.
