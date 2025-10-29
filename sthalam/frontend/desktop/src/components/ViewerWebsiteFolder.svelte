<script lang="ts">
	import { dataState, uiState } from "../state";
	import ResourceItem from "./ResourceItem.svelte";
	import type { Website } from "../types";
	import { RightArrow } from "@osvauld/icons";
	import { sendMessage } from "../utils/helper";

	interface Props {
		website: Website;
		isExpanded: boolean;
		onToggle: () => void;
		onSelect: () => void;
		isSelected: boolean;
	}

	let { website, isExpanded, onToggle, onSelect, isSelected }: Props =
		$props();

	let isSyncing = $state(false);

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

	async function handleFolderSync(e: Event) {
		e.stopPropagation();
		isSyncing = true;
		try {
			await sendMessage('folderSyncViewer', { folderId: website.id });
			console.log('Successfully triggered folder sync for:', website.name);
		} catch (error) {
			console.error('Failed to sync folder:', error);
		} finally {
			isSyncing = false;
		}
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

		<!-- Sync button -->
		<button
			class="sync-btn"
			onclick={handleFolderSync}
			disabled={isSyncing}
			title="Sync folder"
		>
			{#if isSyncing}
				<span class="sync-spinner"></span>
			{:else}
				↻
			{/if}
		</button>
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

<style>
	.sync-btn {
		width: 1.75rem;
		height: 1.75rem;
		border-radius: 4px;
		background: transparent;
		border: 1px solid #8A86E5;
		color: #8A86E5;
		font-size: 1rem;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all 0.2s;
		flex-shrink: 0;
	}

	.sync-btn:hover:not(:disabled) {
		background: #8A86E5;
		color: #16171f;
		transform: rotate(180deg);
	}

	.sync-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.sync-spinner {
		width: 12px;
		height: 12px;
		border: 2px solid rgba(138, 134, 229, 0.3);
		border-top-color: #8A86E5;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
