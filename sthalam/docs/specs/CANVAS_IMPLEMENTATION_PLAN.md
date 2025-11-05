# Canvas Block Implementation Plan

**Status:** 📋 Planning
**Created:** 2025-01-06
**Goal:** Implement unified canvas block with pattern, chart, interactive, and diagram modes

---

## 🎯 Objectives

1. **Implement** CanvasBlock.svelte with multiple modes
2. **Integrate** OCaml WASM evaluator for pattern expressions
3. **Use** WebGL for GPU-accelerated rendering
4. **Support** interactive games, data charts, and diagrams
5. **Achieve** 60 FPS performance target
6. **Enable** creative templates (generative art, games, visualizations)

---

## 📊 Current State

### CanvasBlock Status
- **Type defined:** ✅ `/src/lib/types/huml.ts` (CanvasBlock interface exists)
- **Component:** ❌ `/src/renderer/blocks/CanvasBlock.svelte` (TODO stub only)
- **BlockRenderer:** ✅ Already imports and registers CanvasBlock
- **Spec exists:** ✅ `/docs/specs/CANVAS_BLOCK_SPEC.md` (complete)

### Dependencies Ready
- ✅ OCaml WASM evaluator (cel_eval.js)
- ✅ CEL service (`celEvaluator.ts`)
- ✅ Svelte 5 reactive system
- ✅ WebGL support in browsers
- ✅ Tauri desktop app framework

### Blockers
- ❌ Math functions not in OCaml (sin, cos, sqrt, atan2)
- ❌ Geometry functions not in OCaml (distance, lerp, clamp)

**Resolution:** Implement OCaml refactor first (see OCAML_REFACTOR_PLAN.md)

---

## 🏗️ Architecture Overview

```
┌────────────────────────────────────────┐
│      HUML Template (User)              │
│  expressions, mode, dimensions         │
└───────────────┬────────────────────────┘
                │
┌───────────────▼────────────────────────┐
│     CanvasBlock.svelte                 │
│  • Setup WebGL/Canvas 2D contexts     │
│  • Animation loop (requestAnimFrame)   │
│  • Mouse/keyboard event handlers       │
│  • Mode switching logic                │
└──┬─────────────────────────────────┬───┘
   │                                 │
   │ evaluateGridCEL()              │ texImage2D()
   ↓                                 ↓
┌──────────────┐            ┌───────────────┐
│ OCaml WASM   │   data     │  WebGL        │
│ Evaluator    │  ────────→ │  Renderer     │
│              │ Uint8Array │               │
│ • sin(x)     │            │ • Texture     │
│ • sqrt(y)    │            │ • Shader      │
│ • 10,000x    │            │ • drawArrays  │
└──────────────┘            └───────────────┘
                                    │
                            ┌───────▼────────┐
                            │  Canvas 2D     │
                            │  • Text labels │
                            │  • UI overlays │
                            └────────────────┘
```

---

## 📋 Implementation Plan

### Phase 1: Type Definition & Stub Setup (1 hour)

**File:** `/src/lib/types/huml.ts`

**Update CanvasBlock interface:**
```typescript
export interface CanvasBlock extends BaseBlock {
  type: 'canvas';

  // Core properties
  width?: number;              // Default: 600
  height?: number;             // Default: 400

  // Mode selection
  mode?: 'pattern' | 'chart' | 'interactive' | 'diagram';  // Default: 'pattern'

  // Pattern mode
  gridSize?: number;           // Grid resolution (e.g., 100x100)
  cellSize?: number;           // Pixel size of each cell
  pattern?: string;            // CEL expression name or direct expression
  expressions?: Record<string, string>;  // Named CEL expressions

  // Animation
  autoplay?: boolean;          // Default: true
  fps?: number;                // Target FPS (default: 60)
  onRender?: string;           // Action name to call each frame

  // Chart mode
  chartType?: 'line' | 'bar' | 'scatter' | 'pie';
  data?: string;               // CEL expression returning data array
  labels?: string;             // CEL expression returning labels
  colors?: Record<string, string>;

  // Interactive mode
  onMouseMove?: string;        // Action name
  onMouseClick?: string;       // Action name
  onClick?: string;            // Action name
  onKeyDown?: string;          // Action name

  // Diagram mode
  diagramType?: 'flowchart' | 'sequence' | 'state' | 'tree';
  nodes?: Array<{
    id: string;
    label: string;
    x?: number;
    y?: number;
    shape?: 'rect' | 'circle' | 'diamond' | 'roundedRect';
    color?: string;
    when?: string;  // Conditional visibility
  }>;
  edges?: Array<{
    from: string;
    to: string;
    label?: string;
    color?: string;
    width?: number;
  }>;
}
```

