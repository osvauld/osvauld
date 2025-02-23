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

			// Update the state in Notes instance
			notesInstance.updateEditorState(newState);

			if (tr.docChanged && ydoc) {
				const update = Y.encodeStateAsUpdate(ydoc);
				await notesInstance.handleCollaborationUpdate(update);

				dispatch("collaboration-update", {
					update: Array.from(update),
					clientID: notesInstance.getDoc().clientID,
				});
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
			console.log("insidedsfasdfasdf");
			try {
				const parsed = JSON.parse(event.payload);
				const { update, clientID: remoteClientID } = JSON.parse(parsed);
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
		height: 100%;
		width: 100%;
		max-width: 1200px;
		display: flex;
		flex-direction: column;
	}

	.editor {
		flex-grow: 1;
		background: white;
		border: 1px solid #e2e8f0;
		padding: 1rem;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
	}

	:global(.ProseMirror) {
		position: relative;
		word-wrap: break-word;
		white-space: pre-wrap;
		-webkit-font-variant-ligatures: none;
		font-variant-ligatures: none;
		padding: 4px 8px 4px 14px;
		line-height: 1.2;
		outline: none;
		min-height: 100px;
	}

	:global(.ProseMirror p) {
		margin: 0;
		min-height: 1.2em;
	}

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
		background-color: inherit;
		color: white;
		padding: 0 4px;
		white-space: nowrap;
	}
</style>

<div class="editor-container">
	{#if view}
		<EditorToolbar editorView={view} />
	{/if}
	<div bind:this={element} class="editor"></div>
</div>
