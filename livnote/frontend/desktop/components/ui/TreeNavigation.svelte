<script lang="ts">
	import { dataState, uiState } from "../../state";
	import TreeFolder from "./TreeFolder.svelte";
	import InlineCreateFolder from "./InlineCreateFolder.svelte";
	import { MobileHome } from "../../icons";

	// Track expanded folders in component state - All Notes folder should not be expandable
	let expandedFolders = $state<Set<string>>(new Set());
	let showCreateFolder = $state(false);

	// Derived state for organizing folders
	const organizedFolders = $derived(() => {
		// Separate "all" folder and regular folders
		const allFolder = dataState.vaults.find((v) => v.id === "all");
		const regularFolders = dataState.vaults.filter((v) => v.id !== "all");

		return {
			allFolder,
			regularFolders,
		};
	});

	// Auto-expand the folder that contains the currently opened note
	$effect(() => {
		const currentNoteId = dataState.currentNoteId;
		if (!currentNoteId) return;

		const note = dataState.getNoteById(currentNoteId);
		const folderId = note?.folderId;
		if (!folderId || folderId === "all") return;

		if (!expandedFolders.has(folderId)) {
			// Accordion behavior: expand the note's folder and collapse others
			expandedFolders.clear();
			expandedFolders.add(folderId);
			expandedFolders = new Set(expandedFolders);
		}
	});

	function toggleFolder(folderId: string) {
		// Don't allow All Notes folder to be toggled
		if (folderId === "all") {
			return;
		}

		if (expandedFolders.has(folderId)) {
			// Collapse the folder if it's already expanded
			expandedFolders.delete(folderId);
		} else {
			// Accordion behavior: collapse all other folders and expand this one
			expandedFolders.clear();
			expandedFolders.add(folderId);
		}
		// Trigger reactivity
		expandedFolders = new Set(expandedFolders);
	}

	async function handleFolderSelect(folder: any) {
		await dataState.switchVault(folder);

		// Clear current note selection when switching folders to show list view
		if (dataState.currentNoteId) {
			dataState.clearCurrentNote();
		}
	}

	function handleCreateFolderClick() {
		showCreateFolder = true;
	}

	function handleCreateFolderCancel() {
		showCreateFolder = false;
	}

	function handleCreateFolderComplete() {
		showCreateFolder = false;
	}
</script>

<div
	class="flex flex-col gap-1 h-full max-h-full overflow-y-auto scrollbar-thin"
	role="tree"
	aria-label="Folder and notes navigation"
>
	<!-- All Notes folder -->
	{#if organizedFolders().allFolder}
		<TreeFolder
			folder={organizedFolders().allFolder}
			isExpanded={false}
			onToggle={() => toggleFolder("all")}
			onSelect={() => handleFolderSelect(organizedFolders().allFolder)}
			isSelected={dataState.currentVault.id === "all"}
			level={0}
		/>
	{/if}

	<!-- Regular folders -->
	{#each organizedFolders().regularFolders as folder (folder.id)}
		<TreeFolder
			{folder}
			isExpanded={expandedFolders.has(folder.id)}
			onToggle={() => toggleFolder(folder.id)}
			onSelect={() => handleFolderSelect(folder)}
			isSelected={dataState.currentVault.id === folder.id}
			level={0}
		/>
	{/each}

	<!-- Create new folder section -->
	<div class="mt-auto pt-2 border-t border-osvauld-borderColor">
		{#if showCreateFolder}
			<InlineCreateFolder
				onCancel={handleCreateFolderCancel}
				onComplete={handleCreateFolderComplete}
			/>
		{:else}
			<button
				class="w-5/6 flex items-center gap-3 p-3 rounded-lg text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive transition-colors duration-150 focus:outline-none focus:ring-2 focus:ring-livnotelavender focus:ring-offset-2 focus:ring-offset-osvauld-ninjablack"
				onclick={handleCreateFolderClick}
				aria-label="Create new folder"
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
				<span class="text-sm font-light">Create new folder</span>
			</button>
		{/if}
	</div>
</div>
