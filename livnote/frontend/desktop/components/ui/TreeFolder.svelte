<script lang="ts">
	import { MobileHome, FolderIcon, MobileNote, GoBack } from "../../icons";
	import { dataState } from "../../state";
	import TreeNote from "./TreeNote.svelte";
	import InlineCreateNote from "./InlineCreateNote.svelte";

	interface Props {
		folder: any;
		isExpanded: boolean;
		onToggle: () => void;
		onSelect: () => void;
		isSelected: boolean;
		level: number;
	}

	let { folder, isExpanded, onToggle, onSelect, isSelected, level }: Props =
		$props();

	let showCreateNote = $state(false);
	let hoveredNote = $state<string | null>(null);

	// Get actual count of notes for this folder (for badge display)
	const folderNoteCount = $derived(() => {
		if (folder.id === "all") {
			// For All Notes folder, count all notes across all folders
			return dataState.notes.length;
		}
		// For specific folders, count notes that belong to this folder
		return dataState.notes.filter((note) => note.folderId === folder.id).length;
	});

	// Get notes to display when folder is expanded (respects current filtering)
	const folderNotes = $derived(() => {
		if (folder.id === "all") {
			return dataState.filteredNotes;
		}
		return dataState.filteredNotes.filter(
			(note) => note.folderId === folder.id,
		);
	});

	function handleKeyDown(event: KeyboardEvent) {
		// Stop event propagation to prevent interference with nested interactive elements
		event.stopPropagation();

		if (event.key === "Enter" || event.key === " ") {
			event.preventDefault();
			if (folder.id === "all") {
				onSelect();
			} else {
				onToggle();
				onSelect();
			}
		} else if (
			event.key === "ArrowRight" &&
			!isExpanded &&
			folder.id !== "all"
		) {
			event.preventDefault();
			onToggle();
		} else if (event.key === "ArrowLeft" && isExpanded && folder.id !== "all") {
			event.preventDefault();
			onToggle();
		}
	}

	function handleNoteSelect(note: any) {
		dataState.switchNote(note.id);
	}

	function handleCreateNoteClick() {
		// Ensure folder is selected before creating note
		if (!isSelected) {
			onSelect();
		}
		// Auto-expand folder to show the create note input
		if (!isExpanded) {
			onToggle();
		}
		showCreateNote = true;
	}

	function handleCreateNoteCancel() {
		showCreateNote = false;
	}

	function handleCreateNoteComplete() {
		showCreateNote = false;
	}

	// Calculate indentation based on level
	const indentStyle = $derived(`padding-left: ${level * 1.5}rem`);
</script>

<div
	class="select-none"
	role="treeitem"
	aria-expanded={isExpanded}
	aria-selected={isSelected}
	style={indentStyle}
>
	<!-- Folder header -->
	<div
		class="flex items-center group {isSelected
			? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
			: 'text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive'} rounded-lg p-3"
	>
		<div
			class="flex-1 flex items-center gap-3 rounded-lg transition-colors duration-150 focus:outline-none
				"
			role="button"
			tabindex="0"
			onclick={() => {
				// All Notes folder only selects, doesn't toggle expansion
				if (folder.id === "all") {
					onSelect();
				} else {
					onToggle();
					onSelect();
				}
			}}
			onkeydown={handleKeyDown}
			aria-label="Select {folder.name} folder"
		>
			<!-- Expand/collapse chevron - Hide for All Notes folder -->
			{#if folder.id !== "all"}
				<span
					class="shrink-0 w-4 h-4 flex items-center justify-center transition-transform duration-75 {isExpanded
						? '-rotate-90'
						: 'rotate-180'}"
					aria-hidden="true"
				>
					<GoBack color={isSelected ? "#F2F2F0" : "#85889C"} />
				</span>
			{/if}

			<!-- Folder icon -->
			<span class="shrink-0">
				{#if folder.id === "all"}
					<MobileHome color={isSelected ? "#F2F2F0" : "#85889C"} />
				{:else}
					<FolderIcon color={isSelected ? "#F2F2F0" : "#85889C"} />
				{/if}
			</span>

			<!-- Folder name -->
			<span class="flex-1 truncate text-left font-light">
				{folder.id === "all" ? "All Notes" : folder.name}
			</span>

			<!-- Folder actions (visible when selected or hovered) - Only show for actual folders, not All Notes -->
			{#if folder.id !== "all"}
				<div
					class="transition-opacity duration-150 ml-1 {isSelected
						? 'opacity-100'
						: 'opacity-0 group-hover:opacity-100'}"
				>
					<button
						class="p-1.5 rounded-md transition-colors duration-150 focus:outline-none cursor-pointer {isSelected
							? 'text-osvauld-sideListTextActive hover:bg-osvauld-modalFieldActive'
							: 'text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive'}"
						onclick={(e) => {
							e.stopPropagation();
							handleCreateNoteClick();
						}}
						aria-label="Create new note in {folder.name}"
						title="Create new note"
					>
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
					</button>
				</div>
			{/if}
			<!-- Note count badge -->
			{#if folderNoteCount() > 0}
				<span
					class="shrink-0 text-xs text-osvauld-fieldText"
					aria-label="{folderNoteCount()} notes"
				>
					{folderNoteCount()}
				</span>
			{/if}
		</div>
	</div>

	<!-- Folder contents (notes) - Only show for regular folders, not All Notes -->
	{#if isExpanded && folder.id !== "all"}
		<div
			class="ml-6 border-l border-osvauld-borderColor"
			role="group"
			aria-label="{folder.name} contents"
		>
			<!-- Inline note creation - Show at top of list -->
			{#if showCreateNote}
				<div class="pl-3">
					<InlineCreateNote
						folderId={folder.id}
						onCancel={handleCreateNoteCancel}
						onComplete={handleCreateNoteComplete}
					/>
				</div>
			{/if}

			<!-- Notes list -->
			{#if dataState.isDataLoading}
				<div class="p-3 text-osvauld-fieldText text-sm">Loading notes...</div>
			{:else if folderNotes().length === 0 && !showCreateNote}
				<div class="p-3 text-osvauld-fieldText text-sm opacity-60">
					No notes in this folder
				</div>
			{:else}
				{#each folderNotes() as note (note.id)}
					<TreeNote
						{note}
						isSelected={dataState.currentNoteId === note.id}
						onSelect={() => handleNoteSelect(note)}
						onHover={(noteId) => (hoveredNote = noteId)}
						onHoverLeave={() => (hoveredNote = null)}
						isHovered={hoveredNote === note.id}
					/>
				{/each}
			{/if}
		</div>
	{/if}
</div>
