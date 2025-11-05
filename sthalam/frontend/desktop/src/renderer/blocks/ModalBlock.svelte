<script lang="ts">
/**
 * ModalBlock - Multi-purpose modal component supporting dialog, drawer, and popover modes
 */

import type { ModalBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL, evaluateCEL } from '../../lib/services/celEvaluator';
import BlockRenderer from '../BlockRenderer.svelte';
import { onMount, onDestroy } from 'svelte';

interface Props {
  block: ModalBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();

// Evaluate visibility
const isVisible = $derived(
  block.visible ? Boolean(evaluateCEL(block.visible, context)) : false
);

// Configuration with defaults
const modalType = $derived(block.modalType || 'dialog');
const position = $derived(block.position || 'right');
const showBackdrop = $derived(block.backdrop !== false);
const closeOnBackdrop = $derived(block.closeOnBackdropClick !== false);
const closeOnEsc = $derived(block.closeOnEscape !== false);
const closable = $derived(block.closable !== false);
const size = $derived(block.size || 'medium');
const placement = $derived(block.placement || 'top');

// Evaluate custom CSS
const customStyles = $derived(block.css ? interpolateCEL(block.css, context) : '');

// Size presets
const sizeMap: Record<string, string> = {
  small: '400px',
  medium: '600px',
  large: '800px',
  fullscreen: '100vw'
};

// Build modal styles
const modalStyles = $derived.by(() => {
  const styles: string[] = [];

  // Custom dimensions override size presets
  if (block.width) {
    styles.push(`width: ${block.width}`);
  } else if (modalType === 'dialog') {
    styles.push(`width: ${sizeMap[size]}`);
  }

  if (block.height) {
    styles.push(`height: ${block.height}`);
  }

  // Add custom CSS
  if (customStyles) {
    styles.push(customStyles);
  }

  return styles.join('; ');
});

// Position class for drawer
const positionClass = $derived(
  modalType === 'drawer' ? `modal-drawer-${position}` : ''
);

// Recursively evaluate params (same logic as ButtonBlock)
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
        console.error(`[ModalBlock] Failed to evaluate expression: ${params}`, error);
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

// Handle close action
function handleClose() {
  if (block.onClose && onAction) {
    let params: any = {};

    // Evaluate closeParams if provided
    if (block.closeParams) {
      params = { ...evaluateParams(block.closeParams, context) };
    }

    // If closeParams has stateUpdates, evaluate them as pure expressions
    if (block.closeParams?.stateUpdates) {
      params.stateUpdates = evaluateParams(block.closeParams.stateUpdates, context, true);
    }

    onAction(block.onClose, params);
  }
}

// Handle backdrop click
function handleBackdropClick(event: MouseEvent) {
  if (closeOnBackdrop && event.target === event.currentTarget) {
    handleClose();
  }
}

// Handle escape key
function handleKeydown(event: KeyboardEvent) {
  if (closeOnEsc && event.key === 'Escape' && isVisible) {
    handleClose();
  }
}

// Popover positioning
let anchorElement: HTMLElement | null = null;
let popoverStyles = $state('');

function updatePopoverPosition() {
  if (modalType === 'popover' && block.anchorId && anchorElement) {
    const rect = anchorElement.getBoundingClientRect();
    const spacing = 8; // Gap between anchor and popover

    let top = 0;
    let left = 0;

    switch (placement) {
      case 'top':
        top = rect.top - spacing;
        left = rect.left + rect.width / 2;
        popoverStyles = `top: ${top}px; left: ${left}px; transform: translate(-50%, -100%);`;
        break;
      case 'bottom':
        top = rect.bottom + spacing;
        left = rect.left + rect.width / 2;
        popoverStyles = `top: ${top}px; left: ${left}px; transform: translate(-50%, 0);`;
        break;
      case 'left':
        top = rect.top + rect.height / 2;
        left = rect.left - spacing;
        popoverStyles = `top: ${top}px; left: ${left}px; transform: translate(-100%, -50%);`;
        break;
      case 'right':
        top = rect.top + rect.height / 2;
        left = rect.right + spacing;
        popoverStyles = `top: ${top}px; left: ${left}px; transform: translate(0, -50%);`;
        break;
    }
  }
}

// Setup and cleanup
onMount(() => {
  if (modalType === 'popover' && block.anchorId) {
    anchorElement = document.getElementById(block.anchorId);
    if (anchorElement) {
      updatePopoverPosition();
      window.addEventListener('resize', updatePopoverPosition);
      window.addEventListener('scroll', updatePopoverPosition);
    }
  }
});

onDestroy(() => {
  if (modalType === 'popover') {
    window.removeEventListener('resize', updatePopoverPosition);
    window.removeEventListener('scroll', updatePopoverPosition);
  }
});

// Update popover position when visibility changes
$effect(() => {
  if (isVisible && modalType === 'popover') {
    updatePopoverPosition();
  }
});
</script>

<svelte:window on:keydown={handleKeydown} />

{#if isVisible}
  <!-- Backdrop -->
  {#if showBackdrop && modalType !== 'popover'}
    <div
      class="modal-backdrop {modalType === 'drawer' ? 'modal-backdrop-drawer' : ''}"
      onclick={handleBackdropClick}
      role="presentation"
    />
  {/if}

  <!-- Modal Content -->
  {#if modalType === 'dialog'}
    <div class="modal-dialog" role="dialog" aria-modal="true" style={modalStyles}>
      {#if closable}
        <button class="modal-close" onclick={handleClose} aria-label="Close modal">
          ×
        </button>
      {/if}

      <div class="modal-content">
        {#if block.blocks}
          {#each block.blocks as childBlock (childBlock)}
            <BlockRenderer block={childBlock} {context} {onAction} {onStateChange} />
          {/each}
        {/if}
      </div>
    </div>
  {:else if modalType === 'drawer'}
    <div
      class="modal-drawer {positionClass}"
      role="dialog"
      aria-modal="true"
      style={modalStyles}
    >
      {#if closable}
        <button class="modal-close" onclick={handleClose} aria-label="Close drawer">
          ×
        </button>
      {/if}

      <div class="modal-content">
        {#if block.blocks}
          {#each block.blocks as childBlock (childBlock)}
            <BlockRenderer block={childBlock} {context} {onAction} {onStateChange} />
          {/each}
        {/if}
      </div>
    </div>
  {:else if modalType === 'popover'}
    <div
      class="modal-popover"
      role="tooltip"
      style="{popoverStyles}; {modalStyles}"
    >
      {#if closable}
        <button class="modal-close modal-close-small" onclick={handleClose} aria-label="Close popover">
          ×
        </button>
      {/if}

      <div class="modal-content">
        {#if block.blocks}
          {#each block.blocks as childBlock (childBlock)}
            <BlockRenderer block={childBlock} {context} {onAction} {onStateChange} />
          {/each}
        {/if}
      </div>
    </div>
  {/if}
{/if}

<style>
/* Backdrop */
.modal-backdrop {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.5);
  z-index: 1000;
  opacity: 1;
  transition: opacity 0.2s ease;
}

.modal-backdrop-drawer {
  background: rgba(0, 0, 0, 0.3);
}

/* Dialog Modal */
.modal-dialog {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background: white;
  border-radius: 8px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.2);
  z-index: 1001;
  max-height: 90vh;
  overflow: auto;
  opacity: 1;
  transition: opacity 0.2s ease, transform 0.2s ease;
}

/* Drawer Modal */
.modal-drawer {
  position: fixed;
  background: white;
  box-shadow: 0 0 20px rgba(0, 0, 0, 0.3);
  z-index: 1001;
  overflow: auto;
  transition: transform 0.3s ease;
}

.modal-drawer-right {
  top: 0;
  right: 0;
  width: 400px;
  height: 100vh;
  transform: translateX(0);
}

.modal-drawer-left {
  top: 0;
  left: 0;
  width: 400px;
  height: 100vh;
  transform: translateX(0);
}

.modal-drawer-top {
  top: 0;
  left: 0;
  width: 100vw;
  height: 300px;
  transform: translateY(0);
}

.modal-drawer-bottom {
  bottom: 0;
  left: 0;
  width: 100vw;
  height: 300px;
  transform: translateY(0);
}

/* Popover Modal */
.modal-popover {
  position: fixed;
  background: white;
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  border: 1px solid #e5e7eb;
  z-index: 1001;
  max-width: 300px;
  opacity: 1;
  transition: opacity 0.15s ease, transform 0.15s ease;
}

/* Close Button */
.modal-close {
  position: absolute;
  top: 1rem;
  right: 1rem;
  width: 32px;
  height: 32px;
  border: none;
  background: transparent;
  font-size: 24px;
  line-height: 1;
  color: #6b7280;
  cursor: pointer;
  border-radius: 4px;
  transition: background 0.2s ease, color 0.2s ease;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
}

.modal-close:hover {
  background: #f3f4f6;
  color: #111827;
}

.modal-close-small {
  width: 24px;
  height: 24px;
  font-size: 18px;
  top: 0.5rem;
  right: 0.5rem;
}

/* Modal Content */
.modal-content {
  padding: 2rem;
}

.modal-popover .modal-content {
  padding: 1rem;
}
</style>
