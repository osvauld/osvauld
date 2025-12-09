<script lang="ts">
	import { dataState } from "../state";
	import { sendMessage } from '../utils/api';
	import { Sync, Tick } from "@osvauld/icons";

	let isSyncing = $state(false);
	let syncStatus = $state<'idle' | 'success' | 'error'>('idle');
	let errorMessage = $state('');

	const handleSync = async () => {
		const resourceId = dataState.currentResourceId;
		if (!resourceId) {
			console.error("No resource selected");
			return;
		}

		isSyncing = true;
		syncStatus = 'idle';
		errorMessage = '';

		try {
			console.log("🔄 Syncing resource:", resourceId);

			// No need to save - SyncManager auto-syncs Loro changes to backend

			// Call backend to sync resource to P2P network
			await sendMessage("syncResource", {
				resourceId: resourceId,
			});

			console.log("✅ Resource synced successfully");
			syncStatus = 'success';

			// Reset status after 2 seconds
			setTimeout(() => {
				syncStatus = 'idle';
			}, 2000);
		} catch (error) {
			console.error("❌ Failed to sync resource:", error);
			errorMessage = error instanceof Error ? error.message : 'Failed to sync';
			syncStatus = 'error';

			// Reset status after 3 seconds
			setTimeout(() => {
				syncStatus = 'idle';
			}, 3000);
		} finally {
			isSyncing = false;
		}
	};

	// Show when a resource is selected
	const shouldShow = $derived(!!dataState.currentResourceId);
</script>

{#if shouldShow}
	<button
		onclick={handleSync}
		disabled={isSyncing}
		class="w-full px-4 py-2 rounded-lg font-medium text-sm transition-all flex items-center justify-center gap-2"
		class:bg-green-600={syncStatus === 'success'}
		class:bg-red-600={syncStatus === 'error'}
		class:bg-blue-600={syncStatus === 'idle' && !isSyncing}
		class:bg-gray-600={isSyncing}
		class:text-white={true}
		class:opacity-70={isSyncing}
		class:cursor-not-allowed={isSyncing}
		title={syncStatus === 'error' ? errorMessage : 'Sync resource to P2P network'}
	>
		{#if isSyncing}
			<Sync size={16} color="#ffffff" />
			<span class="animate-pulse">Syncing...</span>
		{:else if syncStatus === 'success'}
			<Tick size={16} color="#ffffff" />
			Synced!
		{:else if syncStatus === 'error'}
			<span>✕</span>
			Failed
		{:else}
			<Sync size={16} color="#ffffff" />
			Sync Resource
		{/if}
	</button>
{/if}