**Tasks:**
- [ ] Update CanvasBlock interface with all properties
- [ ] Export interface
- [ ] Update Block union type

---

### Phase 2: Pattern Mode - WebGL Setup (4-6 hours)

**File:** `/src/renderer/blocks/CanvasBlock.svelte`

**Implement WebGL renderer for pattern mode:**

```svelte
<script lang="ts">
import type { CanvasBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { evaluateCEL, interpolateCEL } from '../../lib/services/celEvaluator';
import { onMount, onDestroy } from 'svelte';

interface Props {
  block: CanvasBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();

// Canvas elements
let canvasElement: HTMLCanvasElement;
let gl: WebGL2RenderingContext | null = null;
let canvas2d: CanvasRenderingContext2D | null = null;

// Animation state
let animationId: number | null = null;
let time = $state(0);
let frameCount = $state(0);
let fps = $state(0);
let lastFrameTime = 0;

// WebGL resources
let texture: WebGLTexture | null = null;
let program: WebGLProgram | null = null;
let positionBuffer: WebGLBuffer | null = null;

// Configuration with defaults
const mode = $derived(block.mode || 'pattern');
const width = $derived(block.width || 600);
const height = $derived(block.height || 400);
const gridSize = $derived(block.gridSize || 100);
const cellSize = $derived(block.cellSize || Math.floor(width / gridSize));
const autoplay = $derived(block.autoplay !== false);
const targetFps = $derived(block.fps || 60);

// Get active expression
const activeExpression = $derived.by(() => {
  if (block.pattern && block.expressions) {
    return block.expressions[block.pattern] || block.pattern;
  }
  return block.pattern || 'sin(x * 0.1 + time)';
});

// Initialize WebGL
function initWebGL() {
  gl = canvasElement.getContext('webgl2');
  if (!gl) {
    console.error('[CanvasBlock] WebGL2 not supported');
    return false;
  }

  // Create shader program
  const vertexShader = createShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER_SOURCE);
  const fragmentShader = createShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER_SOURCE);

  program = createProgram(gl, vertexShader, fragmentShader);
  if (!program) return false;

  // Create position buffer (full-screen quad)
  positionBuffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
  const positions = new Float32Array([
    -1, -1,  // Bottom-left
     1, -1,  // Bottom-right
    -1,  1,  // Top-left
     1,  1,  // Top-right
  ]);
  gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);

  // Create texture for pattern data
  texture = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

  return true;
}

// Shader sources
const VERTEX_SHADER_SOURCE = `#version 300 es
  in vec2 a_position;
  out vec2 v_texCoord;

  void main() {
    gl_Position = vec4(a_position, 0.0, 1.0);
    v_texCoord = a_position * 0.5 + 0.5;
  }
`;

const FRAGMENT_SHADER_SOURCE = `#version 300 es
  precision highp float;

  in vec2 v_texCoord;
  out vec4 outColor;

  uniform sampler2D u_texture;

  void main() {
    float value = texture(u_texture, v_texCoord).r;

    // Map [-1, 1] to color
    // value = 0 (black) to 1 (white) grayscale
    // Could also use color mapping here
    vec3 color = vec3(value);

    outColor = vec4(color, 1.0);
  }
