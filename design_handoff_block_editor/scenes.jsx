// scenes.jsx — the 11 interaction-state mockups composed from block-editor.jsx
// Each scene returns a complete artboard contents (caption + doc surface +
// frozen overlay state). All annotations are inline so a downloaded PNG of
// any single artboard is self-explanatory.

(function () {
  const { Stage, Block, GhostPreview, BlockHandleMenu, PlusMenu, SlashMenu,
          BulkBar, Marquee, Annotation, ED, ED_FONT, GEO } = window;

  // Reused doc body — lets every state share the same surrounding context.
  const DOC = [
    { kind: 'h1', text: 'Architecture notes' },
    { kind: 'p',  text: 'A two-peer CRDT editor backed by a tree of blocks. Char-level merges happen inside each block; tree-level merges (create / move / delete) compose at the document level.' },
    { kind: 'h2', text: 'Open questions' },
    { kind: 'li', text: 'How wide is the gutter when both affordances are visible?' },
    { kind: 'li', text: 'Do drop-into zones apply to non-list blocks at all?' },
    { kind: 'li', text: 'Should the block menu copy Notion verbatim or trim it?' },
    { kind: 'p',  text: 'For now we are choosing to ship narrow, opinionated, and Notion-shaped where it already works.' },
  ];

  function Body({ overrides = {}, dropAfter = null, dropChild = null, dark = false }) {
    return DOC.map((b, i) => {
      const o = overrides[i] || {};
      return (
        <Block key={i} kind={b.kind} text={b.text}
          gutter="hidden" {...o}
          listDepth={b.kind === 'li' ? 0 : undefined}
          dropLine={dropAfter === i ? 'below' : (o.dropLine || null)}
          dropChild={dropChild === i || o.dropChild}
        />
      );
    });
  }

  // ── 01 · Idle / focus ─────────────────────────────────────────
  function IdleFocus({ dark = false }) {
    return (
      <Stage id="01 · IDLE" dark={dark}
        title="Focus & active states"
        summary="No hover. The focused block has a caret only — no border, no shading. Active block is identified by caret position; nothing else changes vs. idle.">
        <Body overrides={{
          1: { caret: true },                  // caret blinks at end of paragraph
        }} dark={dark} />
        <div style={{ position: 'relative' }}>
          <Annotation
            from={{ x: 360, y: -180 }}
            to={{ x: 220, y: -190 }}
            label={'caret = the only marker\nof "this block is focused".'}
            side="right" />
        </div>
      </Stage>
    );
  }

  // ── 02 · Hover affordances ─────────────────────────────────────
  function Hover() {
    return (
      <Stage id="02 · HOVER"
        title="Hover affordances"
        summary="Pointer over any block reveals + and ⋮⋮ in the gutter and tints the row 3.5%. Cursor anywhere over the row counts — including the gutter itself.">
        <Body overrides={{
          3: { gutter: 'shown', bg: 'hover' },
        }} />
        <div style={{ position: 'relative' }}>
          <Annotation from={{ x: 280, y: -130 }} to={{ x: 60, y: -132 }}
            label={'+ click → insert ¶\n+ drag → type picker'} side="right" />
          <Annotation from={{ x: 280, y: -110 }} to={{ x: 84, y: -132 }}
            label={'⋮⋮ click → menu\n⋮⋮ drag → reorder'} side="right" />
          <Annotation from={{ x: 380, y: -88 }} to={{ x: 240, y: -130 }}
            label={'row tint: rgba(20,21,28,0.035)\nfade: 80 ms — never reflows text'}
            side="right" />
        </div>
      </Stage>
    );
  }

  // ── 03 · Empty (focused) ───────────────────────────────────────
  function Empty() {
    return (
      <Stage id="03 · EMPTY"
        title="Empty block — focused"
        summary={"Brand-new block, caret in it, no text yet. Placeholder \u201CType '/' for commands\u201D appears in fgFaint and disappears the moment a key is pressed. Unfocused empty blocks render no placeholder."}>
        <Block kind="h1" text="Architecture notes" />
        <Block kind="p" text="A two-peer CRDT editor backed by a tree of blocks." />
        <Block kind="p" text="" gutter="hidden"
          caret={true} placeholder={"Type '/' for commands"} />
        <Block kind="p" text="" gutter="hidden" />
        <div style={{ position: 'relative' }}>
          <Annotation from={{ x: 350, y: -78 }} to={{ x: 220, y: -78 }}
            label={'focused + empty\n→ show placeholder'} side="right" />
          <Annotation from={{ x: 350, y: -42 }} to={{ x: 220, y: -42 }}
            label={'empty + unfocused\n→ truly blank (no hint)'} side="right" />
        </div>
      </Stage>
    );
  }

  // ── 04 · + button menu ─────────────────────────────────────────
  function PlusOpen() {
    return (
      <Stage id="04 · INSERT"
        title="+ button → type picker"
        summary="Click the + inserts an empty paragraph below and focuses it (zero-friction default). Click-and-hold (>180 ms) or shift-click opens this picker. Same items the slash menu shows, anchored to the gutter.">
        <Body overrides={{ 3: { gutter: 'plusActive' } }} />
        {/* anchor the menu to the +'s rough on-screen position */}
        <div style={{ position: 'absolute', left: 86, top: 296 }}>
          <PlusMenu x={0} y={0} />
        </div>
        <div style={{ position: 'relative' }}>
          <Annotation from={{ x: 380, y: -10 }} to={{ x: 280, y: 4 }}
            label={'menu offsets 4 px down\nfrom the + button rect'} side="right" />
        </div>
      </Stage>
    );
  }

  // ── 05 · Slash command ─────────────────────────────────────────
  function SlashOpen() {
    return (
      <Stage id="05 · SLASH"
        title="Slash command palette"
        summary={"Typing / at offset 0 of an empty block opens this. Anchored to the caret (not the gutter). ↑/↓ navigate, ↵ commits, esc dismisses. The slash itself is consumed — replacing the block kind, never inserted as text."}>
        <Block kind="h1" text="Architecture notes" />
        <Block kind="p" text="A two-peer CRDT editor backed by a tree of blocks." />
        <Block kind="p" text="/" caret={true} />
        <div style={{ position: 'absolute', left: 76, top: 294 }}>
          <SlashMenu x={0} y={0} query="" />
        </div>
        <div style={{ position: 'relative' }}>
          <Annotation from={{ x: 420, y: -160 }} to={{ x: 230, y: -160 }}
            label={'palette anchors at caret —\nbelow when room, else above'} side="right" />
        </div>
      </Stage>
    );
  }

  // ── 06 · Drag pickup ───────────────────────────────────────────
  function DragPickup() {
    return (
      <Stage id="06 · DRAG"
        title="Block picked up — in flight"
        summary={"Mousedown on ⋮⋮ + 4 px movement starts the drag. Source block fades to 25% opacity (a "+'"'+"slot"+'"'+" — keeps neighbour spacing). A floating preview tracks the cursor at +1° rotation with a small drop-shadow."}>
        <Body overrides={{
          3: { ghost: true, gutter: 'gripActive' },
        }} />
        <GhostPreview kind="li"
          text="How wide is the gutter when both affordances are visible?"
          x={300} y={210} rot={-1.5} />
        <div style={{ position: 'relative' }}>
          <Annotation from={{ x: 60, y: -200 }} to={{ x: 80, y: -180 }}
            label={'source: opacity 0.25\nholds row height\n(no jump)'} side="right" />
          <Annotation from={{ x: 480, y: -100 }} to={{ x: 460, y: -120 }}
            label={'preview: white card\n+ 1.5° tilt, 14 px shadow.\ntracks cursor.'} side="left" />
        </div>
      </Stage>
    );
  }

  // ── 07 · Drop indicator — sibling ──────────────────────────────
  function DropSibling({ dark = false }) {
    return (
      <Stage id="07 · DROP · SIBLING" dark={dark}
        title="Drop indicator — drop above/below"
        summary="A 3 px accent line spans the text column (not the gutter). End-cap dot pins the start. Indent matches the prospective sibling's indent — drop on a level-1 list_item drops at level 1.">
        <Body overrides={{
          3: { ghost: true },
        }} dropAfter={4} dark={dark} />
        <GhostPreview kind="li"
          text="How wide is the gutter when both affordances are visible?"
          x={320} y={216} rot={-1.5} />
        <div style={{ position: 'relative' }}>
          <Annotation from={{ x: 380, y: -120 }} to={{ x: 240, y: -118 }}
            label={'3 px line · accent purple\n4 px halo · 6 px end-cap\nspans text column only'} side="right" />
        </div>
      </Stage>
    );
  }

  // ── 08 · Drop indicator — into child ──────────────────────────
  function DropInto() {
    return (
      <Stage id="08 · DROP · INTO"
        title="Drop indicator — drop AS child"
        summary="Cursor over middle band of a list_item AND pushed past content-start by ≥16 px → outline the parent in 1.5 px accent and tint at 10%. Drop commits a child below the parent's existing children.">
        <Body overrides={{
          3: { dropChild: true },
          5: { ghost: true },
        }} />
        <GhostPreview kind="li"
          text="Should the block menu copy Notion verbatim or trim it?"
          x={310} y={252} rot={-1.4} />
        <div style={{ position: 'relative' }}>
          <Annotation from={{ x: 400, y: -180 }} to={{ x: 240, y: -200 }}
            label={'parent outlined +\ntinted 10%. New child\nappends inside.'} side="right" />
        </div>
      </Stage>
    );
  }

  // ── 09 · Block handle menu ─────────────────────────────────────
  function HandleMenu() {
    return (
      <Stage id="09 · BLOCK MENU"
        title="Block menu — opened from ⋮⋮"
        summary="Click the grip → popover. Three groups: BLOCK actions, TURN INTO (kind switch), COLOR. Anchored 6 px below the grip; flips above when room is short. Esc dismisses; selecting an item dismisses + applies.">
        <Body overrides={{
          3: { gutter: 'gripActive', bg: 'hover' },
        }} />
        <div style={{ position: 'absolute', left: 84, top: 226 }}>
          <BlockHandleMenu x={0} y={0} />
        </div>
        <div style={{ position: 'relative' }}>
          <Annotation from={{ x: 460, y: -290 }} to={{ x: 320, y: -296 }}
            label={'first item highlighted\nfor keyboard arrival.\n↑/↓ navigate, ↵ apply.'} side="right" />
        </div>
      </Stage>
    );
  }

  // ── 10 · Multi-select + bulk bar ──────────────────────────────
  function MultiSelect() {
    return (
      <Stage id="10 · MULTI-SELECT"
        title="Multi-select + bulk action bar"
        summary="Three ways in: shift-click another block, drag in the gutter, ⌘A inside a block selects it then ⌘A again selects all. Selected blocks tint with accent at 13%. Bulk bar floats bottom-center while ≥1 is selected.">
        <Body overrides={{
          3: { bg: 'select' },
          4: { bg: 'select' },
          5: { bg: 'select' },
        }} />
        <BulkBar count={3} />
        <div style={{ position: 'relative' }}>
          <Annotation from={{ x: 380, y: -250 }} to={{ x: 220, y: -250 }}
            label={'bg: rgba(138,134,229,0.13)\nno border — Notion-shaped'} side="right" />
        </div>
      </Stage>
    );
  }

  // ── 11 · Post-reorder · nested ────────────────────────────────
  function PostReorder() {
    return (
      <Stage id="11 · NESTED"
        title="Post-reorder, with children"
        summary={"After drop, peers slide 220 ms cubic-bezier(0.2, 0.7, 0.3, 1) into their final positions. Indent guides re-draw in the same frame. The just-moved block flashes its row tint at 8% for 600 ms then fades."}>
        <Block kind="h1" text="Architecture notes" />
        <Block kind="h2" text="Open questions" />
        <Block kind="li" text="Drop-into zones — applies to nestable kinds only." indent={0} listDepth={0} />
        <Block kind="li" text="Should the block menu copy Notion verbatim or trim it?"
          indent={1} listDepth={1}
          bg="select" indentGuide="live" />
        <Block kind="li" text="Are 32 px gutters wide enough at 12 px font?" indent={1} listDepth={1} indentGuide="live" />
        <Block kind="li" text="How wide is the gutter overall?" indent={0} listDepth={0} />
        <Block kind="p" text="For now we are choosing to ship narrow, opinionated, and Notion-shaped where it already works." />

        <div style={{ position: 'relative' }}>
          <Annotation from={{ x: 410, y: -210 }} to={{ x: 260, y: -210 }}
            label={'just-moved block\nflashes 8% accent\nfor 600 ms'} side="right" />
          <Annotation from={{ x: 410, y: -160 }} to={{ x: 80, y: -180 }}
            label={'active branch guide\ntints accent.soft while\ncaret is inside it'} side="right" />
        </div>
      </Stage>
    );
  }

  window.Scenes = {
    IdleFocus, Hover, Empty, PlusOpen, SlashOpen,
    DragPickup, DropSibling, DropInto, HandleMenu, MultiSelect, PostReorder,
  };
})();
