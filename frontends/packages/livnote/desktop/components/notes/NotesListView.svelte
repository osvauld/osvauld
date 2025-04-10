<script lang="ts">
	import { run, stopPropagation } from "svelte/legacy";

	import {
		selectedCategory,
		currentVault,
		noteViewLayout,
		noteId,
		refreshCredentialList,
		notes,
		currentNote,
	} from "../../store/desktop.ui.store";
	import { extractTitle, getLastModifiedDate } from "../utils/helper";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	import { emit } from "@tauri-apps/api/event";
	import RichTextEditor from "./RichTextEditor.svelte";
	import NotePreview from "./NotePreview.svelte";
	import {
		FavStar as Star,
		Star as EmptyStar,
	} from "@osvauld/password-manager-common";
	import { get } from "svelte/store";

	interface NoteData {
		title?: string;
		content?: string;
		last_modified?: number;
		last_accessed?: number;
		editor_state?: string | Record<string, unknown>;
		yjs_state?: Uint8Array | number[];
	}

	interface Note {
		id: string;
		data: NoteData;
		favourite?: boolean;
	}

	interface Vault {
		id: string;
		name: string;
		description?: string;
	}

	interface Props {
		favSelected?: boolean;
	}

	let { favSelected = false }: Props = $props();
	let updatedNotes: Note[] = $state([]);
	let isLoading: boolean = $state(true);
	let error: string | null = $state(null);

	run(() => {
		updatedNotes = $notes;
	});

	// Function to fetch notes based on the current vault
	const fetchNotes = async () => {
		isLoading = true;
		error = null;
		let fetchedNotes: Note[] = [];

		try {
			const currentVaultValue = get(currentVault) as Vault;
			if (currentVaultValue.id === "all") {
				fetchedNotes = await sendMessage("getAllCredentials", {
					favourite: false,
				});
			} else {
				fetchedNotes = await sendMessage("getCredentialsForFolder", {
					folderId: currentVaultValue.id,
				});
			}

			// Filter for valid notes only
			fetchedNotes = fetchedNotes.filter(
				(cred: Note) =>
					cred.data && cred.data.content && cred.data.editor_state,
			);

			// Sort by last accessed/modified (most recent first)
			fetchedNotes.sort((a: Note, b: Note) => {
				const timeA = a.data.last_accessed || a.data.last_modified || 0;
				const timeB = b.data.last_accessed || b.data.last_modified || 0;
				return timeB - timeA;
			});
			console.log("fetched notest", fetchedNotes);
			notes.set(fetchedNotes);
		} catch (err) {
			console.error("Error fetching notes:", err);
			error = "Failed to load notes. Please try again.";
			updatedNotes = [];
		} finally {
			isLoading = false;
		}
	};

	run(() => {
		updatedNotes = favSelected
			? $notes.filter((note: Note) => note.favourite)
			: $notes;
	});

	// Function to toggle favorite status
	const toggleFavorite = async (noteId: string, currentStatus: boolean) => {
		try {
			await sendMessage("toggleFav", {
				resourceId: noteId,
			});

			// Update local state
			const notesWithFavToggleChange = $notes.map((cred: Note) => {
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
	const selectNote = (note: Note) => {
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
	run(() => {
		if (get(currentVault)) {
			fetchNotes();
		}
	});

	// Watch for refresh requests
	run(() => {
		if (get(refreshCredentialList)) {
			fetchNotes();
			refreshCredentialList.set(false);
		}
	});

	// Calculate grid layout
	const getColumnCount = (): number => {
		if (typeof window === "undefined") return 1;
		if (window.innerWidth >= 1440) return 3;
		if (window.innerWidth >= 1024) return 2;
		return 1;
	};

	const getColumnItems = (items: Note[], colIndex: number): Note[] => {
		const colCount = getColumnCount();
		return items.filter((_, index: number) => index % colCount === colIndex);
	};

	// onMount(() => {
	// 	fetchNotes();

	// 	// Listen for window resize to update columns
	// 	// window.addEventListener("resize", fetchNotes);
	// });

	// onDestroy(() => {
	// 	window.removeEventListener("resize", fetchNotes);
	// });
</script>

<div class="grow max-h-full overflow-hidden px-11 py-4 relative">
	<div
		class="h-full pr-1 scrollbar-thin min-w-[37.5rem] {$noteViewLayout
			? 'overflow-hidden '
			: 'overflow-y-auto'}">
		{#if $noteViewLayout}
			<RichTextEditor
				on:collaboration-update={(event) =>
					emit("sync-update", JSON.stringify(event.detail))} />
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
								onclick={() => selectNote(note)}>
								<div
									class="p-4 border-b border-osvauld-borderColor flex justify-between items-center">
									<h3
										class="text-osvauld-fieldText font-medium text-lg truncate">
										{note?.data.title ? note.data.title : "Untitled note"}
									</h3>
									<button
										class="flex items-center justify-center p-1 cursor-pointer"
										onclick={stopPropagation(() =>
											toggleFavorite(note.id, note.favourite ?? false),
										)}>
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
										content={note.data.content ?? ""}
										title={note.data.title ?? ""}
										editorState={typeof note.data.editor_state === "string"
											? JSON.parse(note.data.editor_state)
											: note.data.editor_state}
										yjsState={note.data.yjs_state instanceof Uint8Array
											? Array.from(note.data.yjs_state)
											: note.data.yjs_state}
										maxHeight="180px"
										minHeight="180px" />
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
