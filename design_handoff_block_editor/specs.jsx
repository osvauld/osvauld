// specs.jsx — printable spec sheets: keyboard, tokens, non-goals.
(function () {
  const { Stage, Kbd, ED, ED_FONT, GEO } = window;

  // Shared sheet shell — large legible spec card on a doc surface.
  function Sheet({ id, title, summary, children }) {
    return (
      <Stage id={id} title={title} summary={summary}
        breadcrumb={<><span>spec sheet</span><span style={{flex:1}}/><span>plug into Slint</span></>}>
        <div style={{ height: 16 }} />
        {children}
      </Stage>
    );
  }

  function H({ children }) {
    return (
      <div style={{
        fontFamily: ED_FONT.mono, fontSize: 10.5, fontWeight: 600,
        color: ED.fgMuted, letterSpacing: '0.1em',
        marginTop: 22, marginBottom: 8,
      }}>{children}</div>
    );
  }

  function Row({ cols, head = false, alt = false }) {
    return (
      <div style={{
        display: 'grid', gridTemplateColumns: cols.template || '170px 200px 1fr',
        gap: 14, padding: '7px 12px',
        background: head ? 'transparent' : (alt ? '#FAFAFB' : 'transparent'),
        borderRadius: 4,
        borderBottom: head ? '1px solid ' + ED.border : 'none',
        fontFamily: ED_FONT.ui,
        fontSize: head ? 10.5 : 12.5,
        fontWeight: head ? 600 : 400,
        color: head ? ED.fgMuted : ED.fg,
        letterSpacing: head ? '0.1em' : 0,
        textTransform: head ? 'uppercase' : 'none',
        lineHeight: 1.45,
      }}>
        {cols.values.map((v, i) => (
          <div key={i} style={{
            fontFamily: i === 0 && !head ? ED_FONT.mono : 'inherit',
            color: i === 0 && !head ? ED.accent : 'inherit',
            fontSize: i === 0 && !head ? 12 : 'inherit',
            wordBreak: 'break-word',
          }}>{v}</div>
        ))}
      </div>
    );
  }

  // ── Keyboard shortcuts ─────────────────────────────────────────
  function Keyboard() {
    const tpl = '170px 1fr';
    const sections = [
      ['EDITING', [
        [<><Kbd>↵</Kbd></>,                                'Split block at caret. New empty paragraph below; caret moves into it.'],
        [<><Kbd>⇧</Kbd>+<Kbd>↵</Kbd></>,                   'Soft line break inside the same block (no split).'],
        [<><Kbd>⌫</Kbd></>,                                'At offset 0 of an empty block: delete block, move caret to end of previous block.'],
        [<><Kbd>⌫</Kbd></>,                                'At offset 0 of a non-empty block: merge into previous block.'],
        [<><Kbd>Tab</Kbd></>,                              'list_item only — indent (becomes child of previous sibling).'],
        [<><Kbd>⇧</Kbd>+<Kbd>Tab</Kbd></>,                 'list_item only — outdent.'],
      ]],
      ['NAVIGATION', [
        [<><Kbd>↑</Kbd> / <Kbd>↓</Kbd></>,                 'Move caret across blocks. At top/bottom of the doc, no-op.'],
        [<><Kbd>⌘</Kbd>+<Kbd>↑</Kbd> / <Kbd>⌘</Kbd>+<Kbd>↓</Kbd></>, 'Caret to doc start / doc end.'],
        [<><Kbd>⌘</Kbd>+<Kbd>A</Kbd></>,                   'First press: select all text in current block. Second press: select the block. Third press: select all blocks.'],
      ]],
      ['BLOCK ACTIONS', [
        [<><Kbd>⌘</Kbd>+<Kbd>D</Kbd></>,                   'Duplicate focused block (or all selected blocks).'],
        [<><Kbd>⌘</Kbd>+<Kbd>⇧</Kbd>+<Kbd>↑</Kbd> / <Kbd>↓</Kbd></>, 'Move focused block up / down (preserves indent).'],
        [<><Kbd>⌘</Kbd>+<Kbd>⌥</Kbd>+<Kbd>0</Kbd></>,     'Turn into Paragraph.'],
        [<><Kbd>⌘</Kbd>+<Kbd>⌥</Kbd>+<Kbd>1</Kbd></>,     'Turn into Heading 1.'],
        [<><Kbd>⌘</Kbd>+<Kbd>⌥</Kbd>+<Kbd>2</Kbd></>,     'Turn into Heading 2.'],
        [<><Kbd>⌘</Kbd>+<Kbd>⌥</Kbd>+<Kbd>3</Kbd></>,     'Turn into List item.'],
        [<><Kbd>⌘</Kbd>+<Kbd>⌥</Kbd>+<Kbd>4</Kbd></>,     'Turn into Code.'],
      ]],
      ['MENUS', [
        [<><Kbd>/</Kbd></>,                                'At offset 0 of an empty block — open slash command. Filter as you type.'],
        [<><Kbd>esc</Kbd></>,                              'Dismiss the topmost open menu / popover. With nothing open: clear multi-selection.'],
        [<><Kbd>↑</Kbd> / <Kbd>↓</Kbd></>,                 'Inside any menu — move highlight.'],
        [<><Kbd>↵</Kbd></>,                                'Inside any menu — apply highlighted item.'],
      ]],
      ['DRAG SUBSTITUTES (keyboard-only ops)', [
        [<><Kbd>⌘</Kbd>+<Kbd>⇧</Kbd>+<Kbd>D</Kbd></>,     'Open block menu for focused block (= clicking ⋮⋮).'],
        [<><Kbd>Tab</Kbd> in block menu</>,                'Cycle to TURN INTO submenu.'],
      ]],
    ];

    return (
      <Sheet id="SPEC · KEYBOARD" title="Keyboard shortcuts"
        summary="Every block-level operation is reachable from the keyboard. Mouse-only operations (drag, marquee) all have keyboard substitutes.">
        {sections.map(([title, rows], i) => (
          <div key={i}>
            <H>{title}</H>
            {rows.map(([k, v], j) => (
              <Row key={j} alt={j % 2 === 0} cols={{ template: tpl, values: [k, v] }} />
            ))}
          </div>
        ))}
      </Sheet>
    );
  }

  // ── Color / spacing tokens ─────────────────────────────────────
  function Tokens() {
    const tpl = '180px 220px 1fr';
    const Swatch = ({ c, dark }) => (
      <span style={{
        display: 'inline-block', width: 14, height: 14, borderRadius: 3,
        background: c, border: '1px solid ' + (dark ? 'rgba(255,255,255,0.2)' : ED.border),
        verticalAlign: 'middle', marginRight: 8,
      }} />
    );
    const colors = [
      ['ed.page.bg',          '#FFFFFF / #0D0E13', 'document surface · light / dark', '#FFFFFF'],
      ['ed.fg',               '#1B1C22 / #F5F5F7', 'primary text', '#1B1C22'],
      ['ed.fg.secondary',     '#52546B / #B6B7C3', 'menu hint text, breadcrumb', '#52546B'],
      ['ed.fg.muted',         '#82849A / #7F8192', 'placeholders, mono labels', '#82849A'],
      ['ed.fg.faint',         '#B8BAC8 / #4D4E5C', 'inactive gutter icons, indent guide', '#B8BAC8'],
      ['ed.bg.hover',         'rgba(20,21,28,0.035)', 'block-row hover', '#EFEFF1'],
      ['ed.bg.select',        'rgba(138,134,229,0.13)', 'multi-selected blocks (Sthalam accent)', '#E3E1FA'],
      ['ed.bd.hairline',      'rgba(20,21,28,0.06)', 'all hairlines, dividers', '#EDEDF0'],
      ['ed.accent',           '#8A86E5', 'drop indicator · selected swatch · slash header', '#8A86E5'],
      ['ed.accent.soft',      '#A9A6F0', 'active-branch indent guide', '#A9A6F0'],
      ['ed.drop.line',        '#8A86E5', 'sibling-drop horizontal pin', '#8A86E5'],
      ['ed.drop.into.bd',     '#8A86E5 (1.5 px)', 'drop-as-child outline', '#8A86E5'],
      ['ed.code.bg',          'rgba(20,21,28,0.045)', 'code block background', '#F1F1F3'],
    ];
    const sp = [
      ['doc.leftPad',  '56 px',  'page → text column'],
      ['doc.rightPad', '56 px',  'symmetric'],
      ['doc.topPad',   '32 px',  'before doc title'],
      ['gutter.width', '32 px',  '+ + ⋮⋮ side-by-side'],
      ['gutter.gap',   '4 px',   'gutter → text column'],
      ['icon.visual',  '14 × 14','glyph itself'],
      ['icon.hit',     '20 × 20','hover hit-target'],
      ['indent.step',  '24 px',  'per nesting level'],
      ['guide.width',  '1.5 px', 'indent hairline'],
      ['row.padY',     '4 px',   '¶ / li vertical'],
      ['row.padY.h1',  '16 px',  'heading 1'],
      ['row.padY.h2',  '12 px',  'heading 2'],
      ['radius.row',   '4 px',   'block hover/select rounding'],
      ['radius.menu',  '8 px',   'popovers'],
      ['radius.slash', '10 px',  'slash palette'],
    ];
    const motion = [
      ['hover.fade',     '80 ms ease-out',                'gutter affordance fade'],
      ['drop.line.show', '60 ms',                         'snap on; never animate position'],
      ['reorder.settle', '220 ms cubic-bezier(.2,.7,.3,1)', 'peer cards slide to final slot'],
      ['flash.confirm',  '600 ms 0% → 8% accent → 0%',    'just-moved block confirmation'],
      ['menu.open',      '120 ms ease-out · 4 px y-shift', 'fade + slight slide'],
      ['select.tint',    '100 ms',                        'multi-select bg fade in'],
    ];

    return (
      <Sheet id="SPEC · TOKENS" title="Color, spacing &amp; motion tokens"
        summary="Light values first, dark in italics where they differ. Plug straight into Slint Palette.* and animation easings.">
        <H>COLOR</H>
        <Row head cols={{ template: tpl, values: ['NAME', 'VALUE', 'USAGE'] }} />
        {colors.map(([k, v, n, swc], i) => (
          <Row key={i} alt={i % 2 === 0} cols={{
            template: tpl,
            values: [k, <span><Swatch c={swc} />{v}</span>, n],
          }} />
        ))}
        <H>SPACING</H>
        <Row head cols={{ template: '180px 100px 1fr', values: ['NAME', 'VALUE', 'NOTE'] }} />
        {sp.map(([k, v, n], i) => (
          <Row key={i} alt={i % 2 === 0} cols={{ template: '180px 100px 1fr', values: [k, v, n] }} />
        ))}
        <H>MOTION</H>
        <Row head cols={{ template: '180px 240px 1fr', values: ['NAME', 'CURVE', 'WHERE'] }} />
        {motion.map(([k, v, n], i) => (
          <Row key={i} alt={i % 2 === 0} cols={{ template: '180px 240px 1fr', values: [k, v, n] }} />
        ))}
      </Sheet>
    );
  }

  // ── Non-goals ──────────────────────────────────────────────────
  function NonGoals() {
    const items = [
      ['Inline rich text (bold / italic) in this pass',
       'Out of scope per brief — parley not yet wired. Designing for it now would lock decisions on selection painting and toolbar shape that should follow text-shaping work.'],
      ['Hover-color preview before commit',
       'Notion previews the color on hover. Skipped: adds a layer of state to a CRDT-shared block kind for marginal value. Color commits on click only.'],
      ['Drag the + button to reorder',
       'Considered: dragging + would reorder *and* insert. Rejected: confuses two motions onto one affordance. + always inserts; ⋮⋮ always reorders.'],
      ['Auto-collapsible parents on drop-INTO',
       'When dropping a child into a list_item with many existing children, do NOT auto-collapse. The user will see the new child appended. Collapsing surprises them.'],
      ['Block-level mention / @-references',
       'Brief explicitly defers awareness / collaborative cursors. Same call applies to @-mentions of peers — that\u2019s the same UI surface, future pass.'],
      ['Right-click context menu',
       'Considered for parity. Rejected for pass 1 — every action is reachable via ⋮⋮ menu, slash menu, or shortcut. Adding right-click is a third surface to keep in sync; revisit once those two stabilise.'],
      ['Marquee outside the gutter',
       'Click-and-drag inside the gutter selects blocks. Click-and-drag inside the text column starts a text selection. Two distinct motions on two distinct regions — no mode switch.'],
      ['Animated INTO outline (pulse, breathe)',
       'Still outline only. Animated drop-targets are a tell of toy editors — Notion stays static and so should we.'],
      ['Drag handle on the right edge',
       'Considered as the "narrow gutter" answer (gutter = 0, handle floats over text on hover). Rejected: handle paints over content, hit-testing fights with text selection. Left-margin handle is boring and works.'],
      ['Slash command for non-block actions (export, share)',
       'Slash is for block-kind insertion only. Doc-level actions live in the surrounding shell — keeps the palette short and predictable.'],
    ];

    return (
      <Sheet id="SPEC · NON-GOALS" title="Non-goals — interactions we considered and rejected"
        summary="Each item below was discussed and intentionally cut. Reasoning included so we don\u2019t re-litigate in code review.">
        <div style={{ height: 8 }} />
        {items.map(([title, body], i) => (
          <div key={i} style={{
            display: 'grid', gridTemplateColumns: '24px 1fr',
            gap: 12, padding: '12px 0',
            borderBottom: i === items.length - 1 ? 'none' : '1px solid ' + ED.hairline,
          }}>
            <div style={{
              width: 22, height: 22, borderRadius: 11,
              background: ED.errBg, color: ED.err,
              fontFamily: ED_FONT.mono, fontSize: 11, fontWeight: 600,
              display: 'grid', placeItems: 'center',
              flexShrink: 0,
            }}>×</div>
            <div>
              <div style={{ fontSize: 13.5, fontWeight: 600, color: ED.fg, marginBottom: 4 }}>{title}</div>
              <div style={{ fontSize: 12.5, color: ED.fgSecondary, lineHeight: 1.5 }}>{body}</div>
            </div>
          </div>
        ))}
      </Sheet>
    );
  }

  window.Specs = { Keyboard, Tokens, NonGoals };
})();
