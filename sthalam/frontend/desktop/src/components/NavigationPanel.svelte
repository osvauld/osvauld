<script lang="ts">
	import { dataState, uiState } from "../state";
	import WebsiteFolder from "./WebsiteFolder.svelte";
	import AddSovereignNodeModal from "./AddSovereignNodeModal.svelte";
	import PublishButton from "./PublishButton.svelte";
	import ModeSwitcher from "./ModeSwitcher.svelte";
	import SaveButton from "./SaveButton.svelte";
	import NavigationToggle from "./NavigationToggle.svelte";
	import { Add } from "@osvauld/icons";

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
		class="w-[17rem] shrink-0 h-full pt-2 pb-2 px-1 relative border-r border-osvauld-borderColor flex flex-col overflow-hidden"
		aria-label="Main Navigation"
	>
		<!-- Mode Switcher & Navigation Toggle -->
		<div class="px-2 shrink-0 mb-3 space-y-2">
			<div class="flex items-center gap-2">
				<div class="flex-1">
					<ModeSwitcher />
				</div>
				<NavigationToggle />
			</div>
			<!-- Save Button -->
			<SaveButton />
		</div>

		<!-- Header with All Websites -->
		<div class="px-1 shrink-0 mb-2 space-y-2">
			<!-- All Websites Section -->
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

			<!-- Sovereign Node Button -->
			<button
				class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive transition-colors duration-150"
				onclick={() => uiState.showSovereignNodeModalFn()}
				title="Add sovereign node"
			>
				<span class="shrink-0 text-base">🌐</span>
				<span class="flex-1 text-left text-sm font-normal">
					{dataState.sovereignNodeId ? "Sovereign Node ✓" : "Add Sovereign Node"}
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

		<!-- Publish Button Section -->
		<div class="shrink-0 border-t border-osvauld-borderColor pt-2">
			<PublishButton />
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
						<Add size={12} />
					</span>
					<span class="text-sm font-normal">Create Website</span>
				</button>
			{/if}
		</div>

	</nav>

	<!-- Sovereign Node Modal -->
	{#if uiState.showSovereignNodeModal}
		<AddSovereignNodeModal />
	{/if}
{/if}
