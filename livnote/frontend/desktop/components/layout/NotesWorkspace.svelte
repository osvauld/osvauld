<script lang="ts">
	import { onMount } from "svelte";
	import {
		BackArrow,
		Star as EmptyStar,
		FavStar as Star,
		MenuToggle,
	} from "../../icons";
	import NoteRightContainer from "../ui/NoteRightContainer.svelte";
	import { dataState, uiState } from "../../state";
	import RichTextEditor from "../notes/RichTextEditor.svelte";
	import NavigationPanel from "./NavigationPanel.svelte";
	import { fade } from "svelte/transition";
	import { sendMessage } from "../../utils/helper";

	// Local UI state using $state
	let newNoteTitle = $state("");
	let isEditingTitle = $state(false);
	let inputRef = $state<HTMLInputElement | null>(null);
	let userId = $state("");
	let isNavigatingBack = false;
	let collaboratorSyncInterval: number | null = null;
	// Derived state for favorite status using the reactive notes array
	let isFavourite = $derived(() => {
		const noteId = dataState.currentNoteId;
		if (!noteId) return false;
		const note = dataState.getNoteById(noteId);
		return note?.favourite ?? false;
	});

	// Title editing functions
	const startEditingTitle = () => {
		isEditingTitle = true;

		// Focus the input after the DOM updates
		setTimeout(() => {
			if (inputRef) {
				inputRef.focus();
				inputRef.select();
			}
		}, 0);
	};

	const handleBackButtonMouseDown = () => {
		isNavigatingBack = true;
	};

	const saveTitle = () => {
		if (isNavigatingBack || newNoteTitle.trim().length === 0) {
			isEditingTitle = false;
			return;
		}
		let coordinator = dataState.getNotesCoordinator();
		coordinator?.saveNote(newNoteTitle);
		dataState.currentNoteTitle = newNoteTitle;
		isEditingTitle = false;
	};

	const getInitial = (name: string): string => {
		return name.charAt(0).toUpperCase();
	};

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Enter") {
			saveTitle();
		} else if (event.key === "Escape") {
			isEditingTitle = false;
		}
	}

	// Back button handler - saves and returns to list view
	const handleBackButton = () => {
		const noteId = dataState.currentNoteId;
		if (noteId) {
			dataState.saveNote(noteId);
		}
		dataState.clearCurrentNote();
		uiState.toggleNoteViewLayout(false);
	};

	const toggleFav = async (e: Event) => {
		e.stopPropagation();
		try {
			const noteId = dataState.currentNoteId;
			if (noteId === null) return;

			await sendMessage("toggleFav", {
				resourceId: noteId,
			});
			dataState.updateNoteFavorite(noteId);
		} catch (err) {
			console.error("Error toggling favorite:", err);
			uiState.showToast("Failed to update favorite status", false);
		}
	};

	// Set up periodic collaborator syncing
	const setupCollaboratorSync = () => {
		// Clear any existing interval
		if (collaboratorSyncInterval) {
			clearInterval(collaboratorSyncInterval);
		}

		// Set up new interval to sync collaborators every 2 seconds
		collaboratorSyncInterval = setInterval(() => {
			if (dataState.currentNoteId) {
				const coordinator = dataState.getNotesCoordinator();
				if (coordinator) {
					coordinator.syncCollaborators();
				}
			}
		}, 2000);
	};

	// Clean up collaborator sync interval
	const cleanupCollaboratorSync = () => {
		if (collaboratorSyncInterval) {
			clearInterval(collaboratorSyncInterval);
			collaboratorSyncInterval = null;
		}
	};

	// Toggle navigation panel
	const toggleNavigationPanel = () => {
		uiState.toggleNavigationPanel();
		if (!uiState.showNavigationPanel) {
			// When hiding panel - set CSS var to 0 to allow editor to go under min width
			document.documentElement.style.setProperty("--min-editor-width", "0px");
		} else {
			// When showing panel - restore the min width
			document.documentElement.style.setProperty(
				"--min-editor-width",
				`${uiState.MIN_EDITOR_WIDTH}px`,
			);
		}
	};

	onMount(() => {
		if (dataState.userDetails?.userId) {
			userId = dataState.userDetails?.userId;
		}
		document.documentElement.style.setProperty(
			"--min-editor-width",
			`${uiState.MIN_EDITOR_WIDTH}px`,
		);

		// Set up collaborator syncing
		setupCollaboratorSync();

		// Clean up on unmount
		return () => {
			cleanupCollaboratorSync();
		};
	});
