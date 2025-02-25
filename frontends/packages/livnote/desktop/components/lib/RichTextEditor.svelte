<script>
	import { onMount, onDestroy, createEventDispatcher } from "svelte";
	import { EditorView } from "prosemirror-view";
	import * as Y from "yjs";
	import { listen } from "@tauri-apps/api/event";
	import { notesInstance } from "./utils/notes";
	import { noteId, noteViewLayout } from "../../store/desktop.ui.store";

	const dispatch = createEventDispatcher();
	let element;
	let view = null;
	let autoSaveInterval;
	let unsubscribeUpdate;
	let isLoading = false;
	let error = null;
	let currentlyLoadedNoteId = null;
	let loadingInProgress = false;

	// Listen for noteId changes and load the corresponding note
	$: if (
		$noteId &&
		element &&
		$noteId !== currentlyLoadedNoteId &&
		!loadingInProgress
	) {
		loadNote($noteId);
	}

	async function loadNote(id) {
		if (!element || loadingInProgress) return;

		loadingInProgress = true;

		// Clear any existing content and show loading state
		if (view) {
			console.log("Destroying existing editor view");
			view.destroy();
			view = null;
		}

		isLoading = true;
		error = null;

		try {
			console.log(`Loading note: ${id}`);

			// Load the note with the given ID
			const docInfo = await notesInstance.loadNote(id);

			// Force a small delay to ensure DOM is ready
			await new Promise((resolve) => setTimeout(resolve, 50));

			// If another load operation started while we were waiting, abort
			if (currentlyLoadedNoteId !== null && currentlyLoadedNoteId !== id) {
				console.log("Aborting load - another note was loaded");
				return;
			}

			// Create editor view with the loaded content
			view = createEditorView(element, docInfo.editorState);

			// Mark this note as loaded
			currentlyLoadedNoteId = id;

			// Setup auto-save
			if (autoSaveInterval) {
				clearInterval(autoSaveInterval);
			}

			autoSaveInterval = setInterval(() => {
				notesInstance.saveNote().catch(console.error);
			}, 30000); // Auto-save every 30 seconds

			// Set up listener for sync updates from other peers
			if (unsubscribeUpdate) {
				unsubscribeUpdate();
			}

			unsubscribeUpdate = await listen("sync-update-be", (event) => {
				try {
					const parsed =
						typeof event.payload === "string"
							? JSON.parse(event.payload)
							: event.payload;

					const { update, clientID: remoteClientID } = parsed;

					// Remote update will be applied with 'sync' origin
					notesInstance.applyUpdate(update, remoteClientID);

					// If view exists, force a refresh to show the changes
					if (view) {
						view.updateState(view.state);
					}
				} catch (err) {
					console.error("Error handling update:", err);
				}
			});

			// Force an update to ensure content is rendered
			if (view) {
				const tr = view.state.tr;
				view.dispatch(tr);
			}
		} catch (err) {
			console.error("Error loading note:", err);
			error = `Failed to load note: ${err.message}`;
		} finally {
			isLoading = false;
			loadingInProgress = false;
		}
	}

	function createEditorView(element, state) {
		const { ydoc } = notesInstance.getDoc();

		const dispatchTransaction = async (tr) => {
			if (!view) return;

			try {
				const newState = view.state.apply(tr);
				view.updateState(newState);
				notesInstance.updateEditorState(newState);

				// Only trigger Yjs update if document actually changed
				if (tr.docChanged && ydoc) {
					// This will trigger the 'update' event on ydoc with default (local) origin
					// The Notes class will handle sending the update to peers

					// Dispatch event for collaboration - ONLY send minimal data
					// to avoid cyclic structure serialization issues
					dispatch("collaboration-update", {
						noteId: currentlyLoadedNoteId,
					});
				}
			} catch (err) {
				console.error("Error in dispatch transaction:", err);
			}
		};

		return new EditorView(element, {
			state,
			dispatchTransaction,
		});
	}

	function handleBackButton() {
		// Save before leaving
		if (view) {
			notesInstance.saveNote().catch(console.error);
		}

		// Return to list view
		noteViewLayout.set(false);

		// Clear current note ID
		currentlyLoadedNoteId = null;
	}

	// Initialize when component mounts
	onMount(async () => {
		console.log("RichTextEditor mounted");
		// Clear any state to ensure clean start
		currentlyLoadedNoteId = null;

		if ($noteId) {
			await loadNote($noteId);
		}
	});

	// Clean up when component is destroyed
	onDestroy(() => {
		console.log("RichTextEditor destroyed");
		if (unsubscribeUpdate) {
			unsubscribeUpdate();
		}
		if (view) {
			view.destroy();
			view = null;
		}
		if (autoSaveInterval) {
			clearInterval(autoSaveInterval);
		}
		// Save one final time on destroy
		notesInstance.saveNote().catch(console.error);

		// Clear current note ID
		currentlyLoadedNoteId = null;
	});
