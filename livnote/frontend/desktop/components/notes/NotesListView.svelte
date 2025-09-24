<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { getLastModifiedDate } from "../../utils/helper";
	import { sendMessage } from "../../utils/helper";
	import NotePreview from "./NotePreview.svelte";
	import NoteListPanel from "../ui/NoteListPanel.svelte";
	import NavigationPanel from "../layout/NavigationPanel.svelte";
	import {
		FavStar as Star,
		Star as EmptyStar,
		RightArrow as Arrow,
	} from "../../icons";

	import { dataState, uiState } from "../../state/";

	let resizeTimer = $state<number | null>(null);
	let columnCount = $state<number>(1);

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

	const selectNote = (note: any) => {
		dataState.switchNote(note.id);
	};

	const getColumnCount = (): number => {
		if (typeof window === "undefined") return 1;
		if (window.innerWidth >= 1440) return 3;
		if (window.innerWidth >= 1024) return 2;
		return 1;
	};

	const getColumnItems = (items: any[], colIndex: number) => {
		return items.filter((_, index) => index % columnCount === colIndex);
	};

	function handleResize() {
		if (resizeTimer !== null) {
			clearTimeout(resizeTimer);
		}

		resizeTimer = setTimeout(() => {
			const newColumnCount = getColumnCount();
			if (newColumnCount !== columnCount) {
				columnCount = newColumnCount;
				dataState.notes = [...dataState.notes];
			}
		}, 250) as unknown as number;
	}

	onMount(() => {
		columnCount = getColumnCount();
		window.addEventListener("resize", handleResize);
	});

	onDestroy(() => {
		window.removeEventListener("resize", handleResize);
		if (resizeTimer !== null) {
			clearTimeout(resizeTimer);
		}
	});
</script>

<div class="flex grow max-h-full max-w-full">
	<NavigationPanel />

	<div class="flex-1 flex flex-col overflow-hidden">
		<div
			class="grow max-h-full overflow-hidden px-11 py-4 relative flex flex-col"
		>
			<NoteListPanel />

			<div class="grow pr-1 scrollbar-thin min-w-[37.5rem] overflow-y-auto">
				{#if dataState.isDataLoading}
					<div class="flex justify-center items-center h-full">
						<div class="text-osvauld-fieldText">Loading notes...</div>
					</div>
				{:else if dataState.filteredNotes.length === 0}
					<div class="flex justify-center items-center h-full">
						<div class="text-osvauld-fieldText">
							{dataState.favoriteSelected
								? "No favourites found."
								: "No notes found. Create a new note to get started."}
						</div>
					</div>
				{:else}
					<div class="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-6">
						{#each Array(columnCount) as _, colIndex}
							<div class="flex flex-col gap-6">
								{#each getColumnItems(dataState.filteredNotes, colIndex) as note (note.id)}
									<div
										role="presentation"
										class="bg-osvauld-frameblack border border-osvauld-borderColor rounded-lg overflow-hidden hover:border-livnotelavender transition-colors duration-200 cursor-pointer"
										onclick={() => selectNote(note)}
									>
										<div
											class="p-4 border-b border-osvauld-borderColor flex justify-between items-center"
										>
											<h3
												class="text-osvauld-fieldText font-medium text-lg truncate"
											>
												{note?.title}
											</h3>
											<button
												class="flex items-center justify-center p-1 cursor-pointer"
												onclick={(e) => {
													e.stopPropagation();
													toggleFavorite(note.id, note.favourite ?? false);
												}}
											>
												{#if note.favourite}
													<Star />
												{:else}
													<EmptyStar color="#85889C" />
												{/if}
											</button>
										</div>
										<div class="p-4">
											<NotePreview
												previewHTML={note.preview}
												maxHeight="180px"
												minHeight="180px"
											/>
											<div
												class="text-osvauld-fieldText opacity-60 text-xs mt-4"
											>
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
	</div>
</div>
