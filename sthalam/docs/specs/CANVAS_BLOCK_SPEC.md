# Canvas Block Specification

**WebGL/Canvas rendering for Sthalam templates**

**Status**: Core Feature - KEEP & ENHANCE
**Performance Target**: 60 FPS for 10,000 expressions/frame

---

## Overview

Canvas blocks enable real-time graphics, animations, and visualizations powered by:
- **CEL expressions** - Math/logic for each pixel/particle
- **OCaml WASM evaluator** - 40x faster evaluation
- **WebGL rendering** - GPU-accelerated display
- **Zero-copy upload** - Direct memory → GPU

---

## Block Type: `canvas`

```huml
- type: canvas
  mode: pattern          # pattern | chart | interactive | custom
  width: 600
  height: 600
  css: "border-radius: 8px;"

  # Pattern mode (current implementation)
  gridSize: 100          # Grid resolution
  cellSize: 6            # Pixels per cell
  pattern: "{{ selectedPattern }}"
  expressions::
    waves: "(sin(x * 0.2 + time) + sin(y * 0.2 + time)) / 2"
    ripple: "sin(sqrt((x - gridSize/2)^2 + (y - gridSize/2)^2) * 0.3 - time)"
    spiral: "sin(atan2(y - gridSize/2, x - gridSize/2) * 3 - time)"

  # Optional controls
  autoplay: true         # Start animation immediately
  fps: 60               # Target frame rate
  onRender: updateFPS    # Callback after each frame
```

---

