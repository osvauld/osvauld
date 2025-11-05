<script lang="ts">
/**
 * CanvasBlock - Generic canvas for patterns, games, visualizations
 * HUML controls rendering via state + CEL expressions
 */

import type { CanvasBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { evaluateCEL, interpolateCEL } from '../../lib/services/celEvaluator';
import { onMount, onDestroy, untrack } from 'svelte';

interface Props {
  block: CanvasBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction }: Props = $props();

// Canvas elements
let canvasElement: HTMLCanvasElement;
let gl: WebGL2RenderingContext | null = null;
let ctx2d: CanvasRenderingContext2D | null = null;

// Store WASM function reference outside reactive context
let evaluateGridFn: ((expr: string, gridSize: number, time: number) => any) | null = null;

// Animation state
let animationId: number | null = null;
let time = $state(0);
let frameCount = $state(0);
let fps = $state(0);
let lastFrameTime = 0;
let startTime = 0;

// Mouse state for events
let mouseX = $state(0);
let mouseY = $state(0);

// WebGL resources (for pattern mode)
let texture: WebGLTexture | null = null;
let program: WebGLProgram | null = null;
let positionBuffer: WebGLBuffer | null = null;

// Configuration with defaults
const width = $derived(block.width || 600);
const height = $derived(block.height || 400);
const gridSize = $derived(block.gridSize || 100);
const autoplay = $derived(block.autoplay !== false);
const targetFps = $derived(block.fps || 60);

// Pattern can be either:
// 1. A literal string: "sin(x * 0.1 + time)"
// 2. A CEL expression: "${ patternFromState }" that evaluates to a string
const patternExpr = $derived.by(() => {
  if (!block.pattern) return null;

  // If it's a CEL expression (${ }), evaluate it to get the actual pattern string
  if (block.pattern.startsWith('${') && block.pattern.endsWith('}')) {
    try {
      return evaluateCEL(block.pattern, context);
    } catch (error) {
      console.error('[CanvasBlock] Failed to evaluate pattern:', error);
      return null;
    }
  }

  // Otherwise it's a literal pattern expression string
  return block.pattern;
});

const entities = $derived.by(() => {
  if (!block.entities) return null;
  try {
    return evaluateCEL(block.entities, context);
  } catch (error) {
    console.error('[CanvasBlock] Failed to evaluate entities:', error);
    return null;
  }
});

// Styles
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);

// WebGL Shaders for pattern mode
const VERTEX_SHADER = `#version 300 es
  in vec2 a_position;
  out vec2 v_texCoord;
  void main() {
    gl_Position = vec4(a_position, 0.0, 1.0);
    v_texCoord = a_position * 0.5 + 0.5;
  }
`;

const FRAGMENT_SHADER = `#version 300 es
  precision highp float;
  in vec2 v_texCoord;
  out vec4 outColor;
  uniform sampler2D u_texture;
  void main() {
    float value = texture(u_texture, v_texCoord).r;
    vec3 color = vec3(value);
    outColor = vec4(color, 1.0);
  }
`;

// Initialize WebGL for pattern mode
function initWebGL(): boolean {
  gl = canvasElement.getContext('webgl2');
  if (!gl) {
    console.error('[CanvasBlock] WebGL2 not supported');
    return false;
  }

  // Create shaders
  const vs = gl.createShader(gl.VERTEX_SHADER)!;
  gl.shaderSource(vs, VERTEX_SHADER);
  gl.compileShader(vs);

  const fs = gl.createShader(gl.FRAGMENT_SHADER)!;
  gl.shaderSource(fs, FRAGMENT_SHADER);
  gl.compileShader(fs);

  // Create program
  program = gl.createProgram()!;
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);

  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.error('[CanvasBlock] WebGL program error:', gl.getProgramInfoLog(program));
    return false;
  }

  // Create quad
  positionBuffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, 1,1]), gl.STATIC_DRAW);

  // Create texture
  texture = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);

  return true;
}

// Render pattern using OCaml WASM
async function renderPattern() {
  if (!gl || !program || !texture || !patternExpr || !evaluateGridFn) return;

  try {
    // Absolutely guarantee plain JS values by JSON round-trip
    const params = JSON.parse(JSON.stringify({
      pattern: String(patternExpr),
      size: Number(gridSize),
      currentTime: Number(time)
    }));

    // Call OCaml evaluator using stored function reference
    const result = await evaluateGridFn(params.pattern, params.size, params.currentTime);

    if (!result?.success || !result.output) {
      console.error('[CanvasBlock] Pattern evaluation failed:', result?.error);
      return;
    }

    // Upload to GPU
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.LUMINANCE, gridSize, gridSize, 0, gl.LUMINANCE, gl.UNSIGNED_BYTE, result.output);

    // Render
    gl.useProgram(program);
    const posLoc = gl.getAttribLocation(program, 'a_position');
    gl.enableVertexAttribArray(posLoc);
    gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
    gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);
    gl.uniform1i(gl.getUniformLocation(program, 'u_texture'), 0);
    gl.viewport(0, 0, width, height);
    gl.clearColor(0, 0, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  } catch (error) {
    console.error('[CanvasBlock] Render error:', error);
  }
}

