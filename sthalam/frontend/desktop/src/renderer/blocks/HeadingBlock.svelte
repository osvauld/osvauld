<script lang="ts">
/**
 * HeadingBlock - Render heading (h1-h6)
 */

import type { HeadingBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL, evaluateCEL } from '../../lib/services/celEvaluator';

interface Props {
  block: HeadingBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context }: Props = $props();

const level = $derived(block.level || 1);
const content = $derived.by(() => {
  // If content contains ${ } it's a full expression, evaluate it
  if (block.content.includes('${')) {
    const result = evaluateCEL(block.content, context);
    return typeof result === 'string' ? result : String(result);
  }
  // Otherwise it's a template string with {{ }}, interpolate it
  return interpolateCEL(block.content, context);
});
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);
</script>

{#if level === 1}
  <h1 style={styles} class={block.class}>{@html content}</h1>
{:else if level === 2}
  <h2 style={styles} class={block.class}>{@html content}</h2>
{:else if level === 3}
  <h3 style={styles} class={block.class}>{@html content}</h3>
{:else if level === 4}
  <h4 style={styles} class={block.class}>{@html content}</h4>
{:else if level === 5}
  <h5 style={styles} class={block.class}>{@html content}</h5>
{:else}
  <h6 style={styles} class={block.class}>{@html content}</h6>
{/if}
