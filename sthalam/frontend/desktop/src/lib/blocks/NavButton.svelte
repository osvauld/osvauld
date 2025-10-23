<script lang="ts">
	import { onMount } from 'svelte';
	import type * as Y from 'yjs';
	import type { SubmissionsStore } from '../submissionsStore';
	import { dataState } from '../../state';
	import { sendMessage } from '../../utils/helper';

	interface Props {
		blockId: string;
		blockData: any;
		allBlocks: Map<string, any>;
		ydoc?: Y.Doc;
		onNavigate?: (screenId: string) => void;
	}

	let { blockId, blockData, allBlocks, ydoc, onNavigate }: Props = $props();

	let submissionsStore: SubmissionsStore | null = null;
	let isSubmitting = $state(false);

	// Setup subscription to submissions store (needed for form submission modes)
	function handleStoreReady(event: CustomEvent) {
		const storeInstance = event.detail.submissionsStore;
		if (storeInstance) {
			submissionsStore = storeInstance;
		}
	}

	onMount(() => {
		// Check if coordinator already exists
		const coordinator = dataState.getBlocksuiteCoordinator();
		if (coordinator) {
			const existingStore = coordinator.getSubmissionsStore();
			if (existingStore) {
				submissionsStore = existingStore;
			}
		}

		// Listen for the event in case it fires later
		document.addEventListener("submissions-store-ready", handleStoreReady as EventListener);

		return () => {
			document.removeEventListener("submissions-store-ready", handleStoreReady as EventListener);
		};
	});

	async function handleClick() {
		const action = blockData.action || 'navigate';
		const targetId = blockData.targetContainerId;
		const formId = blockData.formId;
		const fieldName = blockData.fieldName;
		const fieldValue = blockData.value;

		// Determine mode based on properties
		const hasFieldNameAndValue = fieldName && fieldValue !== undefined;
		const hasFormId = !!formId;

		if (isSubmitting) return; // Prevent double-clicks

		// MODE 3: Set field value + submit + navigate (Branching choice)
		if (hasFieldNameAndValue && hasFormId) {
			await handleFieldValueSubmit(formId, fieldName, fieldValue, targetId);
			return;
		}

		// MODE 2: Submit entire form + navigate (Form submit button)
		if (hasFormId && action === 'navigate') {
			await handleFormSubmit(formId, targetId);
			return;
		}

		// MODE 1: Just navigate or show/hide/toggle
		switch (action) {
			case 'show':
				updateContainerVisibility(targetId, true);
				break;

			case 'hide':
				updateContainerVisibility(targetId, false);
				break;

			case 'toggle':
				toggleContainerVisibility(targetId);
				break;

			case 'navigate':
				if (onNavigate && targetId) {
					onNavigate(targetId);
				}
				break;

			default:
				console.warn(`⚠️ Unknown navigation action: ${action}`);
		}
	}

	// MODE 3: Set field value + submit + navigate (Branching choice)
	async function handleFieldValueSubmit(formId: string, fieldName: string, value: any, targetId?: string) {
		if (!submissionsStore) {
			console.error('❌ SubmissionsStore not available');
			return;
		}

		isSubmitting = true;

		try {
			// Collect ALL other form fields (if any exist)
			const formData = collectFormFields(formId);

			// Set this specific field value (overriding any existing value)
			formData[fieldName] = value;

			// Get form metadata
			const formMetadata = allBlocks.get(formId);
			const eventName = formMetadata?.eventName || 'form_submission';

			// Submit to store
			submissionsStore.addSubmission(formId, {
				formId,
				eventName,
				data: formData
			});

			// Save and sync
			const currentResourceId = dataState.currentResourceId;
			if (currentResourceId) {
				await dataState.saveCurrentResource(currentResourceId);
				try {
					await sendMessage('syncResource', { resourceId: currentResourceId });
				} catch (syncError) {
					console.error('Failed to sync:', syncError);
				}
			}

			// Navigate
			if (targetId && onNavigate) {
				setTimeout(() => onNavigate(targetId), 300);
			}
		} catch (error) {
			console.error('❌ Field value submit error:', error);
		} finally {
			isSubmitting = false;
		}
	}

	// MODE 2: Submit entire form + navigate (Form submit button)
	async function handleFormSubmit(formId: string, targetId?: string) {
		if (!submissionsStore) {
			console.error('❌ SubmissionsStore not available');
			return;
		}

		isSubmitting = true;

		try {
			// Collect all form fields
			const formData = collectFormFields(formId);

			// Validate required fields
			let hasErrors = false;
			document.querySelectorAll(`[data-form-id="${formId}"]`).forEach((el) => {
				const fieldId = (el as HTMLElement).getAttribute('data-field-id');
				if (!fieldId) return;

				const fieldBlock = allBlocks.get(fieldId);
				if (!fieldBlock || !fieldBlock.required) return;

				const fieldName = fieldBlock.fieldName || fieldBlock.label || fieldId;
				const value = formData[fieldName];

				if (fieldBlock.type === 'form-field-checkbox') {
					if (!value) hasErrors = true;
				} else {
					if (!value || value.toString().trim() === '') hasErrors = true;
				}
			});

			if (hasErrors) {
				alert('Please fill in all required fields');
				return;
			}

			// Get form metadata
			const formMetadata = allBlocks.get(formId);
			const eventName = formMetadata?.eventName || 'form_submission';

			// Submit to store
			submissionsStore.addSubmission(formId, {
				formId,
				eventName,
				data: formData
			});

			// Save and sync
			const currentResourceId = dataState.currentResourceId;
			if (currentResourceId) {
				await dataState.saveCurrentResource(currentResourceId);
				try {
					await sendMessage('syncResource', { resourceId: currentResourceId });
				} catch (syncError) {
					console.error('Failed to sync:', syncError);
				}
			}

			// Clear form fields
			setTimeout(() => {
				document.querySelectorAll(`[data-form-id="${formId}"]`).forEach((fieldEl) => {
					const input = fieldEl.querySelector('input:not([type="checkbox"]), textarea') as HTMLInputElement | HTMLTextAreaElement;
					if (input) input.value = '';

					const checkbox = fieldEl.querySelector('input[type="checkbox"]') as HTMLInputElement;
					if (checkbox) checkbox.checked = false;
				});
			}, 500);

			// Navigate
			if (targetId && onNavigate) {
				setTimeout(() => onNavigate(targetId), 1000);
			}
		} catch (error) {
			console.error('❌ Form submit error:', error);
		} finally {
			isSubmitting = false;
		}
	}

	// Helper: Collect all form field values
	function collectFormFields(formId: string): Record<string, any> {
		const formData: Record<string, any> = {};

		document.querySelectorAll(`[data-form-id="${formId}"]`).forEach((el) => {
			const fieldId = (el as HTMLElement).getAttribute('data-field-id');
			if (!fieldId) return;

			const fieldBlock = allBlocks.get(fieldId);
			if (!fieldBlock) return;

			const fieldName = fieldBlock.fieldName || fieldBlock.label || fieldId;

			if (fieldBlock.type === 'form-field-checkbox') {
				const checkbox = el.querySelector('input[type="checkbox"]') as HTMLInputElement;
				formData[fieldName] = checkbox?.checked || false;
			} else {
				const input = el.querySelector('input, textarea') as HTMLInputElement | HTMLTextAreaElement;
				formData[fieldName] = input?.value || '';
			}
		});

		return formData;
	}

	function updateContainerVisibility(containerId: string, visible: boolean) {
		if (!ydoc) {
			console.error('❌ No Yjs document available');
			return;
		}

		const blocks = ydoc.getMap('blocks');
		const container = blocks.get(containerId);

		if (!container) {
			console.error(`❌ Container not found: ${containerId}`);
			return;
		}

		// Update visibility in Yjs
		ydoc.transact(() => {
			container.visible = visible;
			blocks.set(containerId, container);
		});

		console.log(`✅ Container ${containerId} visibility set to: ${visible}`);
	}

	function toggleContainerVisibility(containerId: string) {
		if (!ydoc) {
			console.error('❌ No Yjs document available');
			return;
		}

		const blocks = ydoc.getMap('blocks');
		const container = blocks.get(containerId);

		if (!container) {
			console.error(`❌ Container not found: ${containerId}`);
			return;
		}

		// Toggle visibility
		const newVisibility = !container.visible;
		updateContainerVisibility(containerId, newVisibility);
	}
</script>

<button
	class="nav-button"
	class:submitting={isSubmitting}
	style={blockData.css || ""}
	onclick={handleClick}
	disabled={isSubmitting}
	data-block-id={blockId}
>
	{#if isSubmitting}
		⏳ {blockData.content || 'Next'}
	{:else}
		{blockData.content || 'Next'}
	{/if}
</button>

<style>
	.nav-button {
		/* Default fallback styles - can be overridden by custom CSS */
		padding: 0.75rem 1.5rem;
		background: #89b4fa;
		color: #010409;
		border: none;
		border-radius: 8px;
		font-weight: 600;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s;
	}

	.nav-button:hover:not(:disabled) {
		opacity: 0.9;
		transform: translateY(-2px);
	}

	.nav-button:active:not(:disabled) {
		transform: translateY(0);
	}

	.nav-button:disabled {
		cursor: not-allowed;
		opacity: 0.6;
	}

	.nav-button.submitting {
		opacity: 0.7;
	}
</style>
