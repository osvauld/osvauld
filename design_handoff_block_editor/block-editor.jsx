// block-editor.jsx
// Static primitives for the Notion-style block editor interaction spec.
// Each component is a frozen visual state — no real interactivity. They
// compose into the artboards in "Block Editor Spec.html".
//
// Light surface is primary (per brief). Accent + mono pulled from the
// Sthalam design system; everything else is editor-local so the doc
// surface reads as a document, not as Sthalam shell chrome.

// ─── Tokens ────────────────────────────────────────────────────────
const ED = {
  // Surfaces
  pageBg:           '#FFFFFF',
  panelBg:          '#FAFAFA',
  // Foreground
  fg:               '#1B1C22',
  fgSecondary:      '#52546B',
  fgMuted:          '#82849A',
  fgFaint:          '#B8BAC8',
  // Borders
  hairline:         'rgba(20,21,28,0.06)',
  border:           'rgba(20,21,28,0.10)',
  borderStrong:     'rgba(20,21,28,0.16)',
  // Block backgrounds
  hoverBg:          'rgba(20,21,28,0.035)',
  selectBg:         'rgba(138,134,229,0.13)',
  selectBgStrong:   'rgba(138,134,229,0.20)',
  // Drop indicators
  dropLine:         '#8A86E5',
  dropInto:         'rgba(138,134,229,0.10)',
  dropIntoBorder:   'rgba(138,134,229,0.55)',
  // Code
  codeBg:           'rgba(20,21,28,0.045)',
  codeBorder:       'rgba(20,21,28,0.06)',
  codeFg:           '#1B1C22',
  // Indent guide
  indentGuide:      'rgba(20,21,28,0.07)',
  // Caret
  caret:            '#1B1C22',
  // Accent (Sthalam)
  accent:           '#8A86E5',
  accentSoft:       '#A9A6F0',
  accentBg:         'rgba(138,134,229,0.10)',
  // Annotation
  annoFg:           '#7C5CFF',     // a friendlier hue for spec callouts
  annoLine:         'rgba(124,92,255,0.50)',
  annoBg:           '#FFF8E0',
  // Semantic
  err:              '#D8553F',
  errBg:            'rgba(216,85,63,0.08)',
};

const ED_FONT = {
  ui:      'Inter, system-ui, -apple-system, sans-serif',
  mono:    '"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace',
  display: '"Host Grotesk", Inter, system-ui, sans-serif',
};

// Geometry constants — these are the canonical numbers the Slint code
// will plug in. Keep this object in sync with the spec table.
const GEO = {
  docLeftPad:    56,  // page → text column
  docRightPad:   56,
  docTopPad:     32,
  gutterW:       32,  // narrow per brief
  gutterGap:     4,   // gutter → text
  iconBtn:       14,  // grip / plus icon hit-target (visual)
  iconBtnHit:    20,  // hover hit-target (square)
  rowPadY:       4,   // vertical padding inside row
  indentStep:    24,  // px per nesting level
  dropZoneTop:   0.33,// fraction of row height = "above sibling"
  dropZoneMid:   0.34,// "into as child" (only for nestable kinds)
  dropZoneBot:   0.33,// "below sibling"
};

// ─── Block kind definitions ───────────────────────────────────────
const KIND = {
  p:    { font: 15, weight: 400, line: 1.65,  py: 4,  letter:  0       },
  h1:   { font: 28, weight: 700, line: 1.25,  py: 16, letter: -0.02   },
  h2:   { font: 22, weight: 600, line: 1.3,   py: 12, letter: -0.012  },
  code: { font: 13, weight: 400, line: 1.55,  py: 10, letter:  0, mono: true },
  li:   { font: 15, weight: 400, line: 1.65,  py: 3,  letter:  0, bullet: true },
};

// ─── Caret (text-cursor) ──────────────────────────────────────────
function Caret({ height = 18, blink = false }) {
  return (
    <span
      className={blink ? 'ed-caret-blink' : ''}
      style={{
        display: 'inline-block', width: 1.5, height,
        background: ED.caret, marginLeft: 1, marginRight: -1,
        verticalAlign: 'text-bottom', position: 'relative', top: 2,
      }}
    />
  );
}

// ─── Gutter affordance (grip + plus) ──────────────────────────────
// `state` controls visibility; mockups freeze a moment in time.
//   'hidden'  — gutter is empty (no hover)
//   'shown'   — both buttons visible (row is hovered)
//   'gripActive' — grip is being held / menu open
//   'plusActive' — plus is being held / menu open
function Gutter({ state = 'hidden', onPlusLabel, onGripLabel }) {
  if (state === 'hidden') {
    return <div style={{ width: GEO.gutterW, flexShrink: 0 }} />;
  }
  const visible = true;
  return (
    <div style={{
      width: GEO.gutterW, flexShrink: 0,
      display: 'flex', alignItems: 'flex-start', justifyContent: 'flex-end',
      paddingTop: 3, gap: 0,
    }}>
      <PlusBtn active={state === 'plusActive'} />
      <GripBtn active={state === 'gripActive'} />
    </div>
  );
}

