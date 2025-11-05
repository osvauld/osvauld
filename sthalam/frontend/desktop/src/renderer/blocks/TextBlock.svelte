<script lang="ts">
/**
 * TextBlock - Render text content with CEL interpolation
 */

import type { TextBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';

interface Props {
  block: TextBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context }: Props = $props();

// Interpolate content with CEL
const content = $derived(interpolateCEL(block.content, context));

// Evaluate CSS
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);
</script>

<p style={styles} class={block.class}>
  {@html content}
</p>

<style>
  p {
    margin: 0;
  }
</style>
