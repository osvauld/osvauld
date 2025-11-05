<script lang="ts">
/**
 * FormBlock - Form container with submission handling
 * Collects form data from context and writes to submissions_doc
 */

import type { FormBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';
import BlockRenderer from '../BlockRenderer.svelte';
import { loroCoordinator } from '../../shared/loro/loroCoordinator';

interface Props {
  block: FormBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction, onStateChange }: Props = $props();

// Evaluate CSS
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);

/**
 * Handle form submission
 * Collects all form field values from context and saves to submissionsDoc
 */
async function handleSubmit(event: Event) {
  event.preventDefault();

  console.log('📝 [FormBlock] Form submitted:', block.name);

  try {
    // Get the SubmissionsStore
    const submissionsStore = loroCoordinator.getSubmissionsStore();

    // Collect form data from context
    // We collect all form field values that are in the context
    const formData: Record<string, any> = {};

    // Extract form fields from child blocks
    const formFieldNames = extractFormFieldNames(block.blocks || []);

    console.log('📝 [FormBlock] Form fields:', formFieldNames);

    // Collect values from context for each field
    formFieldNames.forEach(fieldName => {
      if (context.hasOwnProperty(fieldName)) {
        formData[fieldName] = context[fieldName];
      }
    });

    console.log('📝 [FormBlock] Form data collected:', formData);

    // Add submission to store
    const submissionId = submissionsStore.addSubmission(block.name, formData);

    console.log('✅ [FormBlock] Submission saved:', submissionId);

    // Trigger onSubmit action if defined
    if (block.onSubmit && onAction) {
      onAction(block.onSubmit, { submissionId, formData });
    }
  } catch (error) {
    console.error('❌ [FormBlock] Submission failed:', error);
  }
}

/**
 * Extract form field names from child blocks
 * Recursively searches for input, textarea, checkbox, select, radio blocks
 */
function extractFormFieldNames(blocks: any[]): string[] {
  const fieldNames: string[] = [];

  for (const block of blocks) {
    // Check if block has a 'name' property (form inputs)
    if (block.name && (
      block.type === 'input' ||
      block.type === 'textarea' ||
      block.type === 'checkbox' ||
      block.type === 'select' ||
      block.type === 'radio'
    )) {
      fieldNames.push(block.name);
    }

    // Recursively search in nested blocks
    if (block.blocks && Array.isArray(block.blocks)) {
      fieldNames.push(...extractFormFieldNames(block.blocks));
    }

    // Check if/else blocks
    if (block.then && Array.isArray(block.then)) {
      fieldNames.push(...extractFormFieldNames(block.then));
    }
    if (block.else && Array.isArray(block.else)) {
      fieldNames.push(...extractFormFieldNames(block.else));
    }

    // Check match/case blocks
    if (block.cases && Array.isArray(block.cases)) {
      for (const caseBlock of block.cases) {
        if (caseBlock.blocks && Array.isArray(caseBlock.blocks)) {
          fieldNames.push(...extractFormFieldNames(caseBlock.blocks));
        }
      }
    }
  }

  return fieldNames;
}
</script>

<form
  style={styles}
  class={block.class}
  onsubmit={handleSubmit}
>
  {#if block.blocks}
    {#each block.blocks as childBlock (childBlock)}
      <BlockRenderer block={childBlock} {context} {onAction} {onStateChange} />
    {/each}
  {/if}
</form>
