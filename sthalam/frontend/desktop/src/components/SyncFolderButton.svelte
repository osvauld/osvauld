<script lang="ts">
	import { dataState } from "../state";
	import { sendMessage } from "../utils/helper";
	import { Sync, Tick } from "@osvauld/icons";

	let isSyncing = $state(false);
	let syncStatus = $state<'idle' | 'success' | 'error'>('idle');
	let errorMessage = $state('');

	const handleSync = async () => {
		const folderId = dataState.currentWebsite?.id;

		if (!folderId || folderId === 'all') {
			console.error("No folder selected");
			return;
		}

		isSyncing = true;
		syncStatus = 'idle';
		errorMessage = '';

		try {
			console.log("🔄 Syncing folder:", folderId);

			// Call backend to request folder resources (backend will determine the node)
			await sendMessage("requestFolderResources", {
				folderId: folderId,
			});

			console.log("✅ Folder sync requested successfully");
			syncStatus = 'success';

			// Reset status after 2 seconds
			setTimeout(() => {
				syncStatus = 'idle';
			}, 2000);
		} catch (error) {
			console.error("❌ Failed to sync folder:", error);
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

	// Show when a folder is selected (not "All Websites")
	const shouldShow = $derived(
		!!dataState.currentWebsite?.id &&
		dataState.currentWebsite.id !== 'all'
	);
</script>

{#if shouldShow}
	<button
		onclick={handleSync}
		disabled={isSyncing}
		class="w-full px-4 py-2 rounded-lg font-medium text-sm transition-all flex items-center justify-center gap-2"
		class:bg-green-600={syncStatus === 'success'}
		class:bg-red-600={syncStatus === 'error'}
		class:bg-purple-600={syncStatus === 'idle' && !isSyncing}
		class:bg-gray-600={isSyncing}
		class:text-white={true}
		class:opacity-70={isSyncing}
		class:cursor-not-allowed={isSyncing}
		title={syncStatus === 'error' ? errorMessage : 'Sync folder from sovereign node'}
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
			Sync Folder
		{/if}
	</button>
{/if}
