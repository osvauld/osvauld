<script lang="ts">
/**
 * ScreenBlock - Top-level screen container
 */

import type { ScreenBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';
import BlockRenderer from '../BlockRenderer.svelte';

interface Props {
  block: ScreenBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();

const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);
</script>

<div
  class="sthalam-screen {block.class || ''}"
  data-screen={block.name}
  style={styles}
>
  {#if block.blocks}
    {#each block.blocks as childBlock (childBlock)}
      <BlockRenderer block={childBlock} {context} {onAction} {onStateChange} />
    {/each}
  {/if}
</div>

<style>
  .sthalam-screen {
    width: 100%;
    height: 100%;
  }
</style>
