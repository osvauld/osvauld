<script lang="ts">
	import { dataState } from "../state/data.svelte";

	let isSaving = $state(false);
	let lastSaveTime = $state<number | null>(null);
	let saveStatus = $state<'idle' | 'saving' | 'success' | 'error'>('idle');

	async function handleSave() {
		const currentResourceId = dataState.currentResourceId;
		if (!currentResourceId) {
			console.warn("⚠️ No resource selected to save");
			return;
		}

		// Don't allow saving while resource is loading
		if (dataState.isResourceLoading) {
			console.log("⏸️ Cannot save - resource is still loading");
			return;
		}

		// Don't allow concurrent saves
		if (isSaving) {
			console.log("⏸️ Save already in progress");
			return;
		}

		try {
			isSaving = true;
			saveStatus = 'saving';
			console.log("💾 Manual save triggered for:", currentResourceId);

			await dataState.saveCurrentResource(currentResourceId);

			lastSaveTime = Date.now();
			saveStatus = 'success';
			console.log("✅ Manual save completed");

			// Reset status after 2 seconds
			setTimeout(() => {
				saveStatus = 'idle';
			}, 2000);
		} catch (error) {
			console.error("❌ Manual save failed:", error);
			saveStatus = 'error';

			// Reset status after 3 seconds
			setTimeout(() => {
				saveStatus = 'idle';
			}, 3000);
		} finally {
			isSaving = false;
		}
	}

	// Keyboard shortcut: Ctrl+S / Cmd+S
	function handleKeydown(event: KeyboardEvent) {
		if ((event.ctrlKey || event.metaKey) && event.key === 's') {
			event.preventDefault();
			handleSave();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<button
	class="save-button"
	class:saving={saveStatus === 'saving'}
	class:success={saveStatus === 'success'}
	class:error={saveStatus === 'error'}
	class:disabled={!dataState.currentResourceId || dataState.isResourceLoading}
	onclick={handleSave}
	disabled={!dataState.currentResourceId || dataState.isResourceLoading}
	title="Save (Ctrl+S)"
>
	{#if saveStatus === 'saving'}
		<span class="icon">⏳</span>
		<span class="text">Saving...</span>
	{:else if saveStatus === 'success'}
		<span class="icon">✓</span>
		<span class="text">Saved</span>
	{:else if saveStatus === 'error'}
		<span class="icon">✗</span>
		<span class="text">Error</span>
	{:else if dataState.isResourceLoading}
		<span class="icon">⏸</span>
		<span class="text">Loading...</span>
	{:else}
		<span class="icon">💾</span>
		<span class="text">Save</span>
	{/if}
</button>

<style>
	.save-button {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 1rem;
		border: 1px solid #2f303e;
		border-radius: 8px;
		background: #0d0e13;
		color: #f2f2f0;
		cursor: pointer;
		font-size: 0.875rem;
		font-weight: 500;
		transition: all 0.2s;
		white-space: nowrap;
	}

	.save-button:hover:not(.disabled) {
		background: #16171f;
		border-color: #4d4f60;
		transform: translateY(-1px);
	}

	.save-button:active:not(.disabled) {
		transform: translateY(0);
	}

	.save-button.disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.save-button.saving {
		background: #16171f;
		border-color: #667eea;
	}

	.save-button.success {
		background: #16171f;
		border-color: #48bb78;
	}

	.save-button.error {
		background: #16171f;
		border-color: #f56565;
	}

	.icon {
		font-size: 1rem;
		line-height: 1;
	}

	.text {
		font-size: 0.875rem;
	}
</style>
