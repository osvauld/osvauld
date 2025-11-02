<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { evaluateExpression } from '../../lib/humlEvaluator';

  interface Props {
    context: Record<string, any>;
    css?: string;
  }

  let { context, css }: Props = $props();

  let canvas: HTMLCanvasElement;
  let animationId: number | null = null;

  onMount(() => {
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    function render() {
      const gridSize = context.gridSize || 40;
      const time = context.time || 0;
      const pattern = context.selectedPattern || 'waves';

      const cellSize = 6;
      const width = gridSize * cellSize;
      const height = gridSize * cellSize;

      canvas.width = width;
      canvas.height = height;

      // Clear canvas
      ctx.fillStyle = '#0a0a0a';
      ctx.fillRect(0, 0, width, height);

      // Use OCaml evaluator with expressions from template
      if (typeof humlEval !== 'undefined' && humlEval.evaluateGrid) {
        // Get expression from state based on selected pattern
        const exprKey = pattern + 'Expr';
        const expr = context[exprKey] || context.wavesExpr;

        try {
          // Call OCaml to compute ALL cells in one go!
          const resultString = humlEval.evaluateGrid(expr, gridSize, time);
          const brightnesses = resultString.split(',').map(s => parseFloat(s));

          // Draw all cells
          for (let y = 0; y < gridSize; y++) {
            for (let x = 0; x < gridSize; x++) {
              const idx = y * gridSize + x;
              const brightness = brightnesses[idx] || 0;

              // Map brightness from [-1, 1] to [0, 255]
              const color = Math.floor((brightness + 1) * 127.5);

              ctx.fillStyle = `rgb(${color}, ${color * 0.5}, ${color * 0.8})`;
              ctx.fillRect(x * cellSize, y * cellSize, cellSize - 1, cellSize - 1);
            }
          }
        } catch (err) {
          console.error('[CanvasPattern] OCaml evaluation failed:', err);
        }
      } else {
        // Fallback to JavaScript Math
        for (let y = 0; y < gridSize; y++) {
          for (let x = 0; x < gridSize; x++) {
            let brightness = 0;

            if (pattern === 'waves') {
              brightness = (Math.sin(x * 0.2 + time) + Math.sin(y * 0.2 + time)) / 2;
            } else if (pattern === 'ripple') {
              const dx = x - gridSize/2;
              const dy = y - gridSize/2;
              const dist = Math.sqrt(dx * dx + dy * dy);
              brightness = Math.sin(dist * 0.3 - time);
            } else if (pattern === 'spiral') {
              const dx = x - gridSize/2;
              const dy = y - gridSize/2;
              const angle = Math.atan2(dy, dx);
              const dist = Math.sqrt(dx * dx + dy * dy);
              brightness = Math.sin(angle * 3 + dist * 0.2 - time);
            }

            const color = Math.floor((brightness + 1) * 127.5);
            ctx.fillStyle = `rgb(${color}, ${color * 0.5}, ${color * 0.8})`;
            ctx.fillRect(x * cellSize, y * cellSize, cellSize - 1, cellSize - 1);
          }
        }
      }

      animationId = requestAnimationFrame(render);
    }

    render();
  });

  onDestroy(() => {
    if (animationId) cancelAnimationFrame(animationId);
  });
</script>

<canvas bind:this={canvas} style={css}></canvas>

<style>
  canvas {
    image-rendering: pixelated;
    image-rendering: crisp-edges;
  }
</style>