`;

// Create shader helper
function createShader(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) return null;

  gl.shaderSource(shader, source);
  gl.compileShader(shader);

  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    console.error('[CanvasBlock] Shader compilation error:', gl.getShaderInfoLog(shader));
    gl.deleteShader(shader);
    return null;
  }

  return shader;
}

// Create program helper
function createProgram(gl: WebGL2RenderingContext, vertexShader: WebGLShader, fragmentShader: WebGLShader): WebGLProgram | null {
  const program = gl.createProgram();
  if (!program) return null;

  gl.attachShader(program, vertexShader);
  gl.attachShader(program, fragmentShader);
  gl.linkProgram(program);

  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.error('[CanvasBlock] Program linking error:', gl.getProgramInfoLog(program));
    gl.deleteProgram(program);
    return null;
  }

  return program;
}

// Render pattern mode
async function renderPattern() {
  if (!gl || !program || !texture) return;

  // Evaluate expression for all cells using OCaml WASM
  const evaluationContext = {
    ...context,
    time,
    gridSize,
    frameCount,
  };

  // Call OCaml evaluator
  const result = await window.CELEvaluator?.evaluateGrid?.(
    activeExpression,
    evaluationContext,
    gridSize
  );

  if (!result?.success || !result.output) {
    console.error('[CanvasBlock] Expression evaluation failed:', result?.error);
    return;
  }

  // Upload texture to GPU (zero-copy!)
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texImage2D(
    gl.TEXTURE_2D,
    0,                          // Level
    gl.LUMINANCE,              // Internal format
    gridSize,                   // Width
    gridSize,                   // Height
    0,                          // Border
    gl.LUMINANCE,              // Format
    gl.UNSIGNED_BYTE,          // Type
    result.output               // Data (Uint8Array from WASM)
  );

  // Render
  gl.useProgram(program);

  // Bind position buffer
  const positionLocation = gl.getAttribLocation(program, 'a_position');
  gl.enableVertexAttribArray(positionLocation);
  gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
  gl.vertexAttribPointer(positionLocation, 2, gl.FLOAT, false, 0, 0);

  // Bind texture
  const textureLocation = gl.getUniformLocation(program, 'u_texture');
  gl.uniform1i(textureLocation, 0);

  // Draw
  gl.viewport(0, 0, width, height);
  gl.clearColor(0, 0, 0, 1);
  gl.clear(gl.COLOR_BUFFER_BIT);
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
}

// Animation loop
function animate(timestamp: number) {
  if (!autoplay) return;

  // Calculate FPS
  if (lastFrameTime > 0) {
    const delta = timestamp - lastFrameTime;
    fps = Math.round(1000 / delta);
  }
  lastFrameTime = timestamp;

  // Update time
  time += 1 / targetFps;
  frameCount++;

  // Render based on mode
  if (mode === 'pattern') {
    renderPattern();
  } else if (mode === 'chart') {
    renderChart();
  } else if (mode === 'interactive') {
    renderInteractive();
  } else if (mode === 'diagram') {
    renderDiagram();
  }

  // Dispatch onRender action
  if (block.onRender && onAction) {
    onAction(block.onRender, { time, frameCount, fps });
  }

  // Continue animation
  animationId = requestAnimationFrame(animate);
}

// Lifecycle
onMount(() => {
  if (mode === 'pattern' || mode === 'interactive') {
    const success = initWebGL();
    if (!success) {
      console.error('[CanvasBlock] WebGL initialization failed');
      return;
    }
  }

  // Initialize Canvas 2D for text overlays
  canvas2d = canvasElement.getContext('2d', { alpha: true });

  // Start animation
  if (autoplay) {
    animationId = requestAnimationFrame(animate);
  }
});

onDestroy(() => {
  // Stop animation
  if (animationId !== null) {
    cancelAnimationFrame(animationId);
  }

  // Clean up WebGL resources
  if (gl) {
    if (texture) gl.deleteTexture(texture);
    if (program) gl.deleteProgram(program);
    if (positionBuffer) gl.deleteBuffer(positionBuffer);
  }
});

// Placeholder for other modes
function renderChart() {
  console.log('[CanvasBlock] Chart mode not yet implemented');
}

function renderInteractive() {
  console.log('[CanvasBlock] Interactive mode not yet implemented');
}

function renderDiagram() {
  console.log('[CanvasBlock] Diagram mode not yet implemented');
}
</script>

<canvas
  bind:this={canvasElement}
  {width}
  {height}
  class={block.class}
  style={block.css}
/>

<style>
canvas {
  display: block;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
}
</style>
```

