<script lang="ts">
/**
 * LabelBlock - Form label element
 */

import type { LabelBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';

interface Props {
  block: LabelBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context }: Props = $props();

// Interpolate content
const content = $derived(interpolateCEL(block.content, context));

// Evaluate CSS
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);
</script>

<label for={block.for} style={styles} class={block.class}>
  {@html content}
</label>