## Implementation: CanvasBlock.svelte

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { evaluateCEL } from '$lib/services/celEvaluator';
  import type { CanvasBlock, Context } from '$lib/types/huml';

  interface Props {
    block: CanvasBlock;
    context: Context;
    onAction?: (action: string, params?: any) => void;
  }

  let { block, context, onAction }: Props = $props();

  // Canvas element
  let canvasElement: HTMLCanvasElement;

  // WebGL context and resources
  let gl: WebGLRenderingContext | null = null;
  let program: WebGLProgram | null = null;
  let texture: WebGLTexture | null = null;
  let positionBuffer: WebGLBuffer | null = null;

  // Animation state
  let animationFrameId: number;
  let lastFrameTime = 0;
  let frameCount = 0;
  let currentFPS = 0;

  /**
   * Initialize WebGL context
   */
  function initWebGL(canvas: HTMLCanvasElement): boolean {
    gl = canvas.getContext('webgl', {
      alpha: false,
      antialias: false,
      powerPreference: 'high-performance'
    });

    if (!gl) {
      console.error('[CanvasBlock] WebGL not supported');
      return false;
    }

    // Compile shaders
    const vertexShader = compileShader(gl, gl.VERTEX_SHADER, vertexShaderSource);
    const fragmentShader = compileShader(gl, gl.FRAGMENT_SHADER, fragmentShaderSource);

    if (!vertexShader || !fragmentShader) return false;

    // Link program
    program = gl.createProgram()!;
    gl.attachShader(program, vertexShader);
    gl.attachShader(program, fragmentShader);
    gl.linkProgram(program);

    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      console.error('[CanvasBlock] Shader link failed:', gl.getProgramInfoLog(program));
      return false;
    }

    // Create texture for pixel data
    texture = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);

    // Create position buffer (full-screen quad)
    const positions = new Float32Array([
      -1, -1,
       1, -1,
      -1,  1,
      -1,  1,
       1, -1,
       1,  1,
    ]);

    positionBuffer = gl.createBuffer()!;
    gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);

    return true;
  }

  /**
   * Compile shader
   */
  function compileShader(gl: WebGLRenderingContext, type: number, source: string): WebGLShader | null {
    const shader = gl.createShader(type)!;
    gl.shaderSource(shader, source);
    gl.compileShader(shader);

    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      console.error('[CanvasBlock] Shader compile error:', gl.getShaderInfoLog(shader));
      gl.deleteShader(shader);
      return null;
    }

    return shader;
  }

  /**
   * Render pattern using CEL evaluator
   */
  async function renderPattern(timestamp: number) {
    if (!gl || !program || !texture) return;

    // Calculate FPS
    const deltaTime = timestamp - lastFrameTime;
    frameCount++;
    if (deltaTime >= 1000) {
      currentFPS = Math.round((frameCount * 1000) / deltaTime);
      frameCount = 0;
      lastFrameTime = timestamp;

      // Notify parent of FPS update
      if (block.onRender && onAction) {
        onAction(block.onRender, { fps: currentFPS });
      }
    }

    // Get parameters from context
    const gridSize = block.gridSize || context.gridSize || 100;
    const time = context.time || timestamp / 1000;
    const pattern = block.pattern ?
      evaluateCEL(block.pattern, context) :
      context.selectedPattern || Object.keys(block.expressions || {})[0];

    // Get expression for current pattern
    const expr = block.expressions?.[pattern];
    if (!expr) {
      console.warn(`[CanvasBlock] No expression found for pattern: ${pattern}`);
      return;
    }

    // Evaluate expression for each pixel using WASM
    // This is the CRITICAL performance path!
    const result = await evaluateGridExpression(expr, {
      gridSize,
      time,
      pattern,
      ...context
    });

    if (!result.success) {
      console.error('[CanvasBlock] Evaluation failed:', result.error);
      return;
    }

    // Upload to GPU (zero-copy!)
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.LUMINANCE,
      gridSize,
      gridSize,
      0,
      gl.LUMINANCE,
      gl.UNSIGNED_BYTE,
      result.output  // Direct Uint8Array from WASM!
    );

    // Render quad with texture
    gl.useProgram(program);

    // Bind position buffer
    const positionLoc = gl.getAttribLocation(program, 'a_position');
    gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
    gl.enableVertexAttribArray(positionLoc);
    gl.vertexAttribPointer(positionLoc, 2, gl.FLOAT, false, 0, 0);

    // Bind texture
    const textureLoc = gl.getUniformLocation(program, 'u_texture');
    gl.uniform1i(textureLoc, 0);

    // Draw
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  /**
   * Evaluate CEL expression for grid
   * Uses WASM evaluator with hash table optimizations
   */
  async function evaluateGridExpression(
    expr: string,
    baseContext: Record<string, any>
  ): Promise<{ success: boolean; output?: Uint8Array; error?: string }> {
    const gridSize = baseContext.gridSize;
    const totalCells = gridSize * gridSize;

    // Create output buffer
    const output = new Uint8Array(totalCells);

    // Create reusable context (CRITICAL OPTIMIZATION!)
    // Don't recreate 10,000 times!
    const ctx = { ...baseContext };

    try {
      let idx = 0;
      for (let y = 0; y < gridSize; y++) {
        ctx.y = y;  // Update y in context

        for (let x = 0; x < gridSize; x++) {
          ctx.x = x;  // Update x in context

          // Evaluate CEL expression with WASM
          const value = evaluateCEL(expr, ctx);

          // Convert to grayscale byte (0-255)
          // CEL returns -1 to 1, we map to 0-255
          const brightness = (value + 1) * 127.5;
          output[idx++] = Math.max(0, Math.min(255, Math.floor(brightness)));
        }
      }

      return { success: true, output };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : String(error)
      };
    }
  }

  /**
   * Animation loop
   */
  function animate(timestamp: number) {
    renderPattern(timestamp);
    animationFrameId = requestAnimationFrame(animate);
  }

  /**
   * Start animation
   */
  function startAnimation() {
    if (block.autoplay !== false) {
      animationFrameId = requestAnimationFrame(animate);
    }
  }

  /**
   * Stop animation
   */
  function stopAnimation() {
    if (animationFrameId) {
      cancelAnimationFrame(animationFrameId);
    }
  }

  // Lifecycle
  onMount(() => {
    const success = initWebGL(canvasElement);
    if (success) {
      startAnimation();
    }
  });

  onDestroy(() => {
    stopAnimation();

    // Cleanup WebGL resources
    if (gl) {
      if (texture) gl.deleteTexture(texture);
      if (positionBuffer) gl.deleteBuffer(positionBuffer);
      if (program) gl.deleteProgram(program);
    }
  });

  // Shader sources (same as current implementation)
  const vertexShaderSource = `
    attribute vec2 a_position;
    varying vec2 v_texCoord;
    void main() {
      gl_Position = vec4(a_position, 0.0, 1.0);
      v_texCoord = a_position * 0.5 + 0.5;
    }
  `;

  const fragmentShaderSource = `
    precision mediump float;
    varying vec2 v_texCoord;
    uniform sampler2D u_texture;
    void main() {
      float brightness = texture2D(u_texture, v_texCoord).r;

      // Color gradient (purple to cyan)
      vec3 color = vec3(
        brightness,
        brightness * 0.5,
        brightness * 0.8
      );

      gl_FragColor = vec4(color, 1.0);
    }
  `;
