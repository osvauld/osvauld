<script lang="ts">
/**
 * BlockRenderer - Core recursive block renderer
 *
 * Renders all HUML blocks with:
 * - Control flow (when, if/else, match, forEach)
 * - CEL expression evaluation
 * - Action dispatching
 * - Lazy evaluation via direct object access
 */

import type { Block, Context, ActionHandler, StateChangeHandler } from '../lib/types/huml';
import { evaluateCEL, interpolateCEL } from '../lib/services/celEvaluator';
import { dispatchAction } from '../lib/services/actionDispatcher';

// Import block components
import ScreenBlock from './blocks/ScreenBlock.svelte';
import ContainerBlock from './blocks/ContainerBlock.svelte';
import SectionBlock from './blocks/SectionBlock.svelte';
import TextBlock from './blocks/TextBlock.svelte';
import HeadingBlock from './blocks/HeadingBlock.svelte';
import LabelBlock from './blocks/LabelBlock.svelte';
import ImageBlock from './blocks/ImageBlock.svelte';
import VideoBlock from './blocks/VideoBlock.svelte';
import FileBlock from './blocks/FileBlock.svelte';
import AudioBlock from './blocks/AudioBlock.svelte';
import InputBlock from './blocks/InputBlock.svelte';
import TextareaBlock from './blocks/TextareaBlock.svelte';
import CheckboxBlock from './blocks/CheckboxBlock.svelte';
import SelectBlock from './blocks/SelectBlock.svelte';
import RadioBlock from './blocks/RadioBlock.svelte';
import ButtonBlock from './blocks/ButtonBlock.svelte';
import LinkBlock from './blocks/LinkBlock.svelte';
import FormBlock from './blocks/FormBlock.svelte';
import CanvasBlock from './blocks/CanvasBlock.svelte';
import ModalBlock from './blocks/ModalBlock.svelte';

interface Props {
  block: Block;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();

/**
 * Check if block should be visible based on 'when' condition
 */
function checkVisibility(block: Block, context: Context): boolean {
  if (!('when' in block) || !block.when) {
    return true; // No condition = always visible
  }

  try {
    const result = evaluateCEL(block.when, context);
    return Boolean(result);
  } catch (error) {
    console.error(`[BlockRenderer] Error evaluating 'when' condition:`, error);
    return false;
  }
}

/**
 * Handle action dispatch
 */
function handleAction(action: string, params?: any): void {
  if (onAction) {
    onAction(action, params);
  } else {
    // Default: use global action dispatcher
    dispatchAction(action, params);
  }
}

/**
 * Handle state change
 */
function handleStateChange(key: string, value: any): void {
  if (onStateChange) {
    onStateChange(key, value);
  }
}

/**
 * Evaluate CSS expression
 */
function evaluateCSS(css: string | undefined, context: Context): string | undefined {
  if (!css) return undefined;

  // If CSS contains {{ }}, interpolate it
  if (css.includes('{{')) {
    return interpolateCEL(css, context);
  }

  return css;
}

// Check visibility
const isVisible = $derived(checkVisibility(block, context));

// Handle control flow blocks
const isIfBlock = $derived('if' in block);
const isMatchBlock = $derived('match' in block);
const hasForEach = $derived('forEach' in block && block.forEach);

</script>

{#if isVisible}
  {#if isIfBlock}
    <!-- If/Then/Else control flow -->
    {@const ifBlock = block as any}
    {@const condition = evaluateCEL(ifBlock.if, context)}

    {#if condition}
      {#if ifBlock.then}
        {#each ifBlock.then as childBlock (childBlock)}
          <svelte:self block={childBlock} {context} {onAction} {onStateChange} />
        {/each}
      {/if}
    {:else}
      {#if ifBlock.else}
        {#each ifBlock.else as childBlock (childBlock)}
          <svelte:self block={childBlock} {context} {onAction} {onStateChange} />
        {/each}
      {/if}
    {/if}

  {:else if isMatchBlock}
    <!-- Match/Cases control flow -->
    {@const matchBlock = block as any}
    {@const matchValue = evaluateCEL(matchBlock.match, context)}
    {@const matchedCase = matchBlock.cases.find((c: any) =>
      c.default || String(c.value) === String(matchValue)
    )}

    {#if matchedCase && matchedCase.blocks}
      {#each matchedCase.blocks as childBlock (childBlock)}
        <svelte:self block={childBlock} {context} {onAction} {onStateChange} />
      {/each}
    {/if}

  {:else if hasForEach}
    <!-- ForEach iteration -->
    {@const items = evaluateCEL(block.forEach, context)}
    {@const itemName = block.as || 'item'}
    {@const keyProp = block.key}

    {#if Array.isArray(items)}
      {#each items as item, index (keyProp ? item[keyProp] : index)}
        {@const loopContext = {
          ...context,
          [itemName]: item,
          [`${itemName}Index`]: index
        }}

        <!-- Render the block with loop context, but without forEach to avoid infinite loop -->
        {@const blockWithoutForEach = { ...block, forEach: undefined, as: undefined, key: undefined }}
        <svelte:self block={blockWithoutForEach} context={loopContext} {onAction} {onStateChange} />
      {/each}
    {/if}

  {:else if block.type === 'screen'}
    <ScreenBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'container'}
    <ContainerBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'section'}
    <SectionBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'text'}
    <TextBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'heading'}
    <HeadingBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'label'}
    <LabelBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'image'}
    <ImageBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'video'}
    <VideoBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'file'}
    <FileBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'audio'}
    <AudioBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'input'}
    <InputBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'textarea'}
    <TextareaBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'checkbox'}
    <CheckboxBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'select'}
    <SelectBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'radio'}
    <RadioBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'button'}
    <ButtonBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'link'}
    <LinkBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'form'}
    <FormBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'canvas'}
    <CanvasBlock {block} {context} {onAction} {onStateChange} />

  {:else if block.type === 'modal'}
    <ModalBlock {block} {context} {onAction} {onStateChange} />

  {:else}
    <div class="unknown-block" style="border: 2px solid red; padding: 1rem;">
      Unknown block type: {block.type}
    </div>
  {/if}
{/if}
