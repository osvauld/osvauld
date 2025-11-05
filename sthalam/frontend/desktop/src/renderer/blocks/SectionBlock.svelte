<script lang="ts">
import type { SectionBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';
import BlockRenderer from '../BlockRenderer.svelte';

interface Props {
  block: SectionBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);
</script>

<section style={styles} class={block.class}>
  {#if block.blocks}
    {#each block.blocks as childBlock (childBlock)}
      <BlockRenderer block={childBlock} {context} {onAction} {onStateChange} />
    {/each}
  {/if}
</section>
