<script lang="ts">
	import {
		selectedCategory,
		currentVault,
		noteViewLayout,
		noteId,
		refreshCredentialList,
	} from "../../store/desktop.ui.store";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	import { listen, emit } from "@tauri-apps/api/event";
	import RichTextEditor from "../lib/RichTextEditor.svelte";
	import NotePreview from "./NotePreview.svelte";
	import Star from "@osvauld/password-manager-common/icons/favStar.svelte";
	import EmptyStar from "@osvauld/password-manager-common/icons/star.svelte";
	import { onMount, onDestroy } from "svelte";
	import * as Y from "yjs";
	import { schema } from "prosemirror-schema-basic";
	import { addListNodes } from "prosemirror-schema-list";
	import { Schema } from "prosemirror-model";

	let credentials = [];
	let isLoading = true;
	let error = null;

	// Function to get title from content (first heading or first line)
	function extractTitle(content) {
		// Try to find a heading tag
		const headingMatch = content.match(/<heading[^>]*>(.*?)<\/heading>/);
		if (headingMatch && headingMatch[1]) {
			return headingMatch[1].replace(/<[^>]+>/g, "").trim();
		}

		// Otherwise, get the first paragraph or line
		const firstParagraphMatch = content.match(
			/<paragraph[^>]*>(.*?)<\/paragraph>/,
		);
		if (firstParagraphMatch && firstParagraphMatch[1]) {
			const text = firstParagraphMatch[1].replace(/<[^>]+>/g, "").trim();
			// Return first 30 chars if there's text
			return text
				? text.length > 30
					? text.substring(0, 30) + "..."
					: text
				: "Untitled Note";
		}

		return "Untitled Note";
	}

	// Function to get last modified date in readable format
	function getLastModifiedDate(timestamp) {
		if (!timestamp) return "Never";
		const date = new Date(timestamp);
		return date.toLocaleDateString() + " " + date.toLocaleTimeString();
	}

	// Function to fetch notes based on the current vault
	async function fetchNotes() {
		isLoading = true;
		error = null;

		try {
			if ($currentVault.id === "all") {
				credentials = await sendMessage("getAllCredentials", {
					favourite: false,
				});
			} else {
				credentials = await sendMessage("getCredentialsForFolder", {
					folderId: $currentVault.id,
				});
			}

			// Filter for notes only
			credentials = credentials.filter(
				(cred) => cred.data && cred.data.content && cred.data.editor_state,
			);

			// Sort by last accessed/modified (most recent first)
			credentials.sort((a, b) => {
				const timeA = a.data.last_accessed || a.data.last_modified || 0;
				const timeB = b.data.last_accessed || b.data.last_modified || 0;
				return timeB - timeA;
			});
		} catch (err) {
			console.error("Error fetching notes:", err);
			error = "Failed to load notes. Please try again.";
			credentials = [];
		} finally {
			isLoading = false;
		}
	}

	// Function to toggle favorite status
	async function toggleFavorite(noteId, currentStatus) {
		try {
			await sendMessage("toggleFavorite", {
				credentialId: noteId,
				favorite: !currentStatus,
			});

			// Update local state
			credentials = credentials.map((cred) => {
				if (cred.id === noteId) {
					return {
						...cred,
						data: {
							...cred.data,
							favourite: !currentStatus,
						},
					};
				}
				return cred;
			});
		} catch (err) {
			console.error("Error toggling favorite:", err);
		}
	}

	// Function to handle note selection
	function selectNote(id) {
		console.log(`Selecting note: ${id}`);

		// First reset the note view to ensure clean state
		noteViewLayout.set(false);

		// Wait for UI update to complete
		setTimeout(() => {
			// Then set the note ID
			noteId.set(id);

			// Finally switch to editor view
			noteViewLayout.set(true);
		}, 50);
	}

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
	function getColumnCount() {
		if (typeof window === "undefined") return 1;
		if (window.innerWidth >= 1440) return 3;
		if (window.innerWidth >= 1024) return 2;
		return 1;
	}

	function getColumnItems(items, colIndex) {
		const colCount = getColumnCount();
		return items.filter((_, index) => index % colCount === colIndex);
	}

	onMount(() => {
		fetchNotes();

		// Listen for window resize to update columns
		window.addEventListener("resize", fetchNotes);
	});

	onDestroy(() => {
		window.removeEventListener("resize", fetchNotes);
	});
</script>

<div class="grow max-h-[85%] px-16 py-4 relative">
	<div class="h-full overflow-y-auto overflow-x-hidden pr-1 scrollbar-none">
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
		{:else if credentials.length === 0}
			<div class="flex justify-center items-center h-full">
				<div class="text-osvauld-fieldText">
					No notes found. Create a new note to get started.
				</div>
			</div>
		{:else}
			<div class="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-6">
				{#each Array(getColumnCount()) as _, colIndex}
					<div class="flex flex-col gap-6">
						{#each getColumnItems(credentials, colIndex) as note (note.id)}
							<div
								class="bg-osvauld-frameblack border border-osvauld-borderColor rounded-lg overflow-hidden hover:border-osvauld-carolinablue transition-colors duration-200 cursor-pointer"
								on:click={() => selectNote(note.id)}>
								<div
									class="p-4 border-b border-osvauld-borderColor flex justify-between items-center">
									<h3
										class="text-osvauld-fieldText font-medium text-lg truncate">
										{extractTitle(note.data.content)}
									</h3>
									<button
										class="flex items-center justify-center p-1"
										on:click|stopPropagation={() =>
											toggleFavorite(note.id, note.data.favourite)}>
										{#if note.data.favourite}
											<Star />
										{:else}
											<EmptyStar color="#85889C" />
										{/if}
									</button>
								</div>
								<div class="p-4">
									<!-- Rich text preview -->
									<NotePreview
										content={note.data.content}
										editorState={note.data.editor_state}
										yjsState={note.data.yjs_state}
										maxHeight="120px" />
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
