<script lang="ts">
	import { dataState } from "../state/data.svelte";
	import PublishWebsiteModal from "./PublishWebsiteModal.svelte";

	let showPublishModal = $state(false);

	const handlePublishClick = () => {
		const currentWebsite = dataState.currentWebsite;
		if (!currentWebsite || currentWebsite.id === "all") {
			console.error("No website selected or 'All Websites' is selected");
			return;
		}
		showPublishModal = true;
	};

	// Show button only if a specific website is selected (not "All Websites")
	const shouldShow = $derived(
		!!dataState.currentWebsite && dataState.currentWebsite.id !== "all"
	);
</script>

{#if shouldShow}
	<div class="publish-button-container">
		<button
			onclick={handlePublishClick}
			class="px-4 py-2 bg-livnotePink text-osvauld-frameblack rounded-lg font-medium text-sm hover:bg-opacity-90 transition-all flex items-center gap-2"
			title="Publish website"
		>
			<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
				<path d="M10 12a2 2 0 100-4 2 2 0 000 4z" />
				<path fill-rule="evenodd" d="M.458 10C1.732 5.943 5.522 3 10 3s8.268 2.943 9.542 7c-1.274 4.057-5.064 7-9.542 7S1.732 14.057.458 10zM14 10a4 4 0 11-8 0 4 4 0 018 0z" clip-rule="evenodd" />
			</svg>
			Publish
		</button>
	</div>

	<!-- Publish Website Modal -->
	<PublishWebsiteModal bind:show={showPublishModal} onClose={() => (showPublishModal = false)} />
{/if}

<style>
	.publish-button-container {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0.5rem;
	}
</style>
