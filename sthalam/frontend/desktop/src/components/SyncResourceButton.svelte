<script lang="ts">
	import { dataState } from "../state/data.svelte";
	import { sendMessage } from "../utils/helper";

	let isSyncing = $state(false);
	let isPublished = $state(false);

	// Check if current folder is already published to sovereign node
	$effect(() => {
		const checkPublished = async () => {
			const currentWebsite = dataState.currentWebsite;
			const sovereignNodeId = dataState.getSovereignNodeId();

			if (!currentWebsite || currentWebsite.id === "all" || !sovereignNodeId) {
				isPublished = false;
				return;
			}

			try {
				// Check if folder is shared with sovereign node
				const sharedUsers = await sendMessage("getSharedFolderUsers", {
					folderId: currentWebsite.id
				});
				isPublished = sharedUsers?.some((user: any) => user.id === sovereignNodeId) || false;
			} catch (error) {
				console.error("Error checking if folder is published:", error);
				isPublished = false;
			}
		};

		checkPublished();
	});

	const handleSync = async () => {
		// Get current website/folder
		const currentWebsite = dataState.currentWebsite;
		if (!currentWebsite || currentWebsite.id === "all") {
			console.error("No website selected");
			return;
		}

		// Get all resources in current folder
		const resources = dataState.filteredResources;
		if (resources.length === 0) {
			console.warn("No resources to sync in this folder");
			return;
		}

		isSyncing = true;
		try {
			console.log("🔄 Syncing resource updates:", resources.length, "resources");

			// Sync each resource
			for (const resource of resources) {
				await sendMessage("publishResource", { resourceId: resource.id });
				console.log("✅ Synced resource:", resource.id);
			}

			console.log("✅ All resources synced successfully!");
			// TODO: Show success toast
		} catch (error) {
			console.error("❌ Failed to sync resources:", error);
			// TODO: Show error toast
		} finally {
			isSyncing = false;
		}
	};

	// Show button only if folder is already published to sovereign node
	const shouldShow = $derived(
		!!dataState.currentWebsite &&
		dataState.currentWebsite.id !== "all" &&
		!!dataState.getSovereignNodeId() &&
		isPublished
	);
</script>

{#if shouldShow}
	<button
		onclick={handleSync}
		disabled={isSyncing}
		class="px-4 py-2 bg-livnotePink text-osvauld-frameblack rounded-lg font-medium text-sm hover:bg-opacity-90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
		title="Sync resource updates"
	>
		{#if isSyncing}
			<svg class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
				<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
				<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
			</svg>
			Syncing...
		{:else}
			<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
				<path fill-rule="evenodd" d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z" clip-rule="evenodd" />
			</svg>
			Sync Updates
		{/if}
	</button>
{/if}
