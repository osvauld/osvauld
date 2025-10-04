<script lang="ts">
	import { onDestroy } from "svelte";
	import { MobileHome, FolderIcon, RightArrow, Add } from "@osvauld/icons";
	import { dataState, uiState } from "../../state";
	import { createEmptyNoteContent } from "../notes/documentUtils";
	import { sendMessage } from "../../utils/helper";
	import TreeNote from "./TreeNote.svelte";

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

	let hoveredNote = $state<string | null>(null);
	let isCreatingNote = $state(false);
	let clickTimer: number | null = null;

	// Get actual count of notes for this folder (for badge display)
	const folderNoteCount = $derived(() => {
		if (folder.id === "all") {
			// For All Notes folder, count all notes across all folders
			return dataState.notes.length;
		}
		// For specific folders, count notes that belong to this folder
		return dataState.notes.filter((note) => note.folderId === folder.id).length;
	});

	// Get notes to display when folder is expanded (shows all notes for this specific folder)
	const folderNotes = $derived(() => {
		if (folder.id === "all") {
			return dataState.filteredNotes;
		}
		// For individual folders, filter from all notes, not just the current vault's filtered notes
		return dataState.notes.filter((note) => note.folderId === folder.id);
	});

	function handleKeyDown(event: KeyboardEvent) {
		// Stop event propagation to prevent interference with nested interactive elements
		event.stopPropagation();

		if (event.key === "Enter" || event.key === " ") {
			event.preventDefault();
			if (folder.id === "all") {
				onSelect();
			} else {
				// Keyboard navigation always switches folder (like double click)
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

	function handleFolderClick() {
		// All Notes folder always selects immediately
		if (folder.id === "all") {
			onSelect();
			return;
		}

		// When viewing note list (noteViewLayout is false), single click switches folder immediately
		if (!uiState.noteViewLayout) {
			onToggle();
			onSelect();
			return;
		}

		// When editing a note (noteViewLayout is true), delay single click to allow double click detection
		// Clear any existing timer
		if (clickTimer) {
			clearTimeout(clickTimer);
		}

		clickTimer = window.setTimeout(() => {
			// Single click: only toggle expand/collapse
			onToggle();
			clickTimer = null;
		}, 50); // 250ms delay to detect double click
	}

	function handleFolderDoubleClick() {
		// Clear the single click timer
		if (clickTimer) {
			clearTimeout(clickTimer);
			clickTimer = null;
		}

		// All Notes folder - just select
		if (folder.id === "all") {
			onSelect();
			return;
		}

		// Double click: toggle and switch folder
		onToggle();
		onSelect();
	}

	async function handleCreateNoteClick() {
		if (isCreatingNote) return;

		// Ensure folder is selected before creating note
		if (!isSelected) {
			onSelect();
		}
		// Auto-expand folder to show the new note once created
		if (!isExpanded) {
			onToggle();
		}

		isCreatingNote = true;

		try {
			// Create note with default "Untitled note" title
			const noteContent = createEmptyNoteContent(
				dataState.clientId,
				dataState.userDetails?.username,
				"Untitled note",
			);

			const note = await sendMessage("addCredential", {
				resourcePayload: JSON.stringify(noteContent),
				folderId: folder.id === "all" ? dataState.currentVault.id : folder.id,
				resourceType: "notes",
			});

			// Switch to the new note using switchNote to ensure folder highlighting is correct
			await dataState.switchNote(note.id);
		} catch (error) {
			console.error("Failed to create note:", error);
			// Could show error toast here
		} finally {
			isCreatingNote = false;
		}
	}

	// Calculate indentation based on level
	const indentStyle = $derived(`padding-left: ${level * 1.5}rem`);

	// Cleanup timer on component unmount
	onDestroy(() => {
		if (clickTimer) {
			clearTimeout(clickTimer);
		}
	});
</script>

<div class="select-none" style={indentStyle}>
	<!-- Folder header -->
	<div
		class="flex items-center group {isSelected
			? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
			: 'text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive'} rounded-lg p-2"
	>
		<div
			class="flex-1 flex items-center gap-1.5 rounded-lg transition-colors duration-150"
			role="treeitem"
			aria-expanded={folder.id !== "all" ? isExpanded : undefined}
			aria-selected={isSelected}
			tabindex="0"
			onclick={handleFolderClick}
			ondblclick={handleFolderDoubleClick}
			onkeydown={handleKeyDown}
			aria-label="Select {folder.name} folder"
		>
			<!-- Expand/collapse chevron - Hide for All Notes folder -->
			{#if folder.id !== "all"}
				<span
					class="shrink-0 w-4 h-4 flex items-center justify-center transition-transform duration-25 {isExpanded
						? 'rotate-90'
						: ''}"
					aria-hidden="true"
				>
					<RightArrow
						color={isSelected ? "#F2F2F0" : "currentColor"}
						size={16}
					/>
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
			<span
				class="flex-1 truncate text-left text-sm font-light select-none cursor-default"
			>
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
						type="button"
						class="p-1.5 rounded-md transition-colors duration-150 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed {isSelected
							? 'text-osvauld-sideListTextActive hover:bg-osvauld-modalFieldActive'
							: 'text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive'}"
						onclick={(e) => {
							e.stopPropagation();
							handleCreateNoteClick();
						}}
						disabled={isCreatingNote}
						aria-label="Create new note in {folder.name}"
						title="Create new note"
					>
						<Add color="currentColor" size={16} />
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
			class="ml-3.5 border-l border-osvauld-borderColor"
			role="group"
			aria-label="{folder.name} contents"
		>
			<!-- Notes list -->
			{#if dataState.isDataLoading}
				<div class="p-3 text-osvauld-fieldText text-sm">Loading notes...</div>
			{:else if folderNotes().length === 0}
				<div class="p-3 text-osvauld-fieldText text-sm opacity-60">
					No notes in this folder
				</div>
			{:else}
				{#each folderNotes() as note (note.id)}
					<TreeNote
						{note}
						isSelected={dataState.currentNoteId === note.id}
						onSelect={() => handleNoteSelect(note)}
						onHover={(noteId: string) => (hoveredNote = noteId)}
						onHoverLeave={() => (hoveredNote = null)}
						isHovered={hoveredNote === note.id}
					/>
				{/each}
			{/if}
		</div>
	{/if}
</div>