function PlusBtn({ active }) {
  return (
    <div style={{
      width: 18, height: 22,
      display: 'grid', placeItems: 'center',
      background: active ? 'rgba(20,21,28,0.08)' : 'transparent',
      borderRadius: 4,
      color: active ? ED.fgSecondary : ED.fgFaint,
      cursor: 'pointer',
      transition: 'background 80ms',
    }} title="Click to insert paragraph · drag for type picker">
      <svg width="11" height="11" viewBox="0 0 11 11" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
        <path d="M5.5 1.5v8M1.5 5.5h8" />
      </svg>
    </div>
  );
}

function GripBtn({ active }) {
  return (
    <div style={{
      width: 14, height: 22,
      display: 'grid', placeItems: 'center',
      background: active ? 'rgba(20,21,28,0.08)' : 'transparent',
      borderRadius: 4,
      color: active ? ED.fgSecondary : ED.fgFaint,
      cursor: active ? 'grabbing' : 'grab',
      transition: 'background 80ms',
    }} title="Click for menu · drag to reorder">
      <svg width="9" height="13" viewBox="0 0 9 13" fill="currentColor">
        <circle cx="2" cy="2" r="1.1"/><circle cx="7" cy="2" r="1.1"/>
        <circle cx="2" cy="6.5" r="1.1"/><circle cx="7" cy="6.5" r="1.1"/>
        <circle cx="2" cy="11" r="1.1"/><circle cx="7" cy="11" r="1.1"/>
      </svg>
    </div>
  );
}

// ─── Bullet for list items ───────────────────────────────────────
function Bullet({ depth = 0 }) {
  // depth 0: filled disc · depth 1: open ring · depth 2: filled square
  const style = { display: 'inline-block', width: 18, flexShrink: 0,
    color: ED.fgSecondary, textAlign: 'center', lineHeight: '1.65em',
    fontFamily: ED_FONT.ui, fontSize: 14, userSelect: 'none' };
  if (depth >= 2) return <span style={{ ...style, fontSize: 9, paddingTop: 5 }}>■</span>;
  if (depth === 1) return <span style={{ ...style, fontSize: 11 }}>○</span>;
  return <span style={style}>•</span>;
}

// ─── Block row ───────────────────────────────────────────────────
//
// A single block in the doc. Static — props pin a frozen state.
//
//   kind:        'p' | 'h1' | 'h2' | 'code' | 'li'
//   text:        string (plain — no inline formatting yet)
//   indent:      number (nesting depth; only meaningful for 'li')
//   gutter:      'hidden' | 'shown' | 'gripActive' | 'plusActive'
//   bg:          'none' | 'hover' | 'select' | 'dropInto'
//   caret:       boolean — render caret at end of text
//   placeholder: string — when text is empty, shown faint
//   dropLine:    'above' | 'below' | null
//   dropChild:   boolean — when true, this block is the drop-INTO target
//   ghost:       boolean — render as the picked-up dragging copy
//   indentGuide: 'none' | 'subtle' | 'strong' | 'live'
//                ('live' shows a soft accent guide for "active branch")
//
function Block({
  kind = 'p', text = '', indent = 0,
  gutter = 'hidden', bg = 'none',
  caret = false, placeholder = null,
  dropLine = null, dropChild = false, ghost = false,
  indentGuide = 'subtle', listDepth = 0,
  width = '100%',
  // optional decoration — child blocks rendered nested
  children = null,
}) {
  const k = KIND[kind];
  const fontSize = k.font;
  const fontFamily = k.mono ? ED_FONT.mono : ED_FONT.ui;
  const fontWeight = k.weight;
  const lineHeight = k.line;

  const isEmpty = !text;
  const showPlaceholder = isEmpty && placeholder;

  const blockBg = (
    bg === 'hover'    ? ED.hoverBg :
    bg === 'select'   ? ED.selectBg :
    bg === 'dropInto' ? ED.dropInto :
                        'transparent'
  );

  // Indent guides — one vertical hairline per nesting level, drawn in the
  // row's left padding. Color controlled by `indentGuide`.
  const guides = [];
  for (let i = 0; i < indent; i++) {
    const x = i * GEO.indentStep + 9;
    guides.push(
      <div key={i} style={{
        position: 'absolute', left: x, top: 0, bottom: 0,
        width: 1.5,
        background: indentGuide === 'live' && i === indent - 1
          ? ED.accentSoft
          : indentGuide === 'strong'
          ? ED.borderStrong
          : ED.indentGuide,
        opacity: indentGuide === 'none' ? 0 : 1,
      }} />
    );
  }

  return (
    <div style={{
      position: 'relative',
      paddingLeft: indent * GEO.indentStep,
      ...(ghost ? { opacity: 0.25 } : {}),
    }}>
      {guides}
      <div style={{
        position: 'relative',
        display: 'flex',
        alignItems: 'flex-start',
        background: blockBg,
        borderRadius: 4,
        transition: 'background 100ms',
      }}>
        {/* drop-line above */}
        {dropLine === 'above' && <DropLine pos="top" indent={indent} />}
        {/* drop-INTO outline (sits inside the row bg already) */}
        {dropChild && <DropIntoOutline />}

        {/* Gutter */}
        <Gutter state={gutter} />

        {/* Bullet (li) */}
        {kind === 'li' && (
          <div style={{
            paddingTop: k.py + 1,
            display: 'flex', alignItems: 'flex-start',
          }}>
            <Bullet depth={listDepth} />
          </div>
        )}

        {/* Content */}
        <div style={{
          flex: 1, minWidth: 0,
          paddingTop: k.py, paddingBottom: k.py,
          paddingLeft: GEO.gutterGap,
          ...(kind === 'code' ? {
            background: ED.codeBg,
            border: '1px solid ' + ED.codeBorder,
            borderRadius: 6,
            padding: `${k.py + 2}px 12px`,
            margin: '6px 0',
          } : {}),
        }}>
          <span style={{
            fontFamily, fontSize, fontWeight, lineHeight,
            letterSpacing: k.letter + 'em',
            color: showPlaceholder ? ED.fgFaint : ED.fg,
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-word',
          }}>
            {showPlaceholder ? placeholder : text}
            {caret && <Caret height={fontSize * lineHeight * 0.78} />}
          </span>
        </div>

        {/* drop-line below */}
        {dropLine === 'below' && <DropLine pos="bottom" indent={indent} />}
      </div>

      {/* Nested children (already wrapped in Block instances by caller) */}
      {children}
    </div>
  );
}

