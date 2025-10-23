# Graph View Migration Plan

## Overview

We are migrating from the current full-block canvas rendering to a **graph-based visual editor** similar to Miro/Excalidraw. This solves the nested container problem where deeply nested elements create unusable "matryoshka doll" layouts.

## The Problem

Currently, when importing nested HUML templates (see `templates/nested-containers.huml`), containers are rendered physically nested inside each other:

```
┌─────────────────────────────────────┐
│ Top Container                       │
│ ┌─────────────────────────────────┐ │
│ │ Level 2A Container              │ │
│ │ ┌─────────────────────────────┐ │ │
│ │ │ Level 3 Container           │ │ │
│ │ │ ┌─────────────────────────┐ │ │ │
│ │ │ │ Level 4 Container       │ │ │ │
│ │ │ └─────────────────────────┘ │ │ │
│ │ └─────────────────────────────┘ │ │
│ └─────────────────────────────────┘ │
└─────────────────────────────────────┘
```

This makes it:
- Impossible to see the overall structure
- Difficult to select and edit deeply nested elements
- Hard to understand parent-child relationships
- Challenging to navigate between components

## The Solution: Graph View

Transform all blocks into **compact rectangular nodes** with **connection lines** showing relationships:

```
┌────────────┐
│ 🖥️ screen  │
│ Main       │
└──┬─────────┘
   │
   ├─────────┬─────────┬─────────┐
   │         │         │         │
┌──┴──┐  ┌──┴──┐  ┌───┴───┐  ┌──┴──┐
│ H   │  │ T   │  │ 📦    │  │ →   │
│Title│  │Text │  │ Top   │  │ Nav │────┐
└─────┘  └─────┘  └───┬───┘  └─────┘    │
                      │                  │
                   ┌──┴──┐               │
                   │ 📦  │               │
                   │L2-A │               │
                   └──┬──┘               │
                      │                  │
                   ┌──┴──┐               │
                   │ 📦  │               │
                   │L3   │               │
                   └─────┘               │
                                         ▼
                                  ┌────────────┐
                                  │ 🖥️ screen  │
                                  │ Details    │
                                  └────────────┘
```

### Key Benefits

1. **Clear Hierarchy** - See entire structure at a glance
2. **No Nesting Chaos** - Each node is independent
3. **Visual Connections** - Parent-child relationships shown with lines
4. **Navigation Paths** - Nav buttons show target connections
5. **Preview on Demand** - Ctrl+Right Click to see rendered output
6. **Edit on Demand** - Ctrl+Right Click text nodes to edit markdown

## Interaction Model

### Graph Mode (Default)

- **All blocks** rendered as compact rectangular cards (200px × 60px)
- **Connection lines** show relationships:
  - Blue subtle lines: Parent → Child
  - Green dashed lines: Navigation → Target
- **Click** on node → Properties panel opens (graph remains visible)
- **Drag** nodes to reposition
- **Pan/Zoom** canvas with existing controls
- **Selected node** highlights its connections

### Preview Mode (Ctrl+Right Click on Containers)

When you Ctrl+Right Click on `screen-container` or `section-container`:

- Enter **fullscreen preview mode**
- Container rendered with full CSS
- All children rendered inside with **inherited styles**
- Navigation buttons work
- Press **Esc** to return to graph

### Editor Mode (Ctrl+Right Click on Text/Markdown)

When you Ctrl+Right Click on `text`, `heading`, or `markdown-text`:

- Enter **fullscreen markdown editor**
- Live preview toggle
- Syntax highlighting
- **Save** updates block content
- **Cancel** or **Esc** returns to graph

## Technical Architecture

### Components

1. **GraphNode.svelte** (NEW)
   - Compact card representation of any block
   - Icon + type + name/content preview
   - Shows child count
   - Shows navigation target
   - Hover effects and selection state

2. **ConnectionLines.svelte** (NEW)
   - SVG overlay for all connection lines
   - Miro-style curved Bezier paths
   - Adaptive curves based on node positions
   - Highlights connections when node selected
   - Animated flow for navigation connections

3. **TextEditor.svelte** (NEW)
   - Fullscreen markdown editor
   - Edit/Preview toggle
   - Save/Cancel actions
   - Esc key handler

4. **ContainerPreview.svelte** (NEW)
   - Wrapper around FullScreenViewer
   - Shows single container in isolation
   - Back button and Esc handler

5. **Canvas.svelte** (UPDATED)
   - Remove old Block rendering
   - Always render GraphNode + ConnectionLines
   - Keep pan/zoom logic
   - Add Ctrl+Right Click handler

