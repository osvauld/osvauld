<script lang="ts">
/**
 * ContainerBlock - Layout container with flex/grid support
 */

import type { ContainerBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL, evaluateCEL } from '../../lib/services/celEvaluator';
import BlockRenderer from '../BlockRenderer.svelte';

interface Props {
  block: ContainerBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();

// Build container styles
const containerStyles = $derived(() => {
  const layout = block.layout || 'block';
  const styles: string[] = [];

  if (layout === 'flex') {
    styles.push('display: flex');
    if (block.direction) styles.push(`flex-direction: ${block.direction}`);
    if (block.gap) styles.push(`gap: ${block.gap}`);
    if (block.alignItems) styles.push(`align-items: ${block.alignItems}`);
    if (block.justifyContent) styles.push(`justify-content: ${block.justifyContent}`);
  } else if (layout === 'grid') {
    styles.push('display: grid');
    if (block.columns) styles.push(`grid-template-columns: repeat(${block.columns}, 1fr)`);
    if (block.gap) styles.push(`gap: ${block.gap}`);
  }

  // Add custom CSS
  if (block.css) {
    let customCSS: string;
    // If CSS contains ${ } it's a full expression, evaluate it
    if (block.css.includes('${')) {
      const result = evaluateCEL(block.css, context);
      customCSS = typeof result === 'string' ? result : String(result);
    } else {
      // Otherwise it's a template string with {{ }}, interpolate it
      customCSS = interpolateCEL(block.css, context);
    }
    styles.push(customCSS);
  }

  return styles.join('; ');
});
</script>

<div style={containerStyles()} class={block.class}>
  {#if block.blocks}
    {#each block.blocks as childBlock (childBlock)}
      <BlockRenderer block={childBlock} {context} {onAction} {onStateChange} />
    {/each}
  {/if}
</div>