**Tasks:**
- [ ] Create CanvasBlock.svelte
- [ ] Implement WebGL setup
- [ ] Implement pattern mode renderer
- [ ] Test with expression: `sin(x * 0.1 + time)`
- [ ] Verify 60 FPS performance
- [ ] Test with canvas pattern examples

---

### Phase 3: Interactive Mode (3-4 hours)

**Add mouse/keyboard handling:**

```svelte
<!-- In CanvasBlock.svelte -->

let mouseX = $state(0);
let mouseY = $state(0);
let mouseDown = $state(false);
let keys = $state<Set<string>>(new Set());

function handleMouseMove(event: MouseEvent) {
  const rect = canvasElement.getBoundingClientRect();
  mouseX = ((event.clientX - rect.left) / rect.width) * 100;  // 0-100%
  mouseY = ((event.clientY - rect.top) / rect.height) * 100;  // 0-100%

  // Update context for expressions
  context = { ...context, mouseX, mouseY };

  // Dispatch action
  if (block.onMouseMove && onAction) {
    onAction(block.onMouseMove, { mouseX, mouseY });
  }
}

function handleMouseDown(event: MouseEvent) {
  mouseDown = true;

  if (block.onClick && onAction) {
    onAction(block.onClick, { mouseX, mouseY });
  }
}

function handleMouseUp() {
  mouseDown = false;
}

function handleKeyDown(event: KeyboardEvent) {
  keys.add(event.key);

  if (block.onKeyDown && onAction) {
    onAction(block.onKeyDown, { key: event.key, keys: Array.from(keys) });
  }
}

function handleKeyUp(event: KeyboardEvent) {
  keys.delete(event.key);
}

<!-- Add to canvas element -->
<canvas
  bind:this={canvasElement}
  {width}
  {height}
  onmousemove={handleMouseMove}
  onmousedown={handleMouseDown}
  onmouseup={handleMouseUp}
  onkeydown={handleKeyDown}
  onkeyup={handleKeyUp}
  tabindex="0"
  class={block.class}
  style={block.css}
/>
```

**Tasks:**
- [ ] Add mouse event handlers
- [ ] Add keyboard event handlers
- [ ] Update context with mouse position
- [ ] Dispatch actions
- [ ] Test interactive examples

---

### Phase 4: Chart Mode (4-6 hours)

**Add data visualization:**

```typescript
async function renderChart() {
  if (!canvas2d) return;

  const ctx = canvas2d;

  // Evaluate data expression
  const dataExpr = block.data || '[]';
  const data = evaluateCEL(dataExpr, context);

  const labelsExpr = block.labels || '[]';
  const labels = evaluateCEL(labelsExpr, context);

  // Clear canvas
  ctx.clearRect(0, 0, width, height);

  // Render based on chart type
  if (block.chartType === 'line') {
    renderLineChart(ctx, data, labels);
  } else if (block.chartType === 'bar') {
    renderBarChart(ctx, data, labels);
  } else if (block.chartType === 'scatter') {
    renderScatterPlot(ctx, data, labels);
  }
}

function renderLineChart(ctx: CanvasRenderingContext2D, data: any[], labels: any[]) {
  // Scale data to canvas
  const maxValue = Math.max(...data);
  const xStep = width / (data.length - 1);
  const yScale = (height - 40) / maxValue;

  // Draw axes
  ctx.strokeStyle = '#6b7280';
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(40, 20);
  ctx.lineTo(40, height - 20);
  ctx.lineTo(width - 20, height - 20);
  ctx.stroke();

  // Draw line
  ctx.strokeStyle = block.colors?.line || '#3b82f6';
  ctx.lineWidth = 3;
  ctx.beginPath();
  data.forEach((value, i) => {
    const x = 40 + i * xStep;
    const y = height - 20 - (value * yScale);
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.stroke();

  // Draw labels
  ctx.fillStyle = '#374151';
  ctx.font = '12px sans-serif';
  ctx.textAlign = 'center';
  labels.forEach((label, i) => {
    const x = 40 + i * xStep;
    ctx.fillText(String(label), x, height - 5);
  });
}
```

