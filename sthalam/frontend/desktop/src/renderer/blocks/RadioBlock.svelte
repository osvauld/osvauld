<script lang="ts">
/**
 * RadioBlock - Radio button group
 */

import type { RadioBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL, evaluateCEL } from '../../lib/services/celEvaluator';

interface Props {
  block: RadioBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();

// Get current selected value from context state
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
  const target = event.target as HTMLInputElement;
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

<div style={styles} class={block.class}>
  {#each block.options as option}
    <label>
      <input
        type="radio"
        name={block.name}
        value={option.value}
        checked={currentValue === option.value}
        disabled={isDisabled}
        onchange={handleChange}
      />
      <span>{option.label}</span>
    </label>
  {/each}
</div>
