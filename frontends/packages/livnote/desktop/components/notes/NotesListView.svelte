<script>
	import {
		selectedCategory,
		currentVault,
		noteViewLayout,
		noteId,
		refreshCredentialList,
		notes,
		currentNote,
	} from "../../store/desktop.ui.store";
	import { extractTitle } from "../utils/helper";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	import { emit } from "@tauri-apps/api/event";
	import RichTextEditor from "./RichTextEditor.svelte";
	import NotePreview from "./NotePreview.svelte";
	import Star from "@osvauld/password-manager-common/icons/favStar.svelte";
	import EmptyStar from "@osvauld/password-manager-common/icons/star.svelte";
	import { onMount, onDestroy } from "svelte";

	export let favSelected;
	let updatedNotes = [];
	let isLoading = true;
	let error = null;

	$: updatedNotes = $notes;

	// Function to get last modified date in readable format
	const getLastModifiedDate = (timestamp) => {
		if (!timestamp) return "Never";
		const date = new Date(timestamp);
		return date.toLocaleDateString() + " " + date.toLocaleTimeString();
	};

	// Function to fetch notes based on the current vault
	const fetchNotes = async () => {
		isLoading = true;
		error = null;
		let fetchedNotes = [];

		try {
			if ($currentVault.id === "all") {
				fetchedNotes = await sendMessage("getAllCredentials", {
					favourite: false,
				});
			} else {
				fetchedNotes = await sendMessage("getCredentialsForFolder", {
					folderId: $currentVault.id,
				});
			}

			// Filter for valid notes only
			fetchedNotes = fetchedNotes.filter(
				(cred) => cred.data && cred.data.content && cred.data.editor_state,
			);

			// Sort by last accessed/modified (most recent first)
			fetchedNotes.sort((a, b) => {
				const timeA = a.data.last_accessed || a.data.last_modified || 0;
				const timeB = b.data.last_accessed || b.data.last_modified || 0;
				return timeB - timeA;
			});
			console.log("fetched notest", fetchedNotes);
			//updatedNotes = fetchedNotes;
			notes.set(fetchedNotes);
		} catch (err) {
			console.error("Error fetching notes:", err);
			error = "Failed to load notes. Please try again.";
			updatedNotes = [];
		} finally {
			isLoading = false;
		}
	};

	$: {
		updatedNotes = favSelected
			? $notes.filter((note) => note.favourite)
			: $notes;
	}

	// Function to toggle favorite status
	const toggleFavorite = async (noteId, currentStatus) => {
		try {
			await sendMessage("toggleFav", {
				resourceId: noteId,
			});

			// Update local state
			const notesWithFavToggleChange = $notes.map((cred) => {
				if (cred.id === noteId) {
					return {
						...cred,
						data: {
							...cred.data,
						},
						favourite: !currentStatus,
					};
				}
				return cred;
			});
			notes.set(notesWithFavToggleChange);
			updatedNotes = notesWithFavToggleChange;
		} catch (err) {
			console.error("Error toggling favorite:", err);
		}
	};

	// Function to handle note selection
	const selectNote = (note) => {
		currentNote.set(note);

		// First reset the note view to ensure clean state
		noteViewLayout.set(false);

		// Wait for UI update to complete
		setTimeout(() => {
			// Then set the note ID
			noteId.set(note.id);

			// Finally switch to editor view
			noteViewLayout.set(true);
		}, 50);
	};

	// Watch for changes to currentVault
	$: if ($currentVault) {
		fetchNotes();
	}

	// Watch for refresh requests
	$: if ($refreshCredentialList) {
		fetchNotes();
		refreshCredentialList.set(false);
	}

	// Calculate grid layout
	const getColumnCount = () => {
		if (typeof window === "undefined") return 1;
		if (window.innerWidth >= 1440) return 3;
		if (window.innerWidth >= 1024) return 2;
		return 1;
	};

	const getColumnItems = (items, colIndex) => {
		const colCount = getColumnCount();
		return items.filter((_, index) => index % colCount === colIndex);
	};

	onMount(() => {
		fetchNotes();

		// Listen for window resize to update columns
		window.addEventListener("resize", fetchNotes);
	});

	onDestroy(() => {
		window.removeEventListener("resize", fetchNotes);
	});
</script>

<div class="grow max-h-[85%] overflow-y-scroll px-16 py-4 relative">
	<div class="h-full overflow-hidden pr-1 scrollbar-none">
		{#if $noteViewLayout}
			<RichTextEditor
				on:collaboration-update="{(event) =>
					emit('sync-update', JSON.stringify(event.detail))}" />
		{:else if isLoading}
			<div class="flex justify-center items-center h-full">
				<div class="text-osvauld-fieldText">Loading notes...</div>
			</div>
		{:else if error}
			<div class="flex justify-center items-center h-full">
				<div class="text-red-500">{error}</div>
			</div>
		{:else if updatedNotes.length === 0}
			<div class="flex justify-center items-center h-full">
				<div class="text-osvauld-fieldText">
					No notes found. Create a new note to get started.
				</div>
			</div>
		{:else}
			<div class="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-6">
				{#each Array(getColumnCount()) as _, colIndex}
					<div class="flex flex-col gap-6">
						{#each getColumnItems(updatedNotes, colIndex) as note (note.id)}
							<div
								role="presentation"
								class="bg-osvauld-frameblack border border-osvauld-borderColor rounded-lg overflow-hidden hover:border-osvauld-carolinablue transition-colors duration-200 cursor-pointer"
								on:click="{() => selectNote(note)}">
								<div
									class="p-4 border-b border-osvauld-borderColor flex justify-between items-center">
									<h3
										class="text-osvauld-fieldText font-medium text-lg truncate">
										{extractTitle(note.data.content)}
									</h3>
									<button
										class="flex items-center justify-center p-1 cursor-pointer"
										on:click|stopPropagation="{() =>
											toggleFavorite(note.id, note.favourite)}">
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
										content="{note.data.content}"
										editorState="{note.data.editor_state}"
										yjsState="{note.data.yjs_state}"
										maxHeight="120px"
										minHeight="120px" />
									<div class="text-osvauld-fieldText opacity-60 text-xs mt-4">
										Last modified: {getLastModifiedDate(
											note.data.last_modified || note.data.last_accessed,
										)}
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
