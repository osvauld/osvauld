<script lang="ts">
/**
 * SelectBlock - Dropdown select with options
 */

import type { SelectBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL, evaluateCEL } from '../../lib/services/celEvaluator';

interface Props {
  block: SelectBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();

// Get current value from context state
const currentValue = $derived(
  block.value ? String(evaluateCEL(block.value, context) || '') : (context[block.name] || '')
);

// Evaluate disabled state
const isDisabled = $derived(
  block.disabled ? Boolean(evaluateCEL(block.disabled, context)) : false
);

// Evaluate CSS
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);

// Handle change
function handleChange(event: Event) {
  const target = event.target as HTMLSelectElement;
  const newValue = target.value;

  // Update local state via onStateChange callback
  if (onStateChange) {
    onStateChange(block.name, newValue);
  }

  // Trigger onChange action if defined
  if (block.onChange && onAction) {
    onAction(block.onChange, {
      name: block.name,
      value: newValue
    });
  }
}
</script>

<select
  name={block.name}
  value={currentValue}
  disabled={isDisabled}
  required={block.required}
  style={styles}
  class={block.class}
  onchange={handleChange}
>
  {#each block.options as option}
    <option value={option.value}>{option.label}</option>
  {/each}
</select>