// ─── Drop indicators ──────────────────────────────────────────────
function DropLine({ pos = 'top', indent = 0, label = null }) {
  const top = pos === 'top' ? -2 : 'auto';
  const bottom = pos === 'bottom' ? -2 : 'auto';
  return (
    <div style={{
      position: 'absolute',
      left: GEO.gutterW + GEO.gutterGap, right: 0,
      top, bottom, height: 3,
      background: ED.dropLine,
      borderRadius: 2,
      boxShadow: '0 0 0 2px rgba(138,134,229,0.18)',
      pointerEvents: 'none',
      zIndex: 2,
    }}>
      {/* End caps — tiny dots so the line reads as a pin not a divider */}
      <div style={{
        position: 'absolute', left: -3, top: -2, width: 7, height: 7,
        borderRadius: 4, background: ED.dropLine,
      }} />
    </div>
  );
}

function DropIntoOutline() {
  return (
    <div style={{
      position: 'absolute', inset: -1,
      borderRadius: 5,
      boxShadow: 'inset 0 0 0 1.5px ' + ED.dropIntoBorder,
      pointerEvents: 'none',
      zIndex: 2,
    }} />
  );
}

// ─── Ghost (picked-up) preview floating beside the cursor ─────────
function GhostPreview({ kind = 'p', text, x = 0, y = 0, rot = -1.2, count = 1 }) {
  return (
    <div style={{
      position: 'absolute', left: x, top: y,
      transform: `rotate(${rot}deg)`,
      pointerEvents: 'none',
      zIndex: 30,
      filter: 'drop-shadow(0 14px 28px rgba(20,21,28,0.18)) drop-shadow(0 4px 8px rgba(20,21,28,0.10))',
    }}>
      <div style={{
        background: '#fff',
        borderRadius: 6,
        padding: '8px 14px 10px',
        minWidth: 220, maxWidth: 320,
        border: '1px solid ' + ED.border,
      }}>
        <Block kind={kind} text={text} gutter="hidden" />
      </div>
      {count > 1 && (
        <div style={{
          position: 'absolute', top: -8, right: -10,
          background: ED.accent, color: '#fff',
          fontFamily: ED_FONT.mono, fontWeight: 600, fontSize: 11,
          minWidth: 22, height: 22, padding: '0 6px',
          borderRadius: 11, display: 'grid', placeItems: 'center',
          boxShadow: '0 2px 8px rgba(138,134,229,0.5)',
        }}>{count}</div>
      )}
      {/* Cursor */}
      <div style={{
        position: 'absolute', left: 24, top: 18,
        transform: `rotate(${-rot}deg)`,
      }}>
        <CursorIcon />
      </div>
    </div>
  );
}

function CursorIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" style={{ filter: 'drop-shadow(0 1px 2px rgba(0,0,0,0.35))' }}>
      <path d="M3 2 L17 9 L10 11 L7 17 Z" fill="#1B1C22" stroke="#fff" strokeWidth="1.2" strokeLinejoin="round"/>
    </svg>
  );
}

// ─── Doc surface (page) ───────────────────────────────────────────
function DocSurface({ children, title = 'Untitled', dark = false, style = {}, breadcrumb = null }) {
  return (
    <div style={{
      width: '100%', height: '100%',
      background: dark ? '#0D0E13' : ED.pageBg,
      color: dark ? '#F5F5F7' : ED.fg,
      fontFamily: ED_FONT.ui,
      overflow: 'hidden',
      display: 'flex', flexDirection: 'column',
      ...style,
    }}>
      {/* tiny breadcrumb / status row to give the surface depth */}
      <div style={{
        height: 30, flexShrink: 0,
        display: 'flex', alignItems: 'center', gap: 8,
        padding: `0 ${GEO.docRightPad}px 0 ${GEO.docLeftPad}px`,
        borderBottom: '1px solid ' + (dark ? 'rgba(255,255,255,0.06)' : ED.hairline),
        fontFamily: ED_FONT.mono, fontSize: 10.5,
        color: dark ? '#7F8192' : ED.fgMuted,
        letterSpacing: '0.04em',
      }}>
        {breadcrumb || (
          <>
            <span>doc · /notes/architecture.md</span>
            <span style={{ flex: 1 }} />
            <span style={{ color: dark ? '#7EE787' : '#3CA269' }}>● synced · 2 peers</span>
          </>
        )}
      </div>

      {/* doc body */}
      <div style={{
        flex: 1, overflow: 'hidden',
        padding: `${GEO.docTopPad}px ${GEO.docRightPad}px 0 ${GEO.docLeftPad - GEO.gutterW}px`,
        // body indent is doc-left minus the gutter region so gutter affordances
        // sit *outside* the text column when shown.
      }}>
        {/* doc title */}
        <div style={{
          paddingLeft: GEO.gutterW + GEO.gutterGap,
          fontFamily: ED_FONT.display,
          fontSize: 32, fontWeight: 700, letterSpacing: '-0.018em',
          color: dark ? '#F5F5F7' : ED.fg,
          marginBottom: 18,
        }}>{title}</div>

        {/* blocks slot */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 0 }}>
          {children}
        </div>
      </div>
    </div>
  );
}

// ─── Annotation: small caption with a thin connector line ─────────
//
// Renders a label + a connector line pointing into the artboard.
// `from` and `to` are {x, y} in artboard coords; the label appears at `from`.
//
function Annotation({ from, to, label, side = 'right', maxWidth = 180 }) {
  const dx = to.x - from.x, dy = to.y - from.y;
  const len = Math.sqrt(dx*dx + dy*dy);
  const ang = Math.atan2(dy, dx) * 180 / Math.PI;
  return (
    <>
      {/* connector line */}
      <div style={{
        position: 'absolute',
        left: from.x, top: from.y,
        width: len, height: 0,
        borderTop: '1px dashed ' + ED.annoLine,
        transform: `rotate(${ang}deg)`,
        transformOrigin: '0 0',
        pointerEvents: 'none',
        zIndex: 5,
      }} />
      {/* end-cap dot */}
      <div style={{
        position: 'absolute',
        left: to.x - 3, top: to.y - 3, width: 6, height: 6,
        borderRadius: 3, background: ED.annoFg,
        zIndex: 5,
      }} />
      {/* label */}
      <div style={{
        position: 'absolute',
        left: side === 'right' ? from.x + 6 : 'auto',
        right: side === 'left' ? `calc(100% - ${from.x - 6}px)` : 'auto',
        top: from.y - 8,
        maxWidth,
        fontFamily: ED_FONT.mono, fontSize: 10.5, lineHeight: 1.35,
        color: ED.annoFg,
        background: ED.annoBg,
        padding: '3px 7px',
        borderRadius: 3,
        boxShadow: '0 1px 2px rgba(20,21,28,0.07)',
        zIndex: 6,
        whiteSpace: 'pre-wrap',
        textAlign: side === 'left' ? 'right' : 'left',
      }}>{label}</div>
    </>
  );
}