**Tasks:**
- [ ] Implement line chart renderer
- [ ] Implement bar chart renderer
- [ ] Implement scatter plot renderer
- [ ] Add axis labels and legends
- [ ] Test with Loro document data
- [ ] Verify reactive updates

---

### Phase 5: Diagram Mode (4-6 hours)

**Add flowchart/diagram rendering:**

```typescript
function renderDiagram() {
  if (!canvas2d) return;

  const ctx = canvas2d;
  ctx.clearRect(0, 0, width, height);

  // Render edges first (behind nodes)
  block.edges?.forEach(edge => {
    const fromNode = block.nodes?.find(n => n.id === edge.from);
    const toNode = block.nodes?.find(n => n.id === edge.to);

    if (fromNode && toNode) {
      drawArrow(ctx, fromNode, toNode, edge);
    }
  });

  // Render nodes
  block.nodes?.forEach(node => {
    // Check conditional visibility
    const shouldRender = node.when
      ? evaluateCEL(node.when, context)
      : true;

    if (shouldRender) {
      drawNode(ctx, node);
    }
  });
}

function drawNode(ctx: CanvasRenderingContext2D, node: any) {
  const x = node.x || 50;
  const y = node.y || 50;
  const nodeWidth = 120;
  const nodeHeight = 60;

  ctx.fillStyle = node.color || '#3b82f6';
  ctx.strokeStyle = '#1e40af';
  ctx.lineWidth = 2;

  // Draw shape
  if (node.shape === 'rect') {
    ctx.fillRect(x - nodeWidth/2, y - nodeHeight/2, nodeWidth, nodeHeight);
    ctx.strokeRect(x - nodeWidth/2, y - nodeHeight/2, nodeWidth, nodeHeight);
  } else if (node.shape === 'circle') {
    ctx.beginPath();
    ctx.arc(x, y, nodeWidth/2, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
  } else if (node.shape === 'diamond') {
    ctx.beginPath();
    ctx.moveTo(x, y - nodeHeight/2);
    ctx.lineTo(x + nodeWidth/2, y);
    ctx.lineTo(x, y + nodeHeight/2);
    ctx.lineTo(x - nodeWidth/2, y);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
  }

  // Draw label
  ctx.fillStyle = '#ffffff';
  ctx.font = '14px sans-serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(node.label, x, y);
}

function drawArrow(ctx: CanvasRenderingContext2D, from: any, to: any, edge: any) {
  const x1 = from.x || 50;
  const y1 = from.y || 50;
  const x2 = to.x || 150;
  const y2 = to.y || 50;

  ctx.strokeStyle = edge.color || '#6b7280';
  ctx.lineWidth = edge.width || 2;

  // Draw line
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x2, y2);
  ctx.stroke();

  // Draw arrow head
  const angle = Math.atan2(y2 - y1, x2 - x1);
  const headLen = 10;
  ctx.beginPath();
  ctx.moveTo(x2, y2);
  ctx.lineTo(
    x2 - headLen * Math.cos(angle - Math.PI / 6),
    y2 - headLen * Math.sin(angle - Math.PI / 6)
  );
  ctx.moveTo(x2, y2);
  ctx.lineTo(
    x2 - headLen * Math.cos(angle + Math.PI / 6),
    y2 - headLen * Math.sin(angle + Math.PI / 6)
  );
  ctx.stroke();

  // Draw label
  if (edge.label) {
    ctx.fillStyle = '#374151';
    ctx.font = '12px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(edge.label, (x1 + x2) / 2, (y1 + y2) / 2 - 10);
  }
}
```