// Render entities (generic shape rendering)
function renderEntities() {
  if (!ctx2d || !entities || !Array.isArray(entities)) return;

  ctx2d.clearRect(0, 0, width, height);

  for (const entity of entities) {
    if (!entity || typeof entity !== 'object') continue;

    const { type, x = 0, y = 0, width: w = 50, height: h = 50, radius = 25, color = '#ffffff', text } = entity;

    ctx2d.fillStyle = color;
    ctx2d.strokeStyle = color;

    switch (type) {
      case 'rect':
        ctx2d.fillRect(x, y, w, h);
        break;
      case 'circle':
        ctx2d.beginPath();
        ctx2d.arc(x, y, radius, 0, Math.PI * 2);
        ctx2d.fill();
        break;
      case 'text':
        ctx2d.font = '16px sans-serif';
        ctx2d.fillText(text || '', x, y);
        break;
    }
  }
}

// Animation loop
function animate(timestamp: number) {
  if (!autoplay) return;

  if (startTime === 0) startTime = timestamp;

  // FPS calculation
  if (lastFrameTime > 0) {
    const delta = timestamp - lastFrameTime;
    fps = Math.round(1000 / delta);
  }
  lastFrameTime = timestamp;

  // Update time
  time = (timestamp - startTime) / 1000;
  frameCount++;

  // Render based on what's provided
  if (patternExpr) {
    renderPattern();
  } else if (entities) {
    renderEntities();
  }

  // Call onRender action with current state
  if (block.onRender && onAction) {
    onAction(block.onRender, { time, frameCount, fps, mouseX, mouseY });
  }

  animationId = requestAnimationFrame(animate);
}

// Event handlers
function handleMouseMove(event: MouseEvent) {
  const rect = canvasElement.getBoundingClientRect();
  mouseX = event.clientX - rect.left;
  mouseY = event.clientY - rect.top;

  if (block.onMouseMove && onAction) {
    onAction(block.onMouseMove, { mouseX, mouseY });
  }
}

function handleClick(event: MouseEvent) {
  const rect = canvasElement.getBoundingClientRect();
  const x = event.clientX - rect.left;
  const y = event.clientY - rect.top;

  if (block.onClick && onAction) {
    onAction(block.onClick, { x, y });
  }
}

function handleKeyDown(event: KeyboardEvent) {
  if (block.onKeyDown && onAction) {
    onAction(block.onKeyDown, { key: event.key });
  }
}

function handleKeyUp(event: KeyboardEvent) {
  if (block.onKeyUp && onAction) {
    onAction(block.onKeyUp, { key: event.key });
  }
}

// Lifecycle
onMount(() => {
  // Capture WASM function reference outside reactive context
  if ((window as any).CELEvaluator?.evaluateGrid) {
    evaluateGridFn = (window as any).CELEvaluator.evaluateGrid.bind((window as any).CELEvaluator);
  }

  // Initialize rendering context
  if (patternExpr) {
    initWebGL();
  } else {
    ctx2d = canvasElement.getContext('2d');
  }

  // Start animation
  if (autoplay) {
    animationId = requestAnimationFrame(animate);
  }
});

onDestroy(() => {
  if (animationId !== null) {
    cancelAnimationFrame(animationId);
  }

  // Cleanup WebGL
  if (gl) {
    if (texture) gl.deleteTexture(texture);
    if (program) gl.deleteProgram(program);
    if (positionBuffer) gl.deleteBuffer(positionBuffer);
  }
});
</script>

<svelte:window onkeydown={handleKeyDown} onkeyup={handleKeyUp} />

<div class="canvas-container" style:width="{width}px">
  <canvas
    bind:this={canvasElement}
    {width}
    {height}
    class={block.class}
    style={styles}
    onmousemove={handleMouseMove}
    onclick={handleClick}
    tabindex="0"
  ></canvas>

  {#if import.meta.env.DEV}
    <div class="canvas-debug">
      <small>
        FPS: {fps} | Frame: {frameCount} | Time: {time.toFixed(2)}s | Mouse: ({mouseX.toFixed(0)}, {mouseY.toFixed(0)})
      </small>
    </div>
  {/if}
</div>

<style>
.canvas-container {
  display: inline-block;
}

canvas {
  display: block;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  background: #000;
}

canvas:focus {
  outline: 2px solid #3b82f6;
  outline-offset: 2px;
}

.canvas-debug {
  margin-top: 0.5rem;
  padding: 0.5rem;
  background: #f3f4f6;
  border-radius: 4px;
  font-family: monospace;
  font-size: 0.75rem;
  color: #6b7280;
}
</style>
