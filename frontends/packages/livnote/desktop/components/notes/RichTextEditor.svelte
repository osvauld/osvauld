<script>
	import {
		onMount,
		onDestroy,
		createEventDispatcher,
		getContext,
	} from "svelte";
	import { EditorView } from "prosemirror-view";
	import { listen } from "@tauri-apps/api/event";
	import { notesInstance } from "./notes";
	import {
		noteId,
		noteViewLayout,
		refreshCredentialList,
	} from "../../store/desktop.ui.store";

	const dispatch = createEventDispatcher();
	let element;
	let view = null;
	let autoSaveInterval;
	let unsubscribeUpdate;
	let isLoading = false;
	let error = null;
	let currentlyLoadedNoteId = null;
	let loadingInProgress = false;
	let saved = false;
	const saveNoteAndSwitch = getContext("saveNoteAndSwitchFunction");

	// Listen for noteId changes and load the corresponding note
	$: if (
		$noteId &&
		element &&
		$noteId !== currentlyLoadedNoteId &&
		!loadingInProgress
	) {
		loadNote($noteId);
	}

	saveNoteAndSwitch(() => {
		if (view) {
			notesInstance.saveNote().catch(console.error);
		}

		// Return to list view
		noteViewLayout.set(false);

		// Clear current note ID
		currentlyLoadedNoteId = null;
	});

	async function saveNoteManual() {
		saved = true;
		notesInstance
			.saveNote()
			.catch(console.error)
			.then(() => refreshCredentialList.set(true));

		setTimeout(() => {
			saved = false;
		}, 1000);
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
				// Savign animation go

				notesInstance.saveNote().catch(console.error);
				saved = true;
				setTimeout(() => {
					saved = false;
				}, 1000);
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

	const prosemirrorInstanceDestructionHandle = () => {
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
		notesInstance
			.saveNote()
			.catch(console.error)
			.then(() => refreshCredentialList.set(true));

		// Clear current note ID
		currentlyLoadedNoteId = null;
	};

	// Initialize when component mounts
	onMount(async () => {
		console.log("RichTextEditor mounted");
		// Clear any state to ensure clean start
		currentlyLoadedNoteId = null;

		if ($noteId) {
			await loadNote($noteId);
		}
	});

	// We need to do cleanup when noteId Changes

	noteId.subscribe((id) => {
		if (id) prosemirrorInstanceDestructionHandle();
	});

	// Clean up when component is destroyed
	onDestroy(() => {
		console.log("RichTextEditor destroyed");
		prosemirrorInstanceDestructionHandle();
	});
</script>

<style>
	.editor-container {
		margin: 0 auto;
		width: 100%;
		height: 100%;
		background: #16171f;
		color: white;
	}

	/* ProseMirror menubar styles for horizontal layout */
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
		z-index: 900;
		border-bottom: 1px solid #2a2b2f;
	}
	:global(.ProseMirror) {
		position: relative;
		padding: 15px;
		min-height: 100px;
		outline: none;
		line-height: 1.5;
		color: white;
		background: #16171f;
	}

	:global(.ProseMirror-menuitem) {
		display: inline-flex;
		align-items: center;
		height: 24px;
		margin-right: 1px;
		cursor: pointer;
	}

	:global(.ProseMirror-menu-dropdown) {
		vertical-align: middle;
		padding: 2px 4px;
		font-size: 14px;
		color: white;
	}

	:global(.ProseMirror-menu-dropdown-wrap) {
		position: relative;
		display: inline-block;
	}

	:global(.ProseMirror-menu-dropdown-menu) {
		position: fixed;
		background: #16171f;
		border: 1px solid #2a2b2f;
		border-radius: 2px;
		padding: 2px 0;
		min-width: 67px;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
	}

	:global(.ProseMirror-menu-dropdown-item) {
		padding: 2px 8px;
		cursor: pointer;
		font-size: 14px;
		color: white;
	}

	:global(.ProseMirror-menu-dropdown-item:hover) {
		background: #2a2b2f;
	}

	:global(.ProseMirror-icon) {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 2px;
		cursor: pointer;
		border: 1px solid transparent;
		border-radius: 2px;
		font-size: 16px;
		color: white;
	}

	:global(.ProseMirror-icon svg) {
		fill: currentColor;
		color: white;
	}

	:global(.ProseMirror-icon:hover) {
		background: #2a2b2f;
	}

	:global(.ProseMirror-menu-disabled) {
		opacity: 0.3;
	}

	:global(.ProseMirror-icon span) {
		color: white;
		font-weight: bold;
	}

	:global(.ProseMirror-menu-dropdown-item:hover) {
		background: #2a2b2f;
	}

	:global(.ProseMirror-icon) {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 2px;
		cursor: pointer;
		border: 1px solid transparent;
		border-radius: 2px;
		font-size: 16px;
		color: white;
	}

	:global(.ProseMirror-icon:hover) {
		background: #2a2b2f;
	}

	:global(.ProseMirror) {
		position: relative;
		padding: 15px;
		min-height: 100px;
		outline: none;
		line-height: 1.5;
		color: white;
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

	:global(.ProseMirror-icon:hover) {
		border-color: #ddd;
		background: #e5e5e5;
	}

	:global(.ProseMirror) {
		position: relative;
		padding: 15px;
		min-height: 100px;
		outline: none;
		line-height: 1.5;
	}

	:global(.ProseMirror p) {
		margin: 0 0 1em 0;
	}

	:global(.ProseMirror h1) {
		font-size: 2em;
		margin: 0.67em 0;
	}

	/* Cursor and selection styles */
	:global(.ProseMirror-yjs-cursor) {
		position: relative;
		margin-left: -1px;
		margin-right: -1px;
		border-left: 1px solid black;
		border-right: 1px solid black;
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
	:global(.ProseMirror-menu-dropdown-menu) {
		z-index: 999;
	}
</style>

<div class="editor-container">
	<div class="editor-main relative h-full">
		{#if isLoading}
			<div
				class="loading-overlay flex justify-center items-center h-full w-full">
				<div class="text-osvauld-fieldText">Loading note...</div>
			</div>
		{:else if error}
			<div class="error-message">{error}</div>
		{/if}

		<div bind:this="{element}"></div>
		<button
			on:click="{saveNoteManual}"
			class="absolute w-20 top-1.5 right-2 bg-osvauld-carolinablue text-osvauld-fieldActive px-2.5 py-1 rounded-md cursor-pointer"
			>{saved ? "Saved" : "Save"}</button>
	</div>
</div>
