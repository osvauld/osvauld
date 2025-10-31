<script context="module" lang="ts">
	// Module-level cache for multi-screen form values (shared across all NavButton instances)
	// This runs ONCE per module, not once per component instance
	// This accumulates data as user navigates, only submits on final submit button
	const formValuesCache: Record<string, Record<string, any>> = {};

	// Log module initialization to detect if module is being re-imported
	console.log(`🔧 [NavButton Module] Module-level code initialized at ${Date.now()}`);
	console.log(`🔧 [NavButton Module] formValuesCache initialized:`, formValuesCache);
</script>

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

		console.log(`🔘 [NavButton] Clicked: formId="${formId}", targetId="${targetId}", fieldName="${fieldName}", fieldValue="${fieldValue}"`);

		if (isSubmitting) {
			console.log(`⚠️ [NavButton] Already submitting, ignoring click`);
			return; // Prevent double-clicks
		}

		// ===== NEW IMPLICIT CACHING LOGIC =====

		// If formId exists, this is a form-related button
		if (formId) {
			console.log(`📋 [NavButton] Form button detected for formId="${formId}"`);

			// STEP 1: Always cache data before navigating or submitting

			// If this button has a specific field value, cache it
			if (fieldName && fieldValue !== undefined) {
				console.log(`📝 [NavButton] Caching field value: ${fieldName} = ${fieldValue}`);
				if (!formValuesCache[formId]) {
					formValuesCache[formId] = {};
				}
				formValuesCache[formId][fieldName] = fieldValue;
			}

			// Always cache visible form fields on the current screen
			console.log(`📦 [NavButton] Collecting and caching visible form fields`);
			collectAndStoreFormFields(formId);

			// STEP 2: Submit or just Navigate?

			const shouldSubmit = blockData.submit === true || !targetId;

			if (shouldSubmit) {
				// Submit form (and navigate to targetId after if provided)
				console.log(`✅ [NavButton] Submitting form ${formId}` + (targetId ? ` then navigating to ${targetId}` : ''));
				await submitForm(formId, targetId);
			} else {
				// Just navigate (data already cached above)
				console.log(`➡️  [NavButton] Navigating to ${targetId} (data cached, not submitting)`);
				if (onNavigate) {
					onNavigate(targetId);
				}
			}

			return;
		}

		// ===== NON-FORM BUTTONS (show/hide/toggle/navigate) =====

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

	// Helper: Collect visible form fields and store them in memory (for multi-screen forms)
	function collectAndStoreFormFields(formId: string) {
		console.log(`🔍 [collectAndStoreFormFields] Called for ${formId}`);

		// Initialize cache for this form if needed
		if (!formValuesCache[formId]) {
			formValuesCache[formId] = {};
			console.log(`📦 [collectAndStoreFormFields] Initialized new cache for ${formId}`);
		}

		let fieldsCollected = 0;
		const foundElements = document.querySelectorAll(`[data-form-id="${formId}"]`);
		console.log(`🔍 [collectAndStoreFormFields] Found ${foundElements.length} form field elements with data-form-id="${formId}"`);

		// Collect all visible form field values
		foundElements.forEach((el, index) => {
			const fieldId = (el as HTMLElement).getAttribute('data-field-id');
			console.log(`🔍 [collectAndStoreFormFields] Element ${index}: fieldId="${fieldId}"`);

			if (!fieldId) {
				console.warn(`⚠️ [collectAndStoreFormFields] Element ${index} has no data-field-id`);
				return;
			}

			const fieldBlock = allBlocks.get(fieldId);
			if (!fieldBlock) {
				console.warn(`⚠️ [collectAndStoreFormFields] No block found for fieldId="${fieldId}"`);
				return;
			}

			const fieldName = fieldBlock.fieldName || fieldBlock.label || fieldId;
			console.log(`🔍 [collectAndStoreFormFields] Processing field: ${fieldName} (type: ${fieldBlock.type})`);

			if (fieldBlock.type === 'form-field-checkbox') {
				const checkbox = el.querySelector('input[type="checkbox"]') as HTMLInputElement;
				const value = checkbox?.checked || false;
				formValuesCache[formId][fieldName] = value;
				console.log(`✅ [collectAndStoreFormFields] Cached checkbox ${fieldName} = ${value}`);
				fieldsCollected++;
			} else {
				const input = el.querySelector('input, textarea') as HTMLInputElement | HTMLTextAreaElement;
				const value = input?.value || '';
				console.log(`🔍 [collectAndStoreFormFields] Field ${fieldName} value: "${value}"`);
				// Store ALL values, even empty ones for multi-screen forms
				formValuesCache[formId][fieldName] = value;
				if (value) {
					fieldsCollected++;
				}
				console.log(`✅ [collectAndStoreFormFields] Cached ${fieldName} = "${value}"`);
			}
		});

		console.log(`📦 [collectAndStoreFormFields] Cached ${fieldsCollected} non-empty fields for ${formId}`);
		console.log(`📦 [collectAndStoreFormFields] Total cache contents:`, JSON.stringify(formValuesCache[formId], null, 2));
	}

	// Submit form with all cached and visible form data
	async function submitForm(formId: string, targetId?: string) {
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

			// Clear the cache for this form after successful submission
			delete formValuesCache[formId];
			console.log(`✅ Form ${formId} submitted successfully, cache cleared`);

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

	// Helper: Collect all form field values (including cached values from MODE 3A)
	function collectFormFields(formId: string): Record<string, any> {
		console.log(`🔍 [collectFormFields] Called for ${formId}`);
		console.log(`🔍 [collectFormFields] Current formValuesCache state:`, formValuesCache);

		const formData: Record<string, any> = {};

		// First, retrieve any cached values from MODE 3A (setValueOnly) and navigation
		if (formValuesCache[formId]) {
			Object.assign(formData, formValuesCache[formId]);
			console.log(`📥 [collectFormFields] Loaded ${Object.keys(formValuesCache[formId]).length} cached values for ${formId}`);
			console.log(`📥 [collectFormFields] Cached values:`, JSON.stringify(formValuesCache[formId], null, 2));
		} else {
			console.warn(`⚠️ [collectFormFields] No cached values found for ${formId}`);
		}

		// Then, collect visible form field values (which can override cached values)
		let visibleFields = 0;
		const foundElements = document.querySelectorAll(`[data-form-id="${formId}"]`);
		console.log(`🔍 [collectFormFields] Found ${foundElements.length} visible form field elements`);

		foundElements.forEach((el, index) => {
			const fieldId = (el as HTMLElement).getAttribute('data-field-id');
			console.log(`🔍 [collectFormFields] Visible element ${index}: fieldId="${fieldId}"`);

			if (!fieldId) return;

			const fieldBlock = allBlocks.get(fieldId);
			if (!fieldBlock) return;

			const fieldName = fieldBlock.fieldName || fieldBlock.label || fieldId;

			if (fieldBlock.type === 'form-field-checkbox') {
				const checkbox = el.querySelector('input[type="checkbox"]') as HTMLInputElement;
				formData[fieldName] = checkbox?.checked || false;
				console.log(`✅ [collectFormFields] Visible checkbox ${fieldName} = ${checkbox?.checked}`);
				visibleFields++;
			} else {
				const input = el.querySelector('input, textarea') as HTMLInputElement | HTMLTextAreaElement;
				formData[fieldName] = input?.value || '';
				console.log(`✅ [collectFormFields] Visible field ${fieldName} = "${input?.value}"`);
				if (input?.value) visibleFields++;
			}
		});

		console.log(`📤 [collectFormFields] Collected ${visibleFields} non-empty visible fields`);
		console.log(`📤 [collectFormFields] Total fields for submission: ${Object.keys(formData).length}`);
		console.log(`📤 [collectFormFields] Final formData:`, JSON.stringify(formData, null, 2));

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
