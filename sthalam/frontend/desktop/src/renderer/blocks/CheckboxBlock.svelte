<script lang="ts">
/**
 * CheckboxBlock - Boolean checkbox with optional label
 */

import type { CheckboxBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL, evaluateCEL } from '../../lib/services/celEvaluator';

interface Props {
  block: CheckboxBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();

// Get current checked state from context
const isChecked = $derived(
  block.checked ? Boolean(evaluateCEL(block.checked, context)) : Boolean(context[block.name])
);

// Interpolate label
const label = $derived(
  block.label ? interpolateCEL(block.label, context) : ''
);

// Evaluate disabled state
const isDisabled = $derived(
  block.disabled ? Boolean(evaluateCEL(block.disabled, context)) : false
);

// Evaluate CSS
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);

// Handle change
function handleChange(event: Event) {
  const target = event.target as HTMLInputElement;
  const newValue = target.checked;

  // Update local state via onStateChange callback
  if (onStateChange) {
    onStateChange(block.name, newValue);
  }

  // Trigger onChange action if defined
  if (block.onChange && onAction) {
    onAction(block.onChange, {
      name: block.name,
      checked: newValue
    });
  }
}
</script>

<label style={styles} class={block.class}>
  <input
    type="checkbox"
    name={block.name}
    checked={isChecked}
    disabled={isDisabled}
    onchange={handleChange}
  />
  {#if label}
    <span>{@html label}</span>
  {/if}
</label>
