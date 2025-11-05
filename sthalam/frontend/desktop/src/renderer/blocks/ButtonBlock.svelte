<script lang="ts">
/**
 * ButtonBlock - Interactive button with action dispatching
 */

import type { ButtonBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL, evaluateCEL } from '../../lib/services/celEvaluator';

interface Props {
  block: ButtonBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction }: Props = $props();

// Interpolate content
const content = $derived.by(() => {
  return interpolateCEL(block.content, context);
});

// Evaluate disabled state
const isDisabled = $derived(
  block.disabled ? Boolean(evaluateCEL(block.disabled, context)) : false
);

// Evaluate CSS
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);

// Handle click
function handleClick(event: Event) {
  if (isDisabled) return;

  // If this is a submit button, let the form handle submission
  if (block.submitForm) {
    // Don't prevent default - let form onsubmit handle it
    return;
  }

  const action = block.onClick || block.action;
  if (!action) {
    console.warn('[ButtonBlock] No action defined');
    return;
  }

  // Build params from block properties
  let params: any = {};

  // Include explicit params if defined
  if (block.params) {
    params = { ...evaluateParams(block.params, context) };
  }

  // Include stateUpdates if defined (for setState action)
  if (block.stateUpdates) {
    params.stateUpdates = evaluateParams(block.stateUpdates, context, true); // true = treat as pure expressions
  }

  // Include targetScreen if defined (for navigate action)
  if (block.targetScreen) {
    params.targetScreen = block.targetScreen;
  }

  // Include other action-specific params
  if (block.formId) params.formId = block.formId;
  if (block.eventName) params.eventName = block.eventName;

  if (onAction) {
    onAction(action, params);
  }
}

// Recursively evaluate params
function evaluateParams(params: any, context: Context, treatAsPureExpr = false): any {
  if (typeof params === 'string') {
    // String interpolation (has {{ }})
    if (params.includes('{{')) {
      return interpolateCEL(params, context);
    }

    // Pure expression (evaluate as CEL) - only if starts with ${
    if (treatAsPureExpr && params.startsWith('${') && params.endsWith('}')) {
      try {
        return evaluateCEL(params, context);
      } catch (error) {
        console.error(`[ButtonBlock] Failed to evaluate expression: ${params}`, error);
        return params;
      }
    }

    // Literal string
    return params;
  }

  if (Array.isArray(params)) {
    return params.map(p => evaluateParams(p, context, treatAsPureExpr));
  }

  if (params && typeof params === 'object') {
    const result: any = {};
    for (const key in params) {
      result[key] = evaluateParams(params[key], context, treatAsPureExpr);
    }
    return result;
  }

  return params;
}
</script>

<button
  type={block.submitForm ? 'submit' : 'button'}
  disabled={isDisabled}
  style={styles}
  class={block.class}
  onclick={handleClick}
>
  {@html content}
</button>