// ─── Title strip for an artboard ──────────────────────────────────
// Used as a small caption above each interaction-state mockup, inside
// the artboard. Keeps the artboard self-explanatory when downloaded.
function CaptionBar({ id, title, summary, tone = 'neutral' }) {
  const accent = tone === 'accent' ? ED.accent : ED.fgMuted;
  return (
    <div style={{
      padding: '14px 24px 12px',
      borderBottom: '1px solid ' + ED.hairline,
      background: '#fff',
      fontFamily: ED_FONT.ui,
      flexShrink: 0,
    }}>
      <div style={{
        display: 'flex', alignItems: 'baseline', gap: 8, marginBottom: 2,
      }}>
        <span style={{
          fontFamily: ED_FONT.mono, fontSize: 10.5, fontWeight: 600,
          color: accent, letterSpacing: '0.06em', textTransform: 'uppercase',
        }}>{id}</span>
        <span style={{
          fontFamily: ED_FONT.display, fontSize: 16, fontWeight: 600,
          color: ED.fg, letterSpacing: '-0.01em',
        }}>{title}</span>
      </div>
      {summary && (
        <div style={{
          fontSize: 12, color: ED.fgSecondary, lineHeight: 1.45,
          maxWidth: 720,
        }}>{summary}</div>
      )}
    </div>
  );
}

// ─── Floating menus ───────────────────────────────────────────────

// Block handle menu — the popover that opens when grip is clicked.
function BlockHandleMenu({ x, y }) {
  const items = [
    { kind: 'group', label: 'BLOCK' },
    { icon: 'trash',   label: 'Delete',           shortcut: '⌫' },
    { icon: 'copy',    label: 'Duplicate',        shortcut: '⌘D' },
    { icon: 'move',    label: 'Move to…',         shortcut: '⌘⇧↑' },
    { kind: 'sep' },
    { kind: 'group', label: 'TURN INTO' },
    { icon: 'pgraph',  label: 'Paragraph',        shortcut: '⌘⌥0' },
    { icon: 'h1',      label: 'Heading 1',        shortcut: '⌘⌥1' },
    { icon: 'h2',      label: 'Heading 2',        shortcut: '⌘⌥2' },
    { icon: 'list',    label: 'List item',        shortcut: '⌘⌥3' },
    { icon: 'code',    label: 'Code',             shortcut: '⌘⌥4' },
    { kind: 'sep' },
    { kind: 'group', label: 'COLOR' },
    { kind: 'colors' },
  ];
  return (
    <div style={{
      position: 'absolute', left: x, top: y,
      width: 240,
      background: '#fff',
      borderRadius: 8,
      border: '1px solid ' + ED.border,
      boxShadow: '0 12px 36px rgba(20,21,28,0.18), 0 2px 8px rgba(20,21,28,0.08)',
      padding: 4,
      fontFamily: ED_FONT.ui,
      zIndex: 20,
    }}>
      {items.map((it, i) => {
        if (it.kind === 'group') {
          return (
            <div key={i} style={{
              padding: '8px 10px 4px',
              fontFamily: ED_FONT.mono, fontSize: 9.5, fontWeight: 600,
              color: ED.fgMuted, letterSpacing: '0.1em',
            }}>{it.label}</div>
          );
        }
        if (it.kind === 'sep') {
          return <div key={i} style={{ height: 1, background: ED.hairline, margin: '4px 0' }} />;
        }
        if (it.kind === 'colors') {
          const swatches = ['#1B1C22', '#D8553F', '#E5A33C', '#3CA269', '#4A7CD8', ED.accent];
          return (
            <div key={i} style={{
              display: 'flex', gap: 6, padding: '4px 10px 6px',
            }}>
              {swatches.map((c, j) => (
                <div key={j} style={{
                  width: 18, height: 18, borderRadius: 4,
                  background: c,
                  border: '1px solid ' + ED.hairline,
                  ...(j === 0 ? { boxShadow: '0 0 0 1.5px ' + ED.accent, transform: 'scale(1.05)' } : {}),
                }} />
              ))}
              <span style={{ marginLeft: 'auto', flex: '0 0 auto',
                fontSize: 11, color: ED.fgMuted, alignSelf: 'center',
              }}>default</span>
            </div>
          );
        }
        const focused = i === 1; // first action highlighted (keyboard arrived here)
        return (
          <div key={i} style={{
            display: 'flex', alignItems: 'center', gap: 10,
            padding: '6px 10px',
            borderRadius: 5,
            background: focused ? ED.hoverBg : 'transparent',
            color: it.icon === 'trash' ? ED.err : ED.fg,
            fontSize: 13, lineHeight: 1.2,
          }}>
            <BlockMenuIcon name={it.icon} />
            <span style={{ flex: 1 }}>{it.label}</span>
            <span style={{
              fontFamily: ED_FONT.mono, fontSize: 10.5, color: ED.fgMuted,
            }}>{it.shortcut}</span>
          </div>
        );
      })}
    </div>
  );
}

