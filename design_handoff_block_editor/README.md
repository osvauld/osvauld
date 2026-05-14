# Handoff: Block-based Collaborative Editor — Interaction Layer

## Overview

Interaction-design spec for a Notion-style block editor inside a desktop app. The data layer (CRDT merge per block + tree-level CRDT for create/move/delete) and a basic editor are already implemented in **Slint**. This handoff covers the **interactivity layer** the developer will now build: gutter affordances, drag-to-reorder, block menu, + button, slash command, multi-select, empty / focus states, nesting visualization, and reorder animations.

## About the Design Files

The files in this bundle are **design references created in HTML** — frozen mockups of every interaction state, plus annotated foundation diagrams and spec tables. They are **not production code to copy directly**.

Target environment: **Slint** (no HTML/CSS). The implementation should use rectangles, layouts, and `TouchArea`s; custom paint is available but should be avoided in favor of compositional designs. Translate every visual into Slint primitives + `Palette.*` tokens.

Each token in the HTML mockup has a comment in `sthalam-theme.css` showing its corresponding `Palette.*` field; the translation is mechanical.

## Fidelity

**High-fidelity.** Final colors, typography, spacing, motion curves, and geometry. All numbers are canonical and intended to be plugged directly into Slint code. Two exceptions:

- The mockups render in HTML with browser font metrics. Slint's text shaper (parley not yet wired) will render plain strings only — no inline bold/italic. Design assumes single-string content per block.
- Drop-shadows / `filter: drop-shadow` on the drag-ghost preview are an HTML-ism. Implement as a soft `box-shadow`-equivalent rectangle behind the floating card.

## Screens / States

The spec is laid out as a single design canvas (`Block Editor Spec.html`) with four sections. Below is what each artboard depicts and what to build.

### Foundations (the geometry every state inherits)

#### F1 · Gutter anatomy
- **Gutter width**: 32 px. Lives outside the text column.
- **Gutter contains** (left to right): `+` button (~14 px visual), 4 px gap, `⋮⋮` 6-dot grip (~14 px visual). Each has a 20 × 20 hit-target.
- **Gutter visibility**: visible when (a) pointer is over the row, OR (b) the block contains the caret.
- **Fade**: 80 ms ease-out. Only opacity fades — content layout never reflows when the gutter shows/hides.

#### F2 · Drop-zone geometry
Each potential drop-target row is split into bands:

```
┌─────────────────────────────────────┐
│  ↑ above (top 33%)                  │
├─────────────────────────────────────┤  ← drop-line lands here
│  ↳ INTO as child (mid 34%)          │
├─────────────────────────────────────┤  ← drop-line lands here
│  ↓ below (bottom 33%)               │
└─────────────────────────────────────┘
```

- The middle "INTO" band only exists when `target.accepts_children == true`. For paragraphs / headings, the row is split 50/50 above/below.
- Even on a nestable target, INTO only commits if `cursor.x >= target.content_x + 16` (the user has to push the cursor inward).

Decision pseudocode:
```rust
fn drop_intent(target, cursor) -> Intent {
    let f = (cursor.y - target.top) / target.height;       // 0..1
    let pushed_in = cursor.x >= target.content_x + 16;     // INTO threshold

    match (target.accepts_children, pushed_in, f) {
        (true,  true,  0.33..=0.67) => Into(target),       // becomes child
        (_,     _,     ..0.50)      => Above(target),      // sibling above
        (_,     _,     _)           => Below(target),      // sibling below
    }
}
```

Currently nestable kinds: `list_item` only. (Headings could nest in the future; treat as paragraph for pass 1.)

