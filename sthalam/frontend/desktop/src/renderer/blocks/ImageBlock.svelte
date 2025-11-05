<script lang="ts">
/**
 * ImageBlock - Image display with src/alt
 */

import type { ImageBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';

interface Props {
  block: ImageBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context }: Props = $props();

// Interpolate src
const src = $derived(interpolateCEL(block.src, context));

// Interpolate alt
const alt = $derived(block.alt ? interpolateCEL(block.alt, context) : '');

// Interpolate width/height if they exist
// Always interpolate to handle both CEL expressions and static values
const width = $derived(block.width ? interpolateCEL(String(block.width), context) : undefined);
const height = $derived(block.height ? interpolateCEL(String(block.height), context) : undefined);

// Evaluate CSS
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);
</script>

<img
  {src}
  {alt}
  {width}
  {height}
  style={styles}
  class={block.class}
/>
