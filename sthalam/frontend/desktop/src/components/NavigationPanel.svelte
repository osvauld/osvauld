<script lang="ts">
	import { dataState, uiState } from "../state";
	import WebsiteFolder from "./WebsiteFolder.svelte";

	let showCreateWebsite = $state(false);
	let websiteName = $state("");

	// Get regular websites (excluding "all")
	const websites = $derived(
		dataState.websites.filter((w) => w.id !== "all"),
	);

	async function handleCreateWebsite() {
		if (!websiteName.trim()) return;

		try {
			await dataState.addWebsite(websiteName);
			websiteName = "";
			showCreateWebsite = false;
		} catch (error) {
			console.error("Failed to create website:", error);
		}
	}

	function handleCancelCreate() {
		websiteName = "";
		showCreateWebsite = false;
	}
</script>

{#if uiState.showNavigationPanel}
	<nav
		class="w-[17rem] shrink-0 h-full pt-4 pb-1 px-1 relative border-r border-osvauld-borderColor flex flex-col"
		aria-label="Main Navigation"
	>
		<!-- All Websites Section -->
		<div class="px-1 shrink-0 mb-2">
			<button
				class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-textActive hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive transition-colors duration-150"
				class:text-osvauld-sideListTextActive={dataState.currentWebsite.id ===
					"all"}
				class:bg-osvauld-fieldActive={dataState.currentWebsite.id === "all"}
				onclick={() =>
					dataState.switchWebsite({ id: "all", name: "All Websites" })}
			>
				<span class="shrink-0 text-lg">🌐</span>
				<span class="flex-1 text-left text-sm font-normal">All Websites</span>
				<span class="shrink-0 text-xs text-textActive">
					{websites.length}
				</span>
			</button>
		</div>

		<!-- Websites Tree -->
		<div class="flex-1 overflow-y-auto py-1 scrollbar-thin min-h-0">
			<div class="flex flex-col gap-1 px-1">
				{#each websites as website (website.id)}
					<WebsiteFolder
						{website}
						isExpanded={uiState.isFolderExpanded(website.id)}
						onToggle={() => uiState.toggleFolderExpansion(website.id)}
						onSelect={() => dataState.switchWebsite(website)}
						isSelected={dataState.currentWebsite.id === website.id}
					/>
				{/each}
			</div>
		</div>

		<!-- Create Website Section -->
		<div class="shrink-0 py-2 border-t border-osvauld-borderColor">
			{#if showCreateWebsite}
				<div class="px-2">
					<input
						type="text"
						bind:value={websiteName}
						placeholder="Website name"
						class="w-full px-3 py-2 text-sm bg-osvauld-fieldActive text-white border border-osvauld-borderColor rounded-lg focus:outline-none focus:ring-1 focus:ring-livnotePink"
						onkeydown={(e) => {
							if (e.key === "Enter") handleCreateWebsite();
							if (e.key === "Escape") handleCancelCreate();
						}}
						autofocus
					/>
					<div class="flex gap-2 mt-2">
						<button
							class="flex-1 px-3 py-1.5 text-sm bg-livnotePink text-black rounded-lg hover:bg-opacity-90"
							onclick={handleCreateWebsite}
						>
							Create
						</button>
						<button
							class="flex-1 px-3 py-1.5 text-sm border border-osvauld-borderColor text-osvauld-fieldText rounded-lg hover:text-white"
							onclick={handleCancelCreate}
						>
							Cancel
						</button>
					</div>
				</div>
			{:else}
				<button
					class="w-5/6 mx-auto flex items-center gap-3 px-3 py-2 rounded-lg text-textActive hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive transition-colors duration-150"
					onclick={() => (showCreateWebsite = true)}
				>
					<span class="shrink-0 w-4 h-4 flex items-center justify-center">
						<svg
							class="w-3 h-3"
							fill="currentColor"
							viewBox="0 0 12 12"
							aria-hidden="true"
						>
							<path
								d="M6 1a1 1 0 011 1v3h3a1 1 0 110 2H7v3a1 1 0 11-2 0V7H2a1 1 0 110-2h3V2a1 1 0 011-1z"
							></path>
						</svg>
					</span>
					<span class="text-sm font-normal">Create Website</span>
				</button>
			{/if}
		</div>
	</nav>
{/if}
