<script lang="ts">
	import { dataState } from "../state";
	import ResourceItem from "./ResourceItem.svelte";
	import type { Website } from "../types";
	import RightArrow from "../lib/icons/src/icons/rightArrow.svelte";
	import Add from "../lib/icons/src/icons/add.svelte";

	interface Props {
		website: Website;
		isExpanded: boolean;
		onToggle: () => void;
		onSelect: () => void;
		isSelected: boolean;
	}

	let { website, isExpanded, onToggle, onSelect, isSelected }: Props =
		$props();

	let isCreatingResource = $state(false);

	// Get resources for this website
	const websiteResources = $derived(() => {
		const filtered = dataState.resources.filter((r) => r.websiteId === website.id);
		return filtered;
	});

	// Get count of resources
	const resourceCount = $derived(() => {
		return websiteResources().length;
	});

	async function handleCreateResource() {
		if (isCreatingResource) return;

		// Ensure website is selected
		if (!isSelected) {
			onSelect();
		}

		// Auto-expand to show the new resource
		if (!isExpanded) {
			onToggle();
		}

		isCreatingResource = true;

		try {
			await dataState.addResource(website.id, "Untitled Page");
		} catch (error) {
			console.error("Failed to create resource:", error);
		} finally {
			isCreatingResource = false;
		}
	}

	function handleResourceSelect(resource: any) {
		dataState.switchResource(resource.id);
	}

	function handleFolderClick() {
		onToggle();
		onSelect();
	}
</script>

<div class="select-none">
	<!-- Website header -->
	<div
		class="flex items-center group rounded-lg p-2"
		class:text-osvauld-sideListTextActive={isSelected}
		class:bg-osvauld-fieldActive={isSelected}
		class:text-textActive={!isSelected}
		class:hover:text-osvauld-sideListTextActive={!isSelected}
		class:hover:bg-osvauld-fieldActive={!isSelected}
	>
		<div
			class="flex-1 flex items-center gap-1.5 rounded-lg transition-colors duration-150 min-w-0 cursor-pointer"
			role="button"
			tabindex="0"
			onclick={handleFolderClick}
			onkeydown={(e) => {
				if (e.key === "Enter" || e.key === " ") {
					e.preventDefault();
					handleFolderClick();
				}
			}}
		>
			<!-- Expand/collapse chevron -->
			<span
				class="shrink-0 w-4 h-4 flex items-center justify-center transition-transform duration-200"
				class:rotate-90={isExpanded}
			>
				<RightArrow color={isSelected ? "#F2F2F0" : "currentColor"} size={16} />
			</span>

			<!-- Website icon -->
			<span class="shrink-0 text-base">🌐</span>

			<!-- Website name -->
			<span
				class="flex-1 text-left text-sm font-normal min-w-0 overflow-hidden text-ellipsis whitespace-nowrap"
				title={website.name}
			>
				{website.name}
			</span>

			<!-- Actions -->
			<div
				class="transition-opacity duration-150 ml-1"
				class:opacity-100={isSelected}
				class:opacity-0={!isSelected}
				class:group-hover:opacity-100={!isSelected}
			>
				<button
					type="button"
					class="p-1.5 rounded-md transition-colors duration-150 cursor-pointer disabled:opacity-50"
					class:text-osvauld-sideListTextActive={isSelected}
					class:hover:bg-osvauld-modalFieldActive={isSelected}
					class:text-osvauld-fieldText={!isSelected}
					class:hover:text-osvauld-sideListTextActive={!isSelected}
					class:hover:bg-osvauld-fieldActive={!isSelected}
					onclick={(e) => {
						e.stopPropagation();
						handleCreateResource();
					}}
					disabled={isCreatingResource}
					title="Create new page"
				>
					<Add color="currentColor" size={16} />
				</button>
			</div>

			<!-- Resource count -->
			<span class="shrink-0 text-xs text-textActive">
				{resourceCount()}
			</span>
		</div>
	</div>

	<!-- Resources list -->
	{#if isExpanded}
		<div class="ml-3.5 border-l border-osvauld-borderColor">
			{#if dataState.isDataLoading}
				<div class="p-3 text-osvauld-fieldText text-sm">Loading pages...</div>
			{:else if websiteResources().length === 0}
				<div class="p-3 text-osvauld-fieldText text-sm opacity-60">
					No pages in this website
				</div>
			{:else}
				{#each websiteResources() as resource (resource.id)}
					<ResourceItem
						{resource}
						isSelected={dataState.currentResourceId === resource.id}
						onSelect={() => handleResourceSelect(resource)}
					/>
				{/each}
			{/if}
		</div>
	{/if}
</div>