function BlockMenuIcon({ name }) {
  const stroke = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.4, strokeLinecap: 'round', strokeLinejoin: 'round' };
  const sw = 14, sh = 14;
  switch (name) {
    case 'trash':  return <svg width={sw} height={sh} viewBox="0 0 14 14" {...stroke}><path d="M2.5 4h9M5 4V2.5h4V4M3.5 4l.5 7.5h6L10.5 4M5.5 6.5v3M8.5 6.5v3"/></svg>;
    case 'copy':   return <svg width={sw} height={sh} viewBox="0 0 14 14" {...stroke}><rect x="3.5" y="3.5" width="7" height="8" rx="1.2"/><path d="M2 9V2.5h6.5"/></svg>;
    case 'move':   return <svg width={sw} height={sh} viewBox="0 0 14 14" {...stroke}><path d="M7 1.5v11M1.5 7h11M7 1.5l-1.5 1.5M7 1.5l1.5 1.5M7 12.5l-1.5-1.5M7 12.5l1.5-1.5M1.5 7l1.5-1.5M1.5 7l1.5 1.5M12.5 7l-1.5-1.5M12.5 7l-1.5 1.5"/></svg>;
    case 'pgraph': return <svg width={sw} height={sh} viewBox="0 0 14 14" {...stroke}><path d="M2.5 3h9M2.5 7h9M2.5 11h6"/></svg>;
    case 'h1':     return <svg width={sw} height={sh} viewBox="0 0 14 14" {...stroke}><path d="M2 3v8M6.5 3v8M2 7h4.5M9 5l1.5-1v7M10 11h1"/></svg>;
    case 'h2':     return <svg width={sw} height={sh} viewBox="0 0 14 14" {...stroke}><path d="M2 3v8M6 3v8M2 7h4M8 5.5c0-1 .8-1.5 1.7-1.5 1 0 1.8.6 1.8 1.5 0 1.5-3.5 3-3.5 5.5h3.5"/></svg>;
    case 'list':   return <svg width={sw} height={sh} viewBox="0 0 14 14" {...stroke}><circle cx="3" cy="4" r="0.8" fill="currentColor"/><circle cx="3" cy="10" r="0.8" fill="currentColor"/><path d="M5.5 4h6M5.5 10h6"/></svg>;
    case 'code':   return <svg width={sw} height={sh} viewBox="0 0 14 14" {...stroke}><path d="M5 4l-3 3 3 3M9 4l3 3-3 3"/></svg>;
    default: return null;
  }
}

// Plus-button menu — opens when + is *clicked-and-held* (or activated by
// keyboard). Click-only inserts an empty paragraph immediately.
function PlusMenu({ x, y }) {
  const items = [
    { id: 'p',    kind: 'p',  icon: 'pgraph', label: 'Paragraph', hint: 'Plain text' },
    { id: 'h1',   kind: 'h1', icon: 'h1',     label: 'Heading 1', hint: 'Section title' },
    { id: 'h2',   kind: 'h2', icon: 'h2',     label: 'Heading 2', hint: 'Subsection' },
    { id: 'li',   kind: 'li', icon: 'list',   label: 'List item', hint: 'Bullet list' },
    { id: 'code', kind: 'code', icon: 'code', label: 'Code',      hint: 'Monospace block' },
  ];
  return (
    <div style={{
      position: 'absolute', left: x, top: y,
      width: 232,
      background: '#fff',
      borderRadius: 8,
      border: '1px solid ' + ED.border,
      boxShadow: '0 12px 36px rgba(20,21,28,0.18), 0 2px 8px rgba(20,21,28,0.08)',
      padding: 4,
      fontFamily: ED_FONT.ui,
      zIndex: 20,
    }}>
      <div style={{
        padding: '8px 10px 4px',
        fontFamily: ED_FONT.mono, fontSize: 9.5, fontWeight: 600,
        color: ED.fgMuted, letterSpacing: '0.1em',
      }}>INSERT BLOCK</div>
      {items.map((it, i) => {
        const focused = i === 0;
        return (
          <div key={it.id} style={{
            display: 'flex', alignItems: 'center', gap: 10,
            padding: '7px 10px',
            borderRadius: 5,
            background: focused ? ED.accentBg : 'transparent',
          }}>
            <div style={{
              width: 26, height: 26, borderRadius: 5,
              background: '#fff',
              border: '1px solid ' + ED.border,
              display: 'grid', placeItems: 'center',
              color: ED.fg,
            }}>
              <BlockMenuIcon name={it.icon} />
            </div>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 13, color: ED.fg, lineHeight: 1.2 }}>{it.label}</div>
              <div style={{ fontSize: 11, color: ED.fgMuted, marginTop: 1 }}>{it.hint}</div>
            </div>
            {focused && <Kbd>↵</Kbd>}
          </div>
        );
      })}
    </div>
  );
}

