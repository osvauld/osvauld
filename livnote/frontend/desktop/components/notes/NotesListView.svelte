<script lang="ts">
	import { onMount, onDestroy, untrack } from "svelte";
	import { getLastModifiedDate } from "../../utils/helper";
	import { sendMessage } from "../../utils/helper";
	import NotePreview from "./NotePreview.svelte";
	import NoteListPanel from "../ui/NoteListPanel.svelte";
	import {
		FavStar as Star,
		Star as EmptyStar,
		RightArrow as Arrow,
	} from "../../icons";

	import { dataState, uiState } from "../../state/";

	// Local state for responsive grid
	let resizeTimer = $state<number | null>(null);
	let columnCount = $state<number>(1);

	// One-by-one rendering state
	let renderedCount = $state<number>(0);
	let renderTimer: number | null = null;

	// Function to toggle favorite status
	const toggleFavorite = async (noteId: string, currentStatus: boolean) => {
		try {
			await sendMessage("toggleFav", {
				resourceId: noteId,
			});
			dataState.updateNoteFavorite(noteId);
		} catch (err) {
			console.error("Error toggling favorite:", err);
			uiState.showToast("Failed to update favorite status", false);
		}
	};

	// Function to handle note selection
	const selectNote = (note: any) => {
		dataState.switchNote(note.id);
	};

	// Calculate grid layout
	const getColumnCount = (): number => {
		if (typeof window === "undefined") return 1;
		if (window.innerWidth >= 1440) return 3;
		if (window.innerWidth >= 1024) return 2;
		return 1;
	};

	const getColumnItems = (items: any[], colIndex: number) => {
		return items.filter((_, index) => index % columnCount === colIndex);
	};

	// Start rendering notes one by one
	const startRendering = () => {
		console.log(
			"Starting one-by-one rendering for",
			dataState.filteredNotes.length,
			"notes",
		);
		renderedCount = 1; // Show first note immediately
		renderNext();
	};

	const renderNext = () => {
		if (renderedCount >= dataState.filteredNotes.length) {
			console.log("All notes rendered");
			return;
		}

		renderTimer = setTimeout(() => {
			renderedCount++;
			renderNext(); // Render the next one
		}, 16); // 16ms = ~60fps
	};

	const stopRendering = () => {
		if (renderTimer) {
			clearTimeout(renderTimer);
			renderTimer = null;
		}
	};

	// Get notes that should be visible
	const getVisibleNotes = () => {
		return dataState.filteredNotes.slice(0, renderedCount);
	};

	// Handle window resize
	function handleResize() {
		if (resizeTimer !== null) {
			clearTimeout(resizeTimer);
		}

		resizeTimer = setTimeout(() => {
			const newColumnCount = getColumnCount();
			if (newColumnCount !== columnCount) {
				columnCount = newColumnCount;
				// Force re-render when column count changes
				dataState.notes = [...dataState.notes];
			}
		}, 250) as unknown as number;
	}

	let previousFilteredNotesLength = 0;

	$effect(() => {
		// Only track the data length, not renderedCount
		const currentLength = dataState.filteredNotes.length;
		const isLoading = dataState.isDataLoading;

		if (
			!isLoading &&
			currentLength > 0 &&
			currentLength !== previousFilteredNotesLength
		) {
			previousFilteredNotesLength = currentLength;
			// Use untrack to prevent this effect from triggering on renderedCount changes
			untrack(() => {
				stopRendering();
				renderedCount = 0;
				startRendering();
			});
		}
	});

	onMount(() => {
		// Initialize column count
		columnCount = getColumnCount();
		// Add resize listener
		window.addEventListener("resize", handleResize);

		// Start rendering if we have notes
		if (!dataState.isDataLoading && dataState.filteredNotes.length > 0) {
			startRendering();
		}
	});

	onDestroy(() => {
		// Clean up
		window.removeEventListener("resize", handleResize);
		if (resizeTimer !== null) {
			clearTimeout(resizeTimer);
		}
		stopRendering();
	});
</script>

<style>
	@keyframes fadeIn {
		from {
			opacity: 0;
			transform: translateY(10px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.note-card-enter {
		animation: fadeIn 0.3s ease-out forwards;
	}

	@keyframes shimmer {
		0% {
			background-position: -200px 0;
		}
		100% {
			background-position: calc(200px + 100%) 0;
		}
	}
</style>

<div class="grow max-h-full overflow-hidden px-11 py-4 relative flex flex-col">
	<NoteListPanel />
	<div class="grow pr-1 scrollbar-thin min-w-[37.5rem] overflow-y-auto">
		{#if dataState.isDataLoading}
			<div class="flex justify-center items-center h-full">
				<div class="text-osvauld-fieldText">Loading notes...</div>
			</div>
		{:else if dataState.filteredNotes.length === 0}
			<div class="flex justify-center items-center h-full">
				<div class="text-osvauld-fieldText">
					No notes found. Create a new note to get started.
				</div>
			</div>
		{:else}
			<div class="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-6">
				{#each Array(columnCount) as _, colIndex}
					<div class="flex flex-col gap-6">
						{#each getColumnItems(getVisibleNotes(), colIndex) as note (note.id)}
							<div
								role="presentation"
								class="note-card-enter bg-osvauld-frameblack border border-osvauld-borderColor rounded-lg overflow-hidden hover:border-osvauld-carolinablue transition-colors duration-200 cursor-pointer"
								onclick={() => selectNote(note)}>
								<div
									class="p-4 border-b border-osvauld-borderColor flex justify-between items-center">
									<h3
										class="text-osvauld-fieldText font-medium text-lg truncate">
										{note?.title}
									</h3>
									<button
										class="flex items-center justify-center p-1 cursor-pointer"
										onclick={(e) => {
											e.stopPropagation();
											toggleFavorite(note.id, note.favourite ?? false);
										}}>
										{#if note.favourite}
											<Star />
										{:else}
											<EmptyStar color="#85889C" />
										{/if}
									</button>
								</div>
								<div class="p-4">
									<!-- Rich text preview -->
									<NotePreview
										previewHTML={note.previewHTML}
										maxHeight="180px"
										minHeight="180px" />
									<div class="text-osvauld-fieldText opacity-60 text-xs mt-4">
										Last modified: {getLastModifiedDate(note.lastModified)}
									</div>
								</div>
							</div>
						{/each}
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