**Tasks:**
- [ ] Implement node rendering (rect, circle, diamond)
- [ ] Implement edge rendering with arrows
- [ ] Add conditional node visibility
- [ ] Add label rendering
- [ ] Test flowchart examples
- [ ] Test state diagram examples

---

### Phase 6: Example Templates (2-3 hours)

**Create comprehensive examples:**

**File 1:** `/docs/examples/13_canvas_patterns.huml`
- Wave patterns
- Ripples
- Spirals
- Interactive mouse effects

**File 2:** `/docs/examples/14_canvas_charts.huml`
- Line chart from submissionsDoc
- Bar chart from contentDoc
- Scatter plot

**File 3:** `/docs/examples/15_canvas_game.huml`
- Brick breaker game
- Score tracking
- Leaderboard integration

**File 4:** `/docs/examples/16_canvas_diagrams.huml`
- Flowchart example
- State machine diagram
- Interactive node selection

**Tasks:**
- [ ] Create canvas_patterns.huml
- [ ] Create canvas_charts.huml
- [ ] Create canvas_game.huml
- [ ] Create canvas_diagrams.huml
- [ ] Test all examples
- [ ] Verify 60 FPS performance

---

### Phase 7: Documentation (2 hours)

**Update template guide:**

Add canvas block section to `HUML_TEMPLATE_GUIDE_ACCURATE.md`:
- Block properties
- Mode descriptions
- Expression examples
- Performance notes
- Common patterns

**Update implementation status:**
- Change canvas from stub to ✅ fully working
- Update block count (19 → 20)

**Tasks:**
- [ ] Document canvas block in template guide
- [ ] Add usage examples
- [ ] Update implementation status
- [ ] Add performance guidelines

---

## ⏱️ Time Estimate

| Phase | Task | Estimate |
|-------|------|----------|
| 1 | Type definition | 1 hour |
| 2 | Pattern mode + WebGL | 4-6 hours |
| 3 | Interactive mode | 3-4 hours |
| 4 | Chart mode | 4-6 hours |
| 5 | Diagram mode | 4-6 hours |
| 6 | Example templates | 2-3 hours |
| 7 | Documentation | 2 hours |
| **TOTAL** | | **20-28 hours** |

---

## ✅ Success Criteria

1. ✅ Pattern mode renders math expressions at 60 FPS
2. ✅ Expressions like `sin(x * 0.1 + time)` work
3. ✅ Interactive mode responds to mouse/keyboard
4. ✅ Chart mode displays data from Loro docs
5. ✅ Diagram mode renders flowcharts/state machines
6. ✅ All 4 modes working and tested
7. ✅ Example templates demonstrating each mode
8. ✅ Documentation complete
9. ✅ WebGL performance: <1ms GPU render time
10. ✅ OCaml performance: ~16ms for 10,000 expressions

---

## 🚀 Getting Started

**Prerequisites:**
1. ✅ Complete OCaml refactor first (see OCAML_REFACTOR_PLAN.md)
2. ✅ Math functions available (sin, cos, sqrt, atan2, etc.)
3. ✅ Geometry functions available (distance, lerp, clamp, etc.)

**Start implementation:**
```bash
# Navigate to project
cd sthalam/frontend/desktop

# Update types
code src/lib/types/huml.ts

# Implement CanvasBlock
code src/renderer/blocks/CanvasBlock.svelte

# Test
npm run dev
# Load canvas_patterns.huml example
```

---

## 📚 References

- Canvas spec: `/docs/specs/CANVAS_BLOCK_SPEC.md`
- OCaml refactor: `/docs/specs/OCAML_REFACTOR_PLAN.md`
- Existing blocks: `/src/renderer/blocks/`
- CEL evaluator: `/src/lib/services/celEvaluator.ts`
- WASM knowledge: `/docs/WASM_TAURI_KNOWLEDGE.md`

---

**Status:** Ready to implement after OCaml refactor! 🎨
