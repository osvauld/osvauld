<script>
	import { onMount, onDestroy, createEventDispatcher } from "svelte";
	import { EditorView } from "prosemirror-view";
	import * as Y from "yjs";
	import { listen } from "@tauri-apps/api/event";
	import EditorToolbar from "./EditorToolbar.svelte";
	import { notesInstance } from "./utils/notes";
	import { noteId } from "../../store/desktop.ui.store";

	const dispatch = createEventDispatcher();
	let element;
	let view;
	let autoSaveInterval;

	async function initializeEditor() {
		if (!element) return;

		try {
			let docInfo;

			if ($noteId) {
				// Load existing note
				docInfo = await notesInstance.loadNote($noteId);
			} else {
				//Get fresh doc for new note
				docInfo = notesInstance.getDoc();
			}

			const { editorState } = docInfo;

			// Create editor view
			view = createEditorView(element, editorState);

			// Setup auto-save
			autoSaveInterval = setInterval(() => {
				notesInstance.saveNote().catch(console.error);
			}, 30000); // Auto-save every 30 seconds
		} catch (err) {
			console.error("Error initializing editor:", err);
		}
	}
	function createEditorView(element, state) {
		const { ydoc } = notesInstance.getDoc();

		const dispatchTransaction = async (tr) => {
			if (!view) return;

			const newState = view.state.apply(tr);
			view.updateState(newState);
			notesInstance.updateEditorState(newState);

			// Only trigger Yjs update if document actually changed
			if (tr.docChanged && ydoc) {
				// This will trigger the 'update' event on ydoc with default (local) origin
				const update = Y.encodeStateAsUpdate(ydoc);
				await notesInstance.handleCollaborationUpdate(update);
			}
		};

		return new EditorView(element, {
			state,
			dispatchTransaction,
		});
	}

	let unsubscribe;

	onMount(async () => {
		await initializeEditor();

		unsubscribe = await listen("sync-update-be", (event) => {
			console.log(event, "sdafsdaf");
			try {
				const parsed = JSON.parse(event.payload);
				const { update, clientID: remoteClientID } = parsed;
				// Remote update will be applied with 'sync' origin
				notesInstance.applyUpdate(update, remoteClientID);
			} catch (err) {
				console.error("Error handling update:", err);
			}
		});
	});
	onDestroy(() => {
		if (unsubscribe) {
			unsubscribe();
		}
		if (view) {
			view.destroy();
		}
		if (autoSaveInterval) {
			clearInterval(autoSaveInterval);
		}
		// Save one final time on destroy
		notesInstance.saveNote().catch(console.error);
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
	<div bind:this={element}></div>
</div>
