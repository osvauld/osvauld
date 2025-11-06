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

// Store WASM function references outside reactive context
let evaluateGridFn: ((expr: string, gridSize: number, time: number) => any) | null = null;
let compileToGLSLFn: ((expr: string, gridSize: number) => any) | null = null;

// GPU shader mode state
let gpuMode = false;
let uniformLocations: Map<string, WebGLUniformLocation> = new Map();
let shaderUniforms: Array<{name: string, celVar: string, glslType: string}> = [];

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

// Pattern for GPU mode: strips {{ }} to get raw identifiers
// Pattern for CPU mode: interpolates {{ }} with actual values
const patternExprGPU = $derived.by(() => {
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

  // Strip {{ }} markers to get raw identifiers for GPU uniforms
  // Example: "sin(x * {{speed}})" becomes "sin(x * speed)"
  return block.pattern.replace(/\{\{([^}]+)\}\}/g, (_, varName) => varName.trim());
});

const patternExprCPU = $derived.by(() => {
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

  // Interpolate any {{ }} template variables with context values
  // This allows patterns like "sin(x * {{speed}})" to work
  try {
    return interpolateCEL(block.pattern, context);
  } catch (error) {
    console.error('[CanvasBlock] Failed to interpolate pattern:', error);
    return null;
  }
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

// Initialize WebGL for CPU pattern mode (texture upload)
function initWebGLCPU(): boolean {
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

// Initialize WebGL for GPU shader mode (dynamic shader)
function initWebGLGPU(): boolean {
  if (!patternExprGPU || !compileToGLSLFn) {
    console.error('[CanvasBlock] GPU mode requires pattern and compileToGLSL');
    return false;
  }

  gl = canvasElement.getContext('webgl2');
  if (!gl) {
    console.error('[CanvasBlock] WebGL2 not supported');
    return false;
  }

  try {
    // Compile CEL expression to GLSL shader (with stripped {{ }} markers)
    const compilation = compileToGLSLFn(patternExprGPU, gridSize);

    if (!compilation.success) {
      console.error('[CanvasBlock] GLSL compilation failed:', compilation.error);
      console.log('[CanvasBlock] Falling back to CPU mode');
      return false;
    }

    console.log('[CanvasBlock] GPU shader compiled successfully');
    console.log('[CanvasBlock] GLSL expression:', compilation.glslExpr);
    console.log('[CanvasBlock] Uniforms:', compilation.uniforms);

    // Store uniforms for later updates
    shaderUniforms = compilation.uniforms;

    // Create vertex shader
    const vs = gl.createShader(gl.VERTEX_SHADER)!;
    gl.shaderSource(vs, VERTEX_SHADER);
    gl.compileShader(vs);

    if (!gl.getShaderParameter(vs, gl.COMPILE_STATUS)) {
      console.error('[CanvasBlock] Vertex shader error:', gl.getShaderInfoLog(vs));
      return false;
    }

    // Create dynamic fragment shader from OCaml
    const fs = gl.createShader(gl.FRAGMENT_SHADER)!;
    gl.shaderSource(fs, compilation.shaderCode);
    gl.compileShader(fs);

    if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
      console.error('[CanvasBlock] Fragment shader error:', gl.getShaderInfoLog(fs));
      console.error('[CanvasBlock] Shader code:', compilation.shaderCode);
      return false;
    }

    // Create program
    program = gl.createProgram()!;
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);

    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      console.error('[CanvasBlock] Program link error:', gl.getProgramInfoLog(program));
      return false;
    }

    // Create quad buffer
    positionBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, 1,1]), gl.STATIC_DRAW);

    // Get uniform locations
    uniformLocations.clear();
    for (const uniform of shaderUniforms) {
      const location = gl.getUniformLocation(program, uniform.name);
      if (location) {
        uniformLocations.set(uniform.celVar, location);
      }
    }

    gpuMode = true;
    return true;
  } catch (error) {
    console.error('[CanvasBlock] GPU initialization error:', error);
    return false;
  }
}

// Render pattern using OCaml WASM (CPU mode)
async function renderPatternCPU() {
  if (!gl || !program || !texture || !patternExprCPU || !evaluateGridFn) return;

  try {
    // Absolutely guarantee plain JS values by JSON round-trip
    const params = JSON.parse(JSON.stringify({
      pattern: String(patternExprCPU),
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

// Render pattern using GPU shader (GPU mode)
function renderPatternGPU() {
  if (!gl || !program || !positionBuffer) return;

  try {
    // Bind shader program FIRST (required before setting uniforms)
    gl.useProgram(program);

    // Update uniforms from context
    for (const uniform of shaderUniforms) {
      const location = uniformLocations.get(uniform.celVar);
      if (!location) continue;

      // Get value from context or built-in variables
      let value: number;
      switch (uniform.celVar) {
        case 'time':
          value = time;
          break;
        case 'gridSize':
          value = gridSize;
          break;
        case 'mouseX':
          value = mouseX;
          break;
        case 'mouseY':
          value = mouseY;
          break;
        default:
          // Try to get from context
          value = context[uniform.celVar] ?? 0;
          // Debug log for custom uniforms
          if (frameCount % 60 === 0) {
            console.log(`[CanvasBlock] Uniform ${uniform.celVar} = ${value} from context`, context);
          }
      }

      // Set uniform value
      gl.uniform1f(location, value);
    }

    // Setup and render quad
    const posLoc = gl.getAttribLocation(program, 'a_position');
    gl.enableVertexAttribArray(posLoc);
    gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
    gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);
    gl.viewport(0, 0, width, height);
    gl.clearColor(0, 0, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  } catch (error) {
    console.error('[CanvasBlock] GPU render error:', error);
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

  // Render based on mode
  if (block.pattern) {
    if (gpuMode) {
      renderPatternGPU();
    } else {
      renderPatternCPU();
    }
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
  // Capture WASM function references outside reactive context
  if ((window as any).CELEvaluator?.evaluateGrid) {
    evaluateGridFn = (window as any).CELEvaluator.evaluateGrid.bind((window as any).CELEvaluator);
  }
  if ((window as any).CELEvaluator?.compileToGLSL) {
    compileToGLSLFn = (window as any).CELEvaluator.compileToGLSL.bind((window as any).CELEvaluator);
  }

  // Initialize rendering context based on mode
  if (block.pattern) {
    // Try GPU mode if requested
    if (block.renderMode === 'gpu' && compileToGLSLFn) {
      const gpuSuccess = initWebGLGPU();
      if (!gpuSuccess) {
        // GPU failed, fall back to CPU
        console.log('[CanvasBlock] Falling back to CPU mode');
        gpuMode = false;
        initWebGLCPU();
      }
    } else {
      // CPU mode
      gpuMode = false;
      initWebGLCPU();
    }
  } else if (entities) {
    // Entity rendering uses 2D canvas
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
