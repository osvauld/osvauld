<script lang="ts">
/**
 * InputBlock - Text input field with local state binding
 */

import type { InputBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL, evaluateCEL } from '../../lib/services/celEvaluator';

interface Props {
  block: InputBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();

// Get current value from context state
const currentValue = $derived(
  block.value ? String(evaluateCEL(block.value, context) || '') : (context[block.name] || '')
);

// Interpolate placeholder
const placeholder = $derived(
  block.placeholder ? interpolateCEL(block.placeholder, context) : ''
);

// Evaluate disabled state
const isDisabled = $derived(
  block.disabled ? Boolean(evaluateCEL(block.disabled, context)) : false
);

// Evaluate CSS
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);

// Handle input change - update local state only (Option B)
function handleInput(event: Event) {
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

// Handle blur event
function handleBlur(event: Event) {
  if (block.onBlur && onAction) {
    const target = event.target as HTMLInputElement;
    onAction(block.onBlur, {
      name: block.name,
      value: target.value
    });
  }
}

// Handle focus event
function handleFocus(event: Event) {
  if (block.onFocus && onAction) {
    const target = event.target as HTMLInputElement;
    onAction(block.onFocus, {
      name: block.name,
      value: target.value
    });
  }
}
</script>

<input
  type={block.inputType || 'text'}
  name={block.name}
  value={currentValue}
  placeholder={placeholder}
  disabled={isDisabled}
  required={block.required}
  style={styles}
  class={block.class}
  oninput={handleInput}
  onblur={handleBlur}
  onfocus={handleFocus}
/>