#### F3 · Nesting / indent
- 24 px left padding per nesting level.
- 1.5 px hairline indent guide per level, color `rgba(20,21,28,0.07)` light / `rgba(255,255,255,0.10)` dark.
- Active branch (the branch containing the caret) tints its deepest guide to `accent.soft` (#A9A6F0).
- Bullet style cycles per depth: ● → ○ → ■ → repeat.

### 11 Interaction states

Each state has its own artboard with caption + summary + inline annotations. Implementor should refer to the HTML for visual confirmation.

| # | State | Trigger | Visual |
|---|---|---|---|
| 01 | Idle / focused | Caret in a block, no hover | Caret is the only marker; no border, no shading. |
| 02 | Hover | Pointer over a block-row | Row tints `rgba(20,21,28,0.035)`; gutter fades in 80 ms. |
| 03 | Empty (focused) | Block is empty AND has caret | Placeholder `Type '/' for commands` in `fg.faint`. Disappears the instant a key is pressed. Empty + unfocused blocks render no placeholder. |
| 04 | + button → type picker | Click-and-hold > 180 ms on `+`, OR shift-click `+` | Popover anchored 4 px below `+`. Plain click on `+` inserts an empty paragraph below and focuses it (zero-friction default). |
| 05 | Slash command | Typing `/` at offset 0 of an empty block | Wider palette (320 px) anchored to caret; with search header, grouped sections, footer with `↑↓` `↵` `esc` hints. The `/` is consumed, never stored as text. |
| 06 | Drag pickup | Mousedown on `⋮⋮` + 4 px movement | Source block drops to opacity 0.25 (preserves row height — no layout jump). Floating preview = white card, 1.5° rotation, 14 px shadow, follows cursor. |
| 07 | Drop indicator — sibling | Drag over top/bottom band of any row | 3 px accent line spans the text column (not the gutter), 4 px halo, 6 px end-cap dot. Indent matches prospective sibling. |
| 08 | Drop indicator — INTO | Drag over middle band of a nestable target with cursor pushed in | Parent row outlined in 1.5 px accent, tinted `rgba(138,134,229,0.10)`. Drop appends as child below existing children — no auto-collapse. |
| 09 | Block menu | Click `⋮⋮` | Popover anchored 6 px below the grip; flips above when room is short. Three groups: BLOCK actions, TURN INTO (kind switch), COLOR. First item is keyboard-highlighted on open. |
| 10 | Multi-select + bulk bar | Shift-click another block, drag in gutter, or `⌘A` repeat | Selected rows tint `rgba(138,134,229,0.13)` (no border). Bulk action bar floats bottom-center while ≥1 block is selected — Delete / Duplicate / Turn into / Move to. |
| 11 | Post-reorder | After a drop commits | Peer cards slide 220 ms `cubic-bezier(0.2, 0.7, 0.3, 1)` to final positions. Just-moved block flashes 0% → 8% accent → 0% over 600 ms. |

### Specs (printable tables)

#### Keyboard shortcuts — see `Block Editor Spec.html` artboard `kbd` for full table

Every block-level operation is reachable from the keyboard. Mouse-only operations (drag, marquee) all have keyboard substitutes.

**Editing**
- `↵` — split block at caret. New empty paragraph below; caret moves into it.
- `⇧↵` — soft line break inside the same block (no split).
- `⌫` at offset 0 of empty block — delete block, caret to end of previous block.
- `⌫` at offset 0 of non-empty block — merge into previous block.
- `Tab` (list_item only) — indent (becomes child of previous sibling).
- `⇧Tab` (list_item only) — outdent.

**Navigation**
- `↑ / ↓` — move caret across blocks; no-op at doc edges.
- `⌘↑ / ⌘↓` — caret to doc start / doc end.
- `⌘A` — first press: select all text in current block. Second: select the block. Third: select all blocks.

**Block actions**
- `⌘D` — duplicate focused block (or all selected blocks).
- `⌘⇧↑ / ⌘⇧↓` — move focused block up / down (preserves indent).
- `⌘⌥0` — turn into Paragraph.
- `⌘⌥1` — turn into Heading 1.
- `⌘⌥2` — turn into Heading 2.
- `⌘⌥3` — turn into List item.
- `⌘⌥4` — turn into Code.

**Menus**
- `/` at offset 0 of empty block — open slash command. Filter as you type.
- `esc` — dismiss the topmost open menu/popover. With nothing open: clear multi-selection.
- `↑/↓` inside menu — move highlight.
- `↵` inside menu — apply highlighted item.

**Drag substitutes (keyboard-only ops)**
- `⌘⇧D` — open block menu for focused block (= clicking `⋮⋮`).
- `Tab` inside block menu — cycle to TURN INTO submenu.

## Design Tokens

Light values listed first; dark theme values in italics where they differ. Each maps to a `Palette.*` field in `sthalam-theme.css`.

### Color
| Token | Light / Dark | Usage |
|---|---|---|
| `ed.page.bg` | `#FFFFFF` / `#0D0E13` | Document surface |
| `ed.fg` | `#1B1C22` / `#F5F5F7` | Primary text |
| `ed.fg.secondary` | `#52546B` / `#B6B7C3` | Menu hint text, breadcrumb |
| `ed.fg.muted` | `#82849A` / `#7F8192` | Placeholders, mono labels |
| `ed.fg.faint` | `#B8BAC8` / `#4D4E5C` | Inactive gutter icons, indent guides |
| `ed.bg.hover` | `rgba(20,21,28,0.035)` | Block-row hover tint |
| `ed.bg.select` | `rgba(138,134,229,0.13)` | Multi-selected blocks |
| `ed.bd.hairline` | `rgba(20,21,28,0.06)` | Hairlines, dividers |
| `ed.accent` | `#8A86E5` | Drop indicator, slash header, swatch select |
| `ed.accent.soft` | `#A9A6F0` | Active-branch indent guide |
| `ed.drop.line` | `#8A86E5` | Sibling-drop horizontal pin |
| `ed.drop.into.bd` | `#8A86E5` (1.5 px) | Drop-as-child outline |
| `ed.code.bg` | `rgba(20,21,28,0.045)` | Code block background |

### Spacing
| Token | Value | Note |
|---|---|---|
| `doc.leftPad` | 56 px | Page → text column |
| `doc.rightPad` | 56 px | Symmetric |
| `doc.topPad` | 32 px | Before doc title |
| `gutter.width` | 32 px | `+` and `⋮⋮` side-by-side |
| `gutter.gap` | 4 px | Gutter → text column |
| `icon.visual` | 14 × 14 | Glyph itself |
| `icon.hit` | 20 × 20 | Hit-target |
| `indent.step` | 24 px | Per nesting level |
| `guide.width` | 1.5 px | Indent hairline |
| `row.padY` | 4 px | ¶ / li vertical |
| `row.padY.h1` | 16 px | Heading 1 |
| `row.padY.h2` | 12 px | Heading 2 |
| `radius.row` | 4 px | Block hover/select rounding |
| `radius.menu` | 8 px | Popovers |
| `radius.slash` | 10 px | Slash palette |

### Motion
| Token | Curve | Where |
|---|---|---|
| `hover.fade` | 80 ms ease-out | Gutter affordance fade |
| `drop.line.show` | 60 ms (snap on; never animate position) | Drop-line appearance |
| `reorder.settle` | 220 ms `cubic-bezier(0.2, 0.7, 0.3, 1)` | Peer cards slide to final slot |
| `flash.confirm` | 600 ms 0% → 8% accent → 0% | Just-moved block confirmation |
| `menu.open` | 120 ms ease-out + 4 px y-shift | Fade + slight slide |
| `select.tint` | 100 ms | Multi-select bg fade-in |

## Non-goals (intentional omissions — do not re-litigate)

1. **Inline rich text (bold/italic)** — out of scope per brief; parley not wired.
2. **Hover-color preview before commit** — Notion previews on hover; we commit on click only. Avoids extra CRDT-shared state.
3. **Drag the + button to reorder** — `+` always inserts; `⋮⋮` always reorders. One affordance per motion.
4. **Auto-collapse parents on drop-INTO** — too surprising; child appends visibly.
5. **Block-level @-mentions** — same surface as awareness/cursors; deferred.
6. **Right-click context menu** — every action reachable via `⋮⋮`, slash, or shortcut. Third surface = sync burden.
7. **Marquee outside the gutter** — gutter drag = block selection; text-column drag = text selection. No mode switch.
8. **Animated INTO outline (pulse, breathe)** — static only. Animated drop-targets are a tell of toy editors.
9. **Drag handle floating over text on hover** — fights text selection. Left-margin gutter is boring and works.
10. **Slash command for non-block actions** (export, share) — kind insertion only; doc-level lives in surrounding shell.

## Files in this handoff

- `Block Editor Spec.html` — main design canvas. Open in a browser to see all states side-by-side, drag-reorder, focus-mode any artboard.
- `block-editor.jsx` — primitive components (`Block`, `Gutter`, `DropLine`, `BlockHandleMenu`, `PlusMenu`, `SlashMenu`, `BulkBar`, ...). Reading this is the fastest way to see exact computed styles per state.
- `scenes.jsx` — how each of the 11 interaction states composes the primitives.
- `specs.jsx` — keyboard / token / non-goals tables.
- `design-canvas.jsx` — pan/zoom canvas wrapper (presentation only; no design intent).
- `sthalam-theme.css` — token CSS with `/* Palette.* */` comments mapping each token to its Slint counterpart.

## Implementation pointers for Slint

- The doc body is a `VerticalBox` (or `Flickable` for scroll) whose children are block-rows.
- Each block-row is a `HorizontalBox`: `[gutter] [content]`. Gutter has fixed 32 px width, opacity bound to `row.is-hovered || block.has-caret`.
- Drop-line is an absolutely-positioned `Rectangle` sibling of the row, height 3, `background: Palette.accent`, only rendered when `drop_intent` matches that row.
- Drop-INTO outline is a 1.5 px `Rectangle` border laid over the target row.
- Multi-select uses a `selection: [int]` model on the parent; bg color of each row is bound to `selection.contains(row.index) ? Palette.bg-select : ...`.
- Reorder animation: animate `y` (or transform) with `220ms cubic-bezier(0.2, 0.7, 0.3, 1)` only on cards whose slot changed.

## Assets

No raster assets. All glyphs (grip, plus, icons) are inline SVG paths in `block-editor.jsx` — translate to Slint `Path` elements or `@image-url` on a packed icon sheet, depending on existing project convention.
