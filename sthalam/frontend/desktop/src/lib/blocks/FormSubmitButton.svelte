<script lang="ts">
	import type * as Y from 'yjs';

	interface Props {
		blockId: string;
		blockData: any;
		ydoc: Y.Doc;
		submissionsDoc?: Y.Doc;
		allBlocks: Map<string, any>;
		onNavigate?: (screenId: string) => void;
	}

	let { blockId, blockData, ydoc, submissionsDoc, allBlocks, onNavigate }: Props = $props();

	let isSubmitting = $state(false);
	let submitStatus = $state<'idle' | 'success' | 'error'>('idle');
	let errorMessage = $state('');

	async function handleSubmit(e: Event) {
		e.preventDefault();

		if (!blockData.formId) {
			errorMessage = 'This button is not linked to any form';
			submitStatus = 'error';
			return;
		}

		isSubmitting = true;
		submitStatus = 'idle';
		errorMessage = '';

		try {
			// 1. Collect all form field data
			const formData: Record<string, any> = {};
			const fieldElements: HTMLElement[] = [];

			// Find all form fields with matching formId
			document.querySelectorAll(`[data-form-id="${blockData.formId}"]`).forEach((el) => {
				fieldElements.push(el as HTMLElement);
			});

			// Validate and collect data from each field
			let hasErrors = false;
			for (const fieldEl of fieldElements) {
				const fieldId = fieldEl.getAttribute('data-field-id');
				if (!fieldId) continue;

				const fieldData = allBlocks.get(fieldId);
				if (!fieldData) continue;

				// Get the Svelte component instance (if we can)
				// For now, we'll read the input values directly from DOM
				const fieldName = fieldData.fieldName || fieldData.label || fieldId;

				if (fieldData.type === 'form-field-checkbox') {
					const checkbox = fieldEl.querySelector('input[type="checkbox"]') as HTMLInputElement;
					formData[fieldName] = checkbox?.checked || false;

					if (fieldData.required && !formData[fieldName]) {
						hasErrors = true;
					}
				} else {
					const input = fieldEl.querySelector('input, textarea') as HTMLInputElement | HTMLTextAreaElement;
					formData[fieldName] = input?.value || '';

					if (fieldData.required && !formData[fieldName].trim()) {
						hasErrors = true;
					}
				}
			}

			if (hasErrors) {
				errorMessage = 'Please fill in all required fields';
				submitStatus = 'error';
				return;
			}

			// 2. Get form metadata
			const formMetadata = allBlocks.get(blockData.formId);
			const eventName = formMetadata?.eventName || 'form_submission';

			// 3. Save to Yjs submissionsDoc (form submissions)
			// YjsManager will automatically detect this update and sync to sovereign node
			if (submissionsDoc) {
				submissionsDoc.transact(() => {
					const submissionsBlocks = submissionsDoc.getMap('blocks');
					const current = submissionsBlocks.get(`${blockData.formId}_submissions`) || { items: [] };

					const newSubmission = {
						id: crypto.randomUUID(),
						formId: blockData.formId,
						eventName,
						data: formData,
						timestamp: Date.now()
					};

					submissionsBlocks.set(`${blockData.formId}_submissions`, {
						items: [...current.items, newSubmission]
					});
				});
			}

			// 4. Show success
			submitStatus = 'success';

			// 5. Clear form fields
			setTimeout(() => {
				fieldElements.forEach((fieldEl) => {
					const input = fieldEl.querySelector('input:not([type="checkbox"]), textarea') as HTMLInputElement | HTMLTextAreaElement;
					if (input) input.value = '';

					const checkbox = fieldEl.querySelector('input[type="checkbox"]') as HTMLInputElement;
					if (checkbox) checkbox.checked = false;
				});
			}, 500);

			// 6. Navigate to target screen if specified
			if (blockData.targetContainerId && onNavigate) {
				setTimeout(() => {
					onNavigate(blockData.targetContainerId);
				}, 1000);
			}
		} catch (error) {
			console.error('Form submission error:', error);
			errorMessage = 'An error occurred while submitting the form';
			submitStatus = 'error';
		} finally {
			isSubmitting = false;

			// Reset status after 3 seconds
			if (submitStatus === 'success') {
				setTimeout(() => {
					submitStatus = 'idle';
				}, 3000);
			}
		}
	}
</script>

<button
	class="submit-button"
	class:submitting={isSubmitting}
	class:success={submitStatus === 'success'}
	class:error={submitStatus === 'error'}
	onclick={handleSubmit}
	disabled={isSubmitting}
	data-block-id={blockId}
>
	{#if isSubmitting}
		<span class="spinner"></span>
		Submitting...
	{:else if submitStatus === 'success'}
		✓ {blockData.content || 'Submit'}
	{:else}
		{blockData.content || 'Submit'}
	{/if}
</button>

{#if submitStatus === 'success'}
	<div class="status-message success-message">
		Form submitted successfully!
	</div>
{/if}

{#if submitStatus === 'error' && errorMessage}
	<div class="status-message error-message">
		{errorMessage}
	</div>
{/if}

<style>
	.submit-button {
		padding: 0.75rem 1.5rem;
		background: #0366d6;
		color: white;
		border: none;
		border-radius: 6px;
		font-weight: 600;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s;
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
	}

	.submit-button:hover:not(:disabled) {
		background: #0256c7;
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(3, 102, 214, 0.3);
	}

	.submit-button:disabled {
		cursor: not-allowed;
		opacity: 0.6;
	}

	.submit-button.submitting {
		background: #6a737d;
	}

	.submit-button.success {
		background: #28a745;
	}

	.submit-button.error {
		background: #d73a49;
	}

	.spinner {
		width: 16px;
		height: 16px;
		border: 2px solid rgba(255, 255, 255, 0.3);
		border-top-color: white;
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.status-message {
		margin-top: 0.75rem;
		padding: 0.75rem 1rem;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 500;
	}

	.success-message {
		background: #d4edda;
		color: #155724;
		border: 1px solid #c3e6cb;
	}

	.error-message {
		background: #f8d7da;
		color: #721c24;
		border: 1px solid #f5c6cb;
	}
</style>