6. **WebsiteBuilder.svelte** (UPDATED)
   - Add view mode state: 'graph' | 'container-preview' | 'text-editor'
   - Route Ctrl+Right Click based on block type
   - Handle Esc key to return to graph

### Data Flow

```
User Action → WebsiteBuilder → Mode Switch
                ↓
    ┌───────────┼───────────┐
    │           │           │
  Click    Ctrl+RC     Ctrl+RC
    │      Container    Text
    ↓           ↓           ↓
Properties  Preview    Editor
  Panel      Mode       Mode
    ↓           ↓           ↓
 (Graph)    FullScreen  Markdown
 visible     Render     Editor
```

## Connection Line Algorithm

Uses Miro-style organic Bezier curves:

```typescript
function getMiroPath(from, to) {
  // Start: center-bottom of source
  const x1 = from.x + from.width / 2;
  const y1 = from.y + from.height;

  // End: center-top of target
  const x2 = to.x + to.width / 2;
  const y2 = to.y;

  // Calculate adaptive curve strength
  const distance = Math.sqrt((x2-x1)² + (y2-y1)²);
  const curvature = Math.min(distance * 0.4, 100);

  // Control points for smooth flow
  const cp1x = x1 + (x2 - x1) * 0.25;
  const cp1y = y1 + curvature;
  const cp2x = x2 - (x2 - x1) * 0.25;
  const cp2y = y2 - curvature;

  return `M ${x1} ${y1} C ${cp1x} ${cp1y}, ${cp2x} ${cp2y}, ${x2} ${y2}`;
}
```

## Layout Flexibility

### From HUML Templates

Support layout hints in HUML:

```huml
name: "My App"
layout: "horizontal"  # or "vertical" or "free"
screenSpacing: 1500   # horizontal distance between screens
levelSpacing: 300     # vertical distance between levels

screens::
  - id: "home"
    x: 100            # Manual positioning
    y: 100

  - id: "about"
    # x, y omitted = auto-positioned based on layout
```

### Manual Repositioning

Users can drag nodes to any position - graph adapts connection lines automatically.

## Visual Design

### Node Cards

- **Header**: Icon + Type (uppercase, gray)
- **Content**: Name or content preview (1 line, ellipsis)
- **Footer**: Child count or navigation target
- **Badge**: "Entry" badge for entry point screens
- **Colors**: Type-specific accent colors
  - Screens: Blue (`#89b4fa`)
  - Containers: Green (`#a6e3a1`)
  - Navigation: Yellow (`#f9e2af`)
  - Forms: Purple (`#cba6f7`)

### Connection Lines

- **Parent → Child**:
  - Blue (`#89b4fa`)
  - Opacity: 0.25 (subtle)
  - Width: 2px
  - Highlighted when either node selected

- **Navigation → Target**:
  - Green (`#a6e3a1`)
  - Dashed (6px dash, 4px gap)
  - Opacity: 0.4
  - **Animated flow** when navigation button selected
  - Arrowhead marker

## Migration Strategy

### Phase 1: Core Graph Components ✅
- Create GraphNode.svelte
- Create ConnectionLines.svelte
- Update Canvas.svelte to render graph mode

### Phase 2: Mode Switching
- Add view mode state to WebsiteBuilder
- Implement Ctrl+Right Click detection
- Create TextEditor.svelte
- Create ContainerPreview.svelte
- Add Esc key handling

### Phase 3: Enhanced HUML Support
- Update templateImporter.ts for layout options
- Support horizontal/vertical/free layouts
- Support manual positioning vs auto-layout

### Phase 4: Polish & UX
- Smooth animations for mode transitions
- Keyboard shortcuts (e.g., 'P' for preview)
- Minimap for large graphs
- Search/filter nodes
- Collapse/expand subtrees

## Testing Plan

1. Import existing nested-containers.huml template
2. Verify all nodes render as compact cards
3. Verify parent-child connection lines appear
4. Click nav button → check line animates to target
5. Ctrl+Right Click container → fullscreen preview
6. Ctrl+Right Click text → markdown editor
7. Verify Esc returns to graph in all cases
8. Test pan/zoom still works
9. Test drag to reposition nodes
10. Verify properties panel updates on click

## Success Metrics

- ✅ Can visualize 4+ levels of nesting clearly
- ✅ Can select any node in 1 click
- ✅ Can understand navigation flow from graph
- ✅ Can preview containers with full CSS
- ✅ Can edit text without leaving graph mode
- ✅ Layout remains usable with 50+ nodes

## References

- Problem visualization: `nested.png`
- Example template: `templates/nested-containers.huml`
- Existing components: `Canvas.svelte`, `FullScreenViewer.svelte`
- Inspiration: Miro, Excalidraw, Figma's component view
