<script lang="ts">
	import { onMount } from "svelte";
	import { BackArrow, Star as EmptyStar, FavStar as Star } from "../../icons";
	import NoteRightContainer from "../ui/NoteRightContainer.svelte";
	import { dataState, uiState } from "../../state";
	import RichTextEditor from "../notes/RichTextEditor.svelte";
	import NavigationPanel from "./NavigationPanel.svelte";
	import Hamburger from "../../icons/Hamburger.svelte";
	import { fade, fly } from "svelte/transition";

	// Local UI state using $state
	let newNoteTitle = $state("");
	let isEditingTitle = $state(false);
	let inputRef = $state<HTMLInputElement | null>(null);
	let userId = $state("");
	let saved = $state(false);
	let title = $state("");
	// Derived state for favorite status
	let isFavourite = $derived(
		dataState.getCurrentNoteData()?.favourite ?? false,
	);
	$effect(() => {
		if (dataState.currentNoteId) {
			title = dataState.getNotesCoordinator()?.getCurrentTitle();
			console.log("title", title);
		}
	});
	// Toggle navigation panel
	function toggleNavigationPanel() {
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
	}

	// Title editing functions
	function startEditingTitle() {
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
		let coordinator = dataState.getNotesCoordinator();
		coordinator?.saveNote(newNoteTitle);
		title = newNoteTitle;
	}

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
		dataState.saveNote(noteId);
		dataState.clearCurrentNote();
		uiState.toggleNoteViewLayout(false);
	};

	const toggleFav = async (e: Event) => {
		e.stopPropagation();
		//TODO
		// Update the note in the notes array
	};

	onMount(async () => {
		if (dataState.userDetails?.userId) {
			userId = dataState.userDetails?.userId;
		}

		// Set CSS variable for minimum editor width
		document.documentElement.style.setProperty(
			"--min-editor-width",
			`${uiState.MIN_EDITOR_WIDTH}px`,
		);
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
	class:manual-toggle={uiState.isNavigationPanelManuallyToggled}>
	<NavigationPanel />

	<div class="flex-1 flex flex-col overflow-hidden">
		<!-- Header section with back button and title -->
		<div class="py-10 px-11 flex items-center justify-start shrink-0">
			<div class="flex justify-between items-center max-w-[44rem]">
				<!-- Burger menu toggle - only show when navigation panel is hidden -->
				{#if !uiState.showNavigationPanel}
					<button
						aria-label="Toggle navigation panel"
						class="mr-3 rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-pointer"
						onclick={toggleNavigationPanel}>
						<Hamburger />
					</button>
				{/if}

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
						onkeydown={(e: KeyboardEvent) =>
							e.key === "Enter" && startEditingTitle()}>
						{title}
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

			{#if dataState.collaborators.length > 0}
				<div class="mx-6 px-6 border-x border-osvauld-borderColor">
					<span class="text-statusColor font-light text-sm block mb-3"
						>Live Collaborators</span>
					<div class="flex items-center">
						<div class="flex">
							{#each dataState.collaborators.slice(0, 3) as collaborator, index (collaborator.id)}
								<div
									class="relative {index !== 0 ? '-ml-3' : ''}"
									in:fade={{ duration: 200 }}
									out:fade={{ duration: 200 }}>
									<div
										class="w-12 h-12 z-10 rounded-full bg-osvauld-fieldActive text-xl font-medium text-collaboratorText border border-collaboratorBorder flex justify-center items-center relative">
										{getInitial(collaborator.name)}

										<!-- Live indicator dot -->
										<div
											class="absolute bottom-0 left-0 w-3 h-3 bg-green-500 rounded-full border-2 border-osvauld-fieldActive"
											in:fade={{ duration: 200 }}>
										</div>
									</div>
								</div>
							{/each}

							{#if dataState.collaborators.length > 3}
								<div class="relative -ml-3">
									<div
										class="w-10 h-10 -z-10 rounded-full bg-osvauld-fieldActive text-sm font-medium text-collaboratorText border border-collaboratorBorder flex justify-center items-center">
										+{dataState.collaborators.length - 3}
									</div>
								</div>
							{/if}
						</div>
					</div>
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