</script>

<style>
	/* Add styles to handle the case when navigation panel is manually toggled */
	:global(.manual-toggle) {
		--min-editor-width: 0px;
	}
</style>

<div
	class="flex grow max-h-full max-w-full"
	class:manual-toggle={uiState.isNavigationPanelManuallyToggled}
>
	<NavigationPanel />

	<div class="flex-1 flex flex-col overflow-hidden">
		<!-- Header section with back button and title -->
		<div class="py-10 px-11 flex items-center justify-start shrink-0">
			<div class="flex justify-between items-center max-w-[44rem]">
				<!-- Burger menu toggle - only show when navigation panel is hidden -->
				{#if !uiState.showNavigationPanel}
					<button
						aria-label="Open navigation panel"
						class="mr-3 rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-e-resize"
						title="Open navigation panel"
						onclick={toggleNavigationPanel}
					>
						<MenuToggle />
					</button>
				{/if}

				<button
					class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-pointer"
					onmousedown={handleBackButtonMouseDown}
					onclick={handleBackButton}
				>
					<BackArrow />
				</button>

				{#if isEditingTitle}
					<div
						class="grow mx-5 flex justify-between items-center py-1 px-3 border rounded-lg border-osvauld-iconblack"
					>
						<input
							bind:this={inputRef}
							bind:value={newNoteTitle}
							maxlength="20"
							onkeydown={handleKeydown}
							onblur={saveTitle}
							class="text-white text-4xl border-0 tracking-wider font-semibold border-transparent focus:border-osvauld-iconblack focus:outline-0 focus:ring-0 active:outline-none focus:ring-offset-0"
						/>
					</div>
				{:else}
					<span
						role="button"
						tabindex="0"
						class="grow truncate mx-5 py-2 font-semibold text-4xl text-osvauld-sideListTextActive select-none"
						ondblclick={startEditingTitle}
						onkeydown={(e: KeyboardEvent) =>
							e.key === "Enter" && startEditingTitle()}
					>
						{dataState.currentNoteTitle}
					</span>
				{/if}
				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-pointer"
					onclick={toggleFav}
				>
					{#if isFavourite()}
						<Star />
					{:else}
						<EmptyStar color="#85889C" />
					{/if}
				</button>
			</div>
			{#if dataState.collaborators.length > 0}
				<div class="ml-auto flex items-center">
					{#each dataState.collaborators.slice(0, 3) as collaborator, index (collaborator.id)}
						<div
							class="relative {index !== 0 ? '-ml-3' : ''}"
							aria-label={collaborator.name}
							title={collaborator.name}
							in:fade={{ duration: 200 }}
							out:fade={{ duration: 200 }}
						>
							<div
								class="w-12 h-12 z-10 rounded-full bg-osvauld-fieldActive text-xl font-medium text-collaboratorText border border-collaboratorBorder flex justify-center items-center relative"
							>
								{getInitial(collaborator.name)}
								<!-- Live indicator dot -->
								<div
									class="absolute bottom-0 left-0 w-3 h-3 bg-green-500 rounded-full border-2 border-osvauld-fieldActive"
									in:fade={{ duration: 200 }}
								></div>
							</div>
						</div>
					{/each}

					{#if dataState.collaborators.length > 3}
						<div class="relative -ml-3" aria-label={`+${dataState.collaborators.length - 3} more collaborators`}>
							<div
								class="w-12 h-12 -z-10 rounded-full bg-osvauld-fieldActive text-sm font-medium text-collaboratorText border border-collaboratorBorder flex justify-center items-center"
							>
								+{dataState.collaborators.length - 3}
							</div>
						</div>
					{/if}
				</div>
			{/if}
		</div>

		<!-- Editor Component -->
		<div class="flex-1 min-h-0 relative p-4">
			<RichTextEditor />
		</div>
	</div>

	<NoteRightContainer />
</div>
