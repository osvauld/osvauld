<script lang="ts">
  import { evaluateExpression } from '../../lib/humlEvaluator';
  import { onMount } from 'svelte';

  interface Props {
    block: any;
    context: Record<string, any>;
    onAction?: (action: string, params?: any) => void;
    onStateChange?: (key: string, value: any) => void;
  }

  let { block, context, onAction, onStateChange }: Props = $props();

  let canvasElements = new Map<string, HTMLCanvasElement>();
  let canvasAnimations = new Map<string, number>();
  let webglContexts = new Map<string, {
    gl: WebGLRenderingContext,
    program: WebGLProgram,
    texture: WebGLTexture,
    positionBuffer: WebGLBuffer
  }>();

  /**
   * Initialize WebGL context
   */
  function initWebGL(canvas: HTMLCanvasElement): WebGLRenderingContext | null {
    const gl = canvas.getContext('webgl', { alpha: false, antialias: false });
    if (!gl) {
      console.error('[WebGL] Failed to get WebGL context');
      return null;
    }
    return gl;
  }

  /**
   * Render canvas with WebGL
   */
  function renderCanvas(canvas: HTMLCanvasElement, blockId: string) {
    const gridSize = context.gridSize || 40;
    const time = context.time || 0;
    const pattern = context.selectedPattern || 'waves';

    const cellSize = 6;
    const width = gridSize * cellSize;
    const height = gridSize * cellSize;

    canvas.width = width;
    canvas.height = height;

    // Get or create WebGL context
    let glContext = webglContexts.get(blockId);
    if (!glContext) {
      const gl = initWebGL(canvas);
      if (!gl) return;

      // Compile and link shaders
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
          vec3 color = vec3(brightness, brightness * 0.5, brightness * 0.8);
          gl_FragColor = vec4(color, 1.0);
        }
      `;

      const vertexShader = gl.createShader(gl.VERTEX_SHADER)!;
      gl.shaderSource(vertexShader, vertexShaderSource);
      gl.compileShader(vertexShader);

      const fragmentShader = gl.createShader(gl.FRAGMENT_SHADER)!;
      gl.shaderSource(fragmentShader, fragmentShaderSource);
      gl.compileShader(fragmentShader);

      const program = gl.createProgram()!;
      gl.attachShader(program, vertexShader);
      gl.attachShader(program, fragmentShader);
      gl.linkProgram(program);

      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        console.error('[WebGL] Shader link failed:', gl.getProgramInfoLog(program));
        return;
      }

      // Create texture
      const texture = gl.createTexture()!;
      gl.bindTexture(gl.TEXTURE_2D, texture);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);

      // Create fullscreen quad
      const positionBuffer = gl.createBuffer()!;
      gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
      const positions = new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]);
      gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);

      glContext = { gl, program, texture, positionBuffer };
      webglContexts.set(blockId, glContext);
    }

    const { gl, program, texture, positionBuffer } = glContext;

    // Get expression from state
    const exprKey = pattern + 'Expr';
    const expr = context[exprKey];

    if (typeof humlEval !== 'undefined' && humlEval.evaluateGrid && expr) {
      try {
        // OCaml computes all cells and returns binary color data
        const result = humlEval.evaluateGrid(expr, String(gridSize), String(time));

        if (result.success) {
          // Upload to GPU texture - use data directly without conversion
          gl.bindTexture(gl.TEXTURE_2D, texture);

          if (typeof result.output === 'string') {
            // Hex string format: Convert to Uint8Array (avoid ClampedArray for speed)
            const hexData = result.output;
            const byteCount = hexData.length / 2;
            const bytes = new Uint8Array(byteCount);

            // Fast hex parsing
            for (let i = 0; i < byteCount; i++) {
              const hex = hexData.substring(i * 2, i * 2 + 2);
              bytes[i] = (hex.charCodeAt(0) - (hex.charCodeAt(0) < 58 ? 48 : 87)) * 16 +
                         (hex.charCodeAt(1) - (hex.charCodeAt(1) < 58 ? 48 : 87));
            }

            gl.texImage2D(
              gl.TEXTURE_2D,
              0,
              gl.LUMINANCE,
              gridSize,
              gridSize,
              0,
              gl.LUMINANCE,
              gl.UNSIGNED_BYTE,
              bytes
            );
          } else {
            // WASM format: Use Uint8Array directly - zero copy!
            gl.texImage2D(
              gl.TEXTURE_2D,
              0,
              gl.LUMINANCE,
              gridSize,
              gridSize,
              0,
              gl.LUMINANCE,
              gl.UNSIGNED_BYTE,
              result.output
            );
          }

          // Render fullscreen quad with texture
          gl.useProgram(program);
          gl.viewport(0, 0, width, height);

          // Set up position attribute
          const positionLocation = gl.getAttribLocation(program, 'a_position');
          gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
          gl.enableVertexAttribArray(positionLocation);
          gl.vertexAttribPointer(positionLocation, 2, gl.FLOAT, false, 0, 0);

          // Set texture uniform
          const textureLocation = gl.getUniformLocation(program, 'u_texture');
          gl.uniform1i(textureLocation, 0);

          // Draw
          gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
        } else {
          console.error('[WebGL] OCaml evaluation failed:', result.error);
        }
      } catch (err) {
        console.error('[WebGL] Exception:', err);
      }
    }
  }

  /**
   * Start canvas animation
   */
  function startCanvasAnimation(blockId: string) {
    const canvas = canvasElements.get(blockId);
    if (!canvas) return;

    function animate() {
      renderCanvas(canvas, blockId);
      const animId = requestAnimationFrame(animate);
      canvasAnimations.set(blockId, animId);
    }
    animate();
  }

  /**
   * Svelte action to initialize canvas
   */
  function initCanvas(node: HTMLCanvasElement, blockId: string) {
    canvasElements.set(blockId, node);
    startCanvasAnimation(blockId);

    return {
      destroy() {
        const animId = canvasAnimations.get(blockId);
        if (animId) cancelAnimationFrame(animId);
        canvasAnimations.delete(blockId);
        canvasElements.delete(blockId);
      }
    };
  }

  /**
   * Evaluate a value that might contain CEL expressions
   */
  function evaluateValue(value: any, localContext?: Record<string, any>): any {
    if (typeof value !== 'string') return value;

    const fullContext = { ...context, ...localContext };

    // Check if it's a pure expression: {{ expression }}
    const trimmed = value.trim();
    if (trimmed.startsWith('{{') && trimmed.endsWith('}}')) {
      const expression = trimmed.slice(2, -2).trim();
      try {
        return evaluateExpression(expression, fullContext);
      } catch (error) {
        console.error('❌ [BlockRenderer] Failed to evaluate:', expression, error);
        return value;
      }
    }

    // Check if it contains embedded expressions: "text {{ expr }} more text"
    if (value.includes('{{')) {
      try {
        return value.replace(/\{\{(.+?)\}\}/g, (match, expression) => {
          const result = evaluateExpression(expression.trim(), fullContext);
          if (result === null || result === undefined) {
            return match;
          }
          return String(result);
        });
      } catch (error) {
        console.error('❌ [BlockRenderer] Failed to interpolate:', value, error);
        return value;
      }
    }

    return value;
  }

  /**
   * Check if block should be visible
   */
  function isBlockVisible(block: any): boolean {
    if (block.visible === undefined) return true;
    if (typeof block.visible === 'boolean') return block.visible;
    return Boolean(evaluateValue(block.visible));
  }

  /**
   * Handle button click
   */
  function handleButtonClick(block: any) {
    if (!onAction) return;

    const action = block.action;
    if (!action) return;

    console.log('🔘 [BlockRenderer] Button clicked:', { action, dataId: block.dataId, context });

    const params: any = {};

    // Collect form data if button is associated with a form
    if (block.formId) {
      const formData = getFormData(block.formId);
      if (formData) {
        params.formData = formData;
      }
    }

    // Add dataId if present (for delete actions, etc.)
    if (block.dataId) {
      const evaluatedId = evaluateValue(block.dataId);
      console.log('🆔 [BlockRenderer] Evaluated dataId:', block.dataId, '=>', evaluatedId);

      // For deleteComment action, use commentId
      if (action === 'deleteComment') {
        params.commentId = evaluatedId;
      } else {
        params.postId = evaluatedId;
      }
    }

    // Add action-specific parameters (optional)
    if (block.postId) {
      params.postId = evaluateValue(block.postId);
    }
    if (block.parentCommentId !== undefined) {
      params.parentCommentId = evaluateValue(block.parentCommentId);
    }
    if (block.commentId) {
      params.commentId = evaluateValue(block.commentId);
    }

    // Add stateUpdates if present (for setState action)
    if (block.stateUpdates) {
      params.stateUpdates = block.stateUpdates;
    }

    console.log('🎬 [BlockRenderer] Action triggered:', action, params);
    onAction(action, params);
  }

  /**
   * Handle form field change
   */
  function handleFieldChange(fieldName: string, value: any) {
    if (!onStateChange) return;

    const stateKey = block.stateKey || fieldName;
    console.log('📝 [BlockRenderer] Field changed:', stateKey, value);
    onStateChange(stateKey, value);
  }

  /**
   * Handle form submit
   */
  function handleSubmit(e: Event, block: any) {
    e.preventDefault();
    console.log('📋 [BlockRenderer] Form submit prevented, button will handle action');
    // Form submission is handled by the submit button's action
    // This just prevents the default browser behavior
  }

  /**
   * Get form data for a button with formId
   */
  function getFormData(formId: string): Record<string, any> | null {
    if (!formId) return null;

    const form = document.getElementById(formId) as HTMLFormElement;
    if (!form) {
      console.warn('⚠️ [BlockRenderer] Form not found:', formId);
      return null;
    }

    const formData = new FormData(form);
    const data: Record<string, any> = {};

    formData.forEach((value, key) => {
      data[key] = value;
    });

    console.log('📋 [BlockRenderer] Collected form data:', data);
    return data;
  }

  /**
   * Evaluate forEach items
   */
  function getForEachItems(block: any): any[] {
    if (!block.forEach) return [];

    const items = evaluateValue(`{{${block.forEach}}}`);
    return Array.isArray(items) ? items : [];
  }
</script>

{#if isBlockVisible(block)}
  {#if block.forEach}
    <!-- forEach loop -->
    {@const items = getForEachItems(block)}
    {@const itemVar = block.forEachAs || 'item'}

    {#each items as item, idx (idx)}
      {@const loopContext = { ...context, [itemVar]: item, index: idx }}

      {#if block.type === 'section-container'}
        <div class="section-container" style={evaluateValue(block.css, loopContext)} data-block-id="{block.id}-{idx}">
          {#if block.blocks}
            {#each block.blocks as childBlock (childBlock.id || Math.random())}
              <svelte:self block={childBlock} context={loopContext} {onAction} {onStateChange} />
            {/each}
          {/if}
        </div>
      {/if}
    {/each}
  {:else}
    <!-- Regular block rendering -->
    {#if block.type === 'section-container'}
      <div class="section-container" style={evaluateValue(block.css)} data-block-id={block.id}>
        {#if block.blocks}
          {#each block.blocks as childBlock (childBlock.id || Math.random())}
            <svelte:self block={childBlock} {context} {onAction} {onStateChange} />
          {/each}
        {/if}
      </div>

    {:else if block.type === 'heading'}
      <h1 style={evaluateValue(block.css)} data-block-id={block.id}>
        {evaluateValue(block.content)}
      </h1>

    {:else if block.type === 'text'}
      <p style={evaluateValue(block.css)} data-block-id={block.id}>
        {evaluateValue(block.content)}
      </p>

    {:else if block.type === 'form'}
      <form
        id={block.id}
        style={evaluateValue(block.css)}
        data-block-id={block.id}
        onsubmit={(e) => handleSubmit(e, block)}
      >
        {#if block.blocks}
          {#each block.blocks as childBlock (childBlock.id || Math.random())}
            <svelte:self block={childBlock} {context} {onAction} {onStateChange} />
          {/each}
        {/if}
      </form>

    {:else if block.type === 'form-field-textarea'}
      {@const fieldValue = block.stateKey ? context[block.stateKey] : ''}
      <textarea
        name={block.fieldName || block.name}
        placeholder={evaluateValue(block.placeholder)}
        value={fieldValue || ''}
        style={evaluateValue(block.css)}
        data-block-id={block.id}
        oninput={(e) => handleFieldChange(block.name, (e.target as HTMLTextAreaElement).value)}
      ></textarea>

    {:else if block.type === 'form-field-text'}
      {@const fieldValue = block.stateKey ? context[block.stateKey] : ''}
      <input
        type="text"
        name={block.fieldName || block.name}
        placeholder={evaluateValue(block.placeholder)}
        value={fieldValue || ''}
        style={evaluateValue(block.css)}
        data-block-id={block.id}
        oninput={(e) => handleFieldChange(block.name, (e.target as HTMLTextElement).value)}
      />

    {:else if block.type === 'nav-button'}
      {@const isDisabled = block.disabled ? Boolean(evaluateValue(block.disabled)) : false}
      <button
        type={block.formId ? 'submit' : 'button'}
        disabled={isDisabled}
        style={evaluateValue(block.css)}
        data-block-id={block.id}
        onclick={() => handleButtonClick(block)}
      >
        {evaluateValue(block.content || block.text)}
      </button>

    {:else if block.type === 'canvas-pattern'}
      <canvas
        use:initCanvas={block.id}
        style={evaluateValue(block.css)}
        data-block-id={block.id}
      ></canvas>

    {:else}
      <!-- Unknown block type -->
      <div data-block-id={block.id} style="border: 2px dashed red; padding: 8px;">
        Unknown block type: {block.type}
      </div>
    {/if}
  {/if}
{/if}

<style>
  .section-container {
    display: block;
  }

  textarea, input {
    font-family: inherit;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
