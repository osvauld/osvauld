<script lang="ts">
	import { dataState, uiState } from "../../state";
	import TreeFolder from "./TreeFolder.svelte";

	// Track expanded folders in component state
	let expandedFolders = $state<Set<string>>(new Set());

	// Get regular folders (excluding "all" folder)
	const regularFolders = $derived(() => {
		return dataState.vaults.filter((v) => v.id !== "all");
	});

	// Auto-expand the folder that contains the currently opened note
	$effect(() => {
		const currentNoteId = dataState.currentNoteId;
		if (!currentNoteId) return;

		const note = dataState.getNoteById(currentNoteId);
		const folderId = note?.folderId;
		if (!folderId || folderId === "all") return;

		if (!expandedFolders.has(folderId)) {
			// Expand the note's folder without collapsing others
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
			// Expand the folder without affecting others
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
</script>

<div
	class="flex flex-col gap-1 px-1 h-full overflow-y-auto scrollbar-thin"
	role="tree"
	aria-label="Folders navigation"
>
	<!-- Regular folders -->
	{#each regularFolders() as folder (folder.id)}
		<TreeFolder
			{folder}
			isExpanded={expandedFolders.has(folder.id)}
			onToggle={() => toggleFolder(folder.id)}
			onSelect={() => handleFolderSelect(folder)}
			isSelected={dataState.currentVault.id === folder.id}
			level={0}
		/>
	{/each}
</div>
