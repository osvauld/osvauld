<script lang="ts">
	import { onMount } from "svelte";
	import {
		BackArrow,
		Star as EmptyStar,
		FavStar as Star,
	} from "@osvauld/password-manager-common";
	import { sendMessage } from "@osvauld/password-manager-common";
	import NoteRightContainer from "../ui/NoteRightContainer.svelte";
	import { dataState, uiState } from "../../state";
	import RichTextEditor from "../notes/RichTextEditor.svelte";
	import NavigationPanel from "./NavigationPanel.svelte";
	import { notesInstance } from "../notes/notes";

	// Local UI state using $state
	let newNoteTitle = $state("");
	let isEditingTitle = $state(false);
	let inputRef = $state<HTMLInputElement | null>(null);
	let userId = $state("");
	let saved = $state(false);

	// Derived state for favorite status
	let isFavourite = $derived(dataState.currentNote?.favourite ?? false);

	// Title editing functions
	function startEditingTitle() {
		newNoteTitle = dataState.currentNote?.data?.title ?? "Untitled note";
		isEditingTitle = true;

		// Focus the input after the DOM updates
		setTimeout(() => {
			if (inputRef) {
				inputRef.focus();
				inputRef.select();
			}
		}, 0);
	}

	function saveTitle() {
		if (newNoteTitle.trim() && dataState.currentNote) {
			// Update the note title in state
			dataState.currentNote = {
				...dataState.currentNote,
				data: {
					...dataState.currentNote.data,
					title: newNoteTitle,
				},
			};

			saveNoteManual();
		}
		isEditingTitle = false;
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Enter") {
			saveTitle();
		} else if (event.key === "Escape") {
			isEditingTitle = false;
		}
	}

	// Back button handler - saves and returns to list view
	const handleBackButton = () => {
		if (dataState.currentNote) {
			notesInstance
				.saveNote(dataState.currentNote.data?.title || "Untitled")
				.catch(console.error);
		}

		// Switch to list view
		uiState.toggleNoteViewLayout(false);
		dataState.clearCurrentNote();
	};

	const toggleFav = async (e) => {
		e.stopPropagation();
		if (!dataState.currentNote) return;

		const newFavStatus = !isFavourite;

		try {
			await sendMessage("toggleFav", {
				resourceId: dataState.currentNote.id,
			});

			// Update the note in state
			dataState.currentNote = {
				...dataState.currentNote,
				favourite: newFavStatus,
			};

			dataState.updateNoteFavorite(dataState.currentNote.id);
			// Update the note in the notes array
		} catch (err) {
			console.error("Error toggling favorite:", err);
			uiState.showToast("Failed to update favorite status", false);
		}
	};

	// Handle copying note content
	const saveNoteManual = () => {
		if (!dataState.currentNote) return;

		saved = true;
		notesInstance
			.saveNote(dataState.currentNote.data?.title || "Untitled")
			.catch(console.error);

		setTimeout(() => {
			saved = false;
		}, 1000);
	};

	onMount(async () => {
		userId = await sendMessage("getUserId");
	});
</script>

<div class="flex grow max-h-full max-w-[calc(100vw)]">
	<NavigationPanel />

	<div class="flex-1 flex flex-col overflow-hidden">
		<!-- Header section with back button and title -->
		<div class="py-10 px-11 flex items-center justify-start shrink-0">
			<div class="flex justify-between items-center max-w-[44rem]">
				<button
					class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-pointer"
					onclick={handleBackButton}>
					<BackArrow />
				</button>

				{#if isEditingTitle}
					<div
						class="grow mx-5 flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack">
						<input
							bind:this={inputRef}
							bind:value={newNoteTitle}
							maxlength="20"
							onkeydown={handleKeydown}
							onblur={saveTitle}
							class="text-white text-4xl bg-osvauld-frameblack border-0 tracking-wider font-semibold border-transparent focus:border-osvauld-iconblack focus:outline-0 focus:ring-0 active:outline-none focus:ring-offset-0" />
					</div>
				{:else}
					<span
						role="button"
						tabindex="0"
						class="grow truncate mx-5 py-2 font-semibold text-4xl text-osvauld-sideListTextActive"
						ondblclick={startEditingTitle}
						onkeydown={(e) => e.key === "Enter" && startEditingTitle()}>
						{dataState.currentNote?.data?.title || "Untitled"}
					</span>
				{/if}
				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-pointer"
					onclick={toggleFav}>
					{#if isFavourite}
						<Star />
					{:else}
						<EmptyStar color="#85889C" />
					{/if}
				</button>
			</div>
		</div>

		<!-- Editor Component -->
		<div class="grow relative p-4">
			<RichTextEditor />
		</div>
	</div>

	<NoteRightContainer />
</div>
