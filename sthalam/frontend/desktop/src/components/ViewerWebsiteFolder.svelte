<script lang="ts">
	import { dataState, uiState } from "../state";
	import ResourceItem from "./ResourceItem.svelte";
	import type { Website } from "../types";
	import { RightArrow } from "@osvauld/icons";

	interface Props {
		website: Website;
		isExpanded: boolean;
		onToggle: () => void;
		onSelect: () => void;
		isSelected: boolean;
	}

	let { website, isExpanded, onToggle, onSelect, isSelected }: Props =
		$props();

	// Get resources for this website
	const websiteResources = $derived(() => {
		const filtered = dataState.resources.filter((r) => r.websiteId === website.id);
		return filtered;
	});

	// Get count of resources
	const resourceCount = $derived(() => {
		return websiteResources().length;
	});

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