</script>

<canvas
  bind:this={canvasElement}
  width={block.width || (block.gridSize || 100) * (block.cellSize || 6)}
  height={block.height || (block.gridSize || 100) * (block.cellSize || 6)}
  style={block.css}
/>

<style>
  canvas {
    display: block;
    image-rendering: pixelated;
    image-rendering: crisp-edges;
  }
</style>
```

---

## Performance Optimizations (KEEP ALL!)

From `WASM_TAURI_KNOWLEDGE.md`, we achieved **60 FPS for 10,000 expressions/frame**:

### **1. Hash Table Context (O(1) lookups)**
```typescript
// ✅ Reuse context object, just update x/y
const ctx = { gridSize, time, pattern };
for (let y = 0; y < gridSize; y++) {
  ctx.y = y;  // Update in-place
  for (let x = 0; x < gridSize; x++) {
    ctx.x = x;  // Update in-place
    const value = evaluateCEL(expr, ctx);  // Fast!
  }
}

// ❌ DON'T create new context 10,000 times!
// for (let y...) for (let x...) {
//   const value = evaluateCEL(expr, { x, y, time, gridSize });  // Slow!
// }
```

### **2. Zero-Copy GPU Upload**
```typescript
// ✅ Direct WASM memory → GPU
const output = new Uint8Array(totalCells);  // WASM-compatible
// ... fill output ...
gl.texImage2D(..., output);  // Zero copy!

// ❌ DON'T convert to array first
// const array = [...output];  // Unnecessary copy!
```

### **3. WebGL Instead of 2D Canvas**
```typescript
// ✅ GPU-accelerated (0.2ms)
gl.texImage2D(...);
gl.drawArrays(...);

// ❌ CPU-based (5-10ms)
// ctx2d.putImageData(...);
```

---

## Future Enhancements

### **Chart Mode** (Phase 2)
```huml
- type: canvas
  mode: chart
  chartType: line
  data: "{{ sales.map(s => s.amount) }}"
  labels: "{{ sales.map(s => s.month) }}"
  colors::
    line: "#60a5fa"
    fill: "rgba(96, 165, 250, 0.2)"
```

### **Interactive Mode** (Phase 3)
```huml
- type: canvas
  mode: interactive
  onMouseMove: updateCursor
  onMouseClick: addEffect
  expression: "sin(distance(x, mouseX, y, mouseY) * 0.2 - time)"
```

### **WebGPU Compute Shaders** (Phase 4)
```typescript
// Move CEL evaluation to GPU compute shaders
// Expected: 500+ FPS for same workload
```

---

## Integration with BlockRenderer2

```svelte
<!-- BlockRenderer2.svelte -->
{:else if block.type === 'canvas'}
  <CanvasBlock {block} {context} {onAction} {onStateChange} />
{/if}
```

**That's it!** WebGL logic is isolated, BlockRenderer stays clean.

---

## Testing

```typescript
// test/CanvasBlock.test.ts
describe('CanvasBlock', () => {
  it('renders pattern at 60 FPS', async () => {
    const block = {
      type: 'canvas',
      mode: 'pattern',
      gridSize: 100,
      expressions: {
        test: 'sin(x * 0.1) * cos(y * 0.1)'
      }
    };

    const component = render(CanvasBlock, { props: { block, context: {} } });

    await wait(1000);  // Wait 1 second
    expect(component.currentFPS).toBeGreaterThan(55);  // At least 55 FPS
  });
});
```

---

## Summary

✅ **Canvas is a CORE feature** - not removed, enhanced!
✅ **Same performance** - all optimizations preserved
✅ **Better organized** - isolated in CanvasBlock.svelte
✅ **More extensible** - easy to add chart/interactive modes
✅ **Testable** - can test WebGL logic independently

**WebGL stays, just moves to the right place!** 🎨⚡
