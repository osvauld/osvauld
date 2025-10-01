<script lang="ts">
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
</script>

<div class="flex flex-1 h-full overflow-hidden">
	<div class="flex-1 flex flex-col h-full overflow-hidden">
		<div
			class="flex-1 overflow-hidden pl-4 pr-1 pb-2 pt-0 relative flex flex-col"
		>
			<NoteListPanel />
			<div class="flex-1 pr-1 scrollbar-thin pb-4 overflow-y-auto">
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
					<div
						class="grid gap-3"
						style="grid-template-columns: repeat(auto-fit, minmax(300px, 300px)); justify-content: start;"
					>
						{#each dataState.filteredNotes as note (note.id)}
							<div
								role="presentation"
								class="w-[300px] bg-osvauld-frameblack border border-osvauld-borderColor rounded-lg overflow-hidden hover:border-livnotelavender transition-colors duration-200 cursor-pointer"
								onclick={() => selectNote(note)}
							>
								<div
									class="px-4 py-2 border-b border-osvauld-borderColor flex justify-between items-center"
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
											<Star size={20} />
										{:else}
											<EmptyStar color="#85889C" size={20} />
										{/if}
									</button>
								</div>
								<div class="p-4">
									<NotePreview
										previewHTML={note.preview}
										maxHeight="120px"
										minHeight="120px"
									/>
									<div class="text-osvauld-fieldText opacity-60 text-xs mt-4">
										Last modified: {getLastModifiedDate(note.lastModified)}
									</div>
								</div>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		</div>
	</div>
</div>
