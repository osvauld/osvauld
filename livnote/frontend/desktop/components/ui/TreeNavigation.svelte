<script lang="ts">
	import { dataState, uiState } from "../../state";
	import TreeFolder from "./TreeFolder.svelte";

	// Track the last note ID we auto-expanded for
	let lastAutoExpandedNoteId = $state<string | null>(null);

	// Get regular folders (excluding "all" folder)
	const regularFolders = $derived(
		dataState.vaults.filter((v) => v.id !== "all"),
	);

	// Auto-expand the folder that contains the currently opened note
	// Only expand when switching to a different note, not continuously
	$effect(() => {
		const currentNoteId = dataState.currentNoteId;

		// Reset tracking when note is closed
		if (!currentNoteId) {
			lastAutoExpandedNoteId = null;
			return;
		}

		// Only auto-expand when the note ID actually changes to a different note
		if (currentNoteId === lastAutoExpandedNoteId) return;

		const note = dataState.getNoteById(currentNoteId);
		const folderId = note?.folderId;
		if (!folderId || folderId === "all") return;

		// Expand the note's folder using global state
		uiState.expandFolder(folderId);
		lastAutoExpandedNoteId = currentNoteId;
	});

	function toggleFolder(folderId: string) {
		uiState.toggleFolderExpansion(folderId);
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
	{#each regularFolders as folder (folder.id)}
		<TreeFolder
			{folder}
			isExpanded={uiState.isFolderExpanded(folder.id)}
			onToggle={() => toggleFolder(folder.id)}
			onSelect={() => handleFolderSelect(folder)}
			isSelected={dataState.currentVault.id === folder.id}
			level={0}
		/>
	{/each}
</div>