// Slash menu — wider, with search field and grouped sections.
function SlashMenu({ x, y, query = '' }) {
  const groups = [
    {
      label: 'BASIC',
      items: [
        { kind: 'p',  icon: 'pgraph', label: 'Paragraph', hint: 'Plain block of text' },
        { kind: 'h1', icon: 'h1',     label: 'Heading 1', hint: 'Big section header' },
        { kind: 'h2', icon: 'h2',     label: 'Heading 2', hint: 'Medium section header' },
      ],
    },
    {
      label: 'LISTS',
      items: [
        { kind: 'li', icon: 'list',   label: 'Bulleted list', hint: 'Use Tab/Shift-Tab to nest' },
      ],
    },
    {
      label: 'CODE',
      items: [
        { kind: 'code', icon: 'code', label: 'Code block', hint: 'Monospaced fixed-width text' },
      ],
    },
  ];

  const filterMatches = (s) => !query || s.toLowerCase().includes(query.toLowerCase());

  // Flat list with index to figure out which item is "selected"
  const flat = [];
  groups.forEach((g) => g.items.forEach((it) => { if (filterMatches(it.label)) flat.push({ ...it, group: g.label }); }));
  const selectedIdx = 0;

  return (
    <div style={{
      position: 'absolute', left: x, top: y,
      width: 320,
      background: '#fff',
      borderRadius: 10,
      border: '1px solid ' + ED.border,
      boxShadow: '0 16px 48px rgba(20,21,28,0.20), 0 2px 8px rgba(20,21,28,0.08)',
      overflow: 'hidden',
      fontFamily: ED_FONT.ui,
      zIndex: 20,
    }}>
      {/* Search header */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 8,
        padding: '10px 12px',
        borderBottom: '1px solid ' + ED.hairline,
      }}>
        <span style={{
          fontFamily: ED_FONT.mono, fontSize: 13, color: ED.accent, fontWeight: 600,
        }}>/</span>
        <span style={{ fontSize: 13, color: ED.fg, flex: 1 }}>
          {query || <span style={{ color: ED.fgMuted }}>Filter…</span>}
          <Caret height={14} blink />
        </span>
        <span style={{
          fontFamily: ED_FONT.mono, fontSize: 10, color: ED.fgMuted,
        }}>esc to close</span>
      </div>
      {/* Items */}
      <div style={{ padding: 4, maxHeight: 280, overflow: 'hidden' }}>
        {groups.map((g, gi) => {
          const items = g.items.filter((it) => filterMatches(it.label));
          if (!items.length) return null;
          return (
            <div key={gi}>
              <div style={{
                padding: '8px 10px 4px',
                fontFamily: ED_FONT.mono, fontSize: 9.5, fontWeight: 600,
                color: ED.fgMuted, letterSpacing: '0.1em',
              }}>{g.label}</div>
              {items.map((it, i) => {
                const idx = flat.findIndex((f) => f.kind === it.kind);
                const focused = idx === selectedIdx;
                return (
                  <div key={it.kind} style={{
                    display: 'flex', alignItems: 'center', gap: 10,
                    padding: '7px 10px',
                    borderRadius: 6,
                    background: focused ? ED.accentBg : 'transparent',
                  }}>
                    <div style={{
                      width: 30, height: 30, borderRadius: 6,
                      background: '#fff',
                      border: '1px solid ' + ED.border,
                      display: 'grid', placeItems: 'center',
                      color: ED.fg,
                    }}>
                      <BlockMenuIcon name={it.icon} />
                    </div>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: 13, color: ED.fg, lineHeight: 1.2 }}>{it.label}</div>
                      <div style={{ fontSize: 11, color: ED.fgMuted, marginTop: 1 }}>{it.hint}</div>
                    </div>
                    {focused && <Kbd>↵</Kbd>}
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
      {/* Footer */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 12,
        padding: '8px 12px',
        borderTop: '1px solid ' + ED.hairline,
        background: '#FAFAFB',
        fontFamily: ED_FONT.mono, fontSize: 10.5, color: ED.fgMuted,
      }}>
        <span><Kbd>↑</Kbd><Kbd>↓</Kbd> nav</span>
        <span><Kbd>↵</Kbd> select</span>
        <span><Kbd>esc</Kbd> dismiss</span>
      </div>
    </div>
  );
}

// Bulk-action bar — floats at the bottom-center when ≥1 block is selected.
function BulkBar({ count = 3 }) {
  const items = [
    { icon: 'trash',  label: 'Delete',     short: '⌫' },
    { icon: 'copy',   label: 'Duplicate',  short: '⌘D' },
    { icon: 'pgraph', label: 'Turn into…', short: 'T' },
    { icon: 'move',   label: 'Move to…',   short: '⌘⇧↑' },
  ];
  return (
    <div style={{
      position: 'absolute', left: '50%', bottom: 16,
      transform: 'translateX(-50%)',
      background: '#1B1C22', color: '#F5F5F7',
      borderRadius: 10,
      boxShadow: '0 10px 30px rgba(0,0,0,0.25)',
      padding: '4px 4px 4px 12px',
      display: 'flex', alignItems: 'center', gap: 4,
      fontFamily: ED_FONT.ui,
      zIndex: 20,
    }}>
      <div style={{
        fontFamily: ED_FONT.mono, fontSize: 11.5,
        color: ED.accentSoft, fontWeight: 600,
        marginRight: 8,
      }}>{count} selected</div>
      <div style={{ width: 1, height: 18, background: 'rgba(255,255,255,0.12)', marginRight: 4 }} />
      {items.map((it, i) => (
        <div key={i} style={{
          display: 'flex', alignItems: 'center', gap: 6,
          padding: '6px 9px',
          borderRadius: 6, fontSize: 12,
          color: it.icon === 'trash' ? '#F47068' : '#F5F5F7',
          background: i === 0 ? 'rgba(244,112,104,0.12)' : 'transparent',
        }}>
          <BlockMenuIcon name={it.icon} />
          <span>{it.label}</span>
          <span style={{
            fontFamily: ED_FONT.mono, fontSize: 10, color: '#7F8192',
            marginLeft: 2,
          }}>{it.short}</span>
        </div>
      ))}
      <div style={{ width: 1, height: 18, background: 'rgba(255,255,255,0.12)', margin: '0 2px 0 4px' }} />
      <div style={{
        width: 26, height: 26, display: 'grid', placeItems: 'center',
        color: '#7F8192', borderRadius: 5,
      }}>×</div>
    </div>
  );
}

// ─── Marquee selection rectangle ──────────────────────────────────
function Marquee({ x, y, w, h }) {
  return (
    <div style={{
      position: 'absolute', left: x, top: y, width: w, height: h,
      background: 'rgba(138,134,229,0.08)',
      border: '1px solid ' + ED.dropIntoBorder,
      borderRadius: 2,
      pointerEvents: 'none',
      zIndex: 4,
    }} />
  );
}

// ─── Small kbd pill ──────────────────────────────────────────────
function Kbd({ children, dark = false }) {
  return (
    <span style={{
      display: 'inline-grid', placeItems: 'center',
      minWidth: 18, height: 18, padding: '0 5px',
      borderRadius: 4,
      background: dark ? 'rgba(255,255,255,0.08)' : '#F4F4F6',
      border: '1px solid ' + (dark ? 'rgba(255,255,255,0.12)' : ED.border),
      color: dark ? '#B6B7C3' : ED.fgSecondary,
      fontFamily: ED_FONT.mono, fontSize: 10.5, lineHeight: 1,
      marginLeft: 1, marginRight: 1,
    }}>{children}</span>
  );
}

// ─── Stage helpers ────────────────────────────────────────────────

// Wraps an artboard in a top caption + the doc surface, with a defined
// horizontal slot for floating overlays placed by callers.
function Stage({ id, title, summary, dark = false, breadcrumb, children, surfaceStyle }) {
  return (
    <div style={{
      width: '100%', height: '100%',
      display: 'flex', flexDirection: 'column',
      background: dark ? '#0A0B10' : '#fff',
    }}>
      <CaptionBar id={id} title={title} summary={summary} />
      <div style={{ flex: 1, position: 'relative', overflow: 'hidden' }}>
        <DocSurface dark={dark} breadcrumb={breadcrumb} style={surfaceStyle}>
          {children}
        </DocSurface>
      </div>
    </div>
  );
}

// Export to window — these are picked up by inline scripts in the HTML.
Object.assign(window, {
  ED, ED_FONT, GEO, KIND,
  Block, Gutter, PlusBtn, GripBtn, Bullet, Caret,
  DropLine, DropIntoOutline, GhostPreview, CursorIcon,
  DocSurface, CaptionBar, Stage,
  BlockHandleMenu, PlusMenu, SlashMenu, BulkBar, Marquee,
  Annotation, Kbd, BlockMenuIcon,
});

// Caret blink keyframes — injected once.
if (typeof document !== 'undefined' && !document.getElementById('ed-caret-style')) {
  const s = document.createElement('style');
  s.id = 'ed-caret-style';
  s.textContent = '@keyframes ed-caret-blink{0%,49%{opacity:1}50%,100%{opacity:0}}.ed-caret-blink{animation:ed-caret-blink 1s steps(1) infinite}';
  document.head.appendChild(s);
}