</script>

<style>
	.editor-container {
		margin: 0 auto;
		width: 100%;
		height: 100%;
		background: #16171f;
		color: white;
		display: flex;
		flex-direction: column;
	}

	.editor-header {
		display: flex;
		align-items: center;
		padding: 8px 16px;
		border-bottom: 1px solid #2a2b2f;
		background: #16171f;
	}

	.back-button {
		background: transparent;
		border: 1px solid #2a2b2f;
		color: #bfc0cc;
		padding: 6px 12px;
		border-radius: 4px;
		margin-right: 12px;
		cursor: pointer;
		transition: all 0.2s;
	}

	.back-button:hover {
		background: #2a2b2f;
		color: #f2f2f0;
	}

	.editor-main {
		flex: 1;
		overflow: auto;
		position: relative;
	}

	.loading-overlay {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(22, 23, 31, 0.7);
		z-index: 10;
	}

	.error-message {
		color: #ff6a6a;
		padding: 16px;
		text-align: center;
	}

	/* ProseMirror styles */
	:global(.ProseMirror) {
		position: relative;
		padding: 15px;
		min-height: 100px;
		outline: none;
		line-height: 1.5;
		color: white;
		background: #16171f;
	}

	:global(.ProseMirror p) {
		margin: 0 0 1em 0;
	}

	:global(.ProseMirror h1) {
		font-size: 2em;
		margin: 0.67em 0;
		color: white;
	}

	/* Cursor and selection styles */
	:global(.ProseMirror-yjs-cursor) {
		position: relative;
		margin-left: -1px;
		margin-right: -1px;
		border-left: 1px solid white;
		border-right: 1px solid white;
		pointer-events: none;
	}

	:global(.ProseMirror-yjs-cursor > div) {
		position: absolute;
		top: -1.05em;
		left: -1px;
		font-size: 13px;
		background-color: rgb(250, 129, 0);
		font-family: serif;
		font-style: normal;
		font-weight: normal;
		line-height: normal;
		user-select: none;
		color: white;
		padding: 2px 6px;
		border-radius: 3px;
		white-space: nowrap;
	}

	/* Other ProseMirror styles from your original file */
	:global(.ProseMirror-menubar-wrapper) {
		position: relative;
	}

	:global(.ProseMirror-menubar) {
		height: 48px;
		padding: 4px 8px;
		white-space: nowrap;
		overflow-x: auto;
		background: #16171f;
		display: flex;
		align-items: center;
		gap: 1px;
		border-bottom: 1px solid #2a2b2f;
	}

	:global(.ProseMirror-menu-dropdown-menu) {
		z-index: 999;
	}
</style>

<div class="editor-container">
	<div class="editor-header">
		<button class="back-button" on:click={handleBackButton}>
			← Back to Notes
		</button>
		<h2 class="text-osvauld-fieldText">
			{currentlyLoadedNoteId ? "Edit Note" : "New Note"}
		</h2>
	</div>

	<div class="editor-main">
		{#if isLoading}
			<div class="loading-overlay">
				<div class="text-osvauld-fieldText">Loading note...</div>
			</div>
		{:else if error}
			<div class="error-message">{error}</div>
		{/if}

		<div bind:this={element}></div>
	</div>
</div>
