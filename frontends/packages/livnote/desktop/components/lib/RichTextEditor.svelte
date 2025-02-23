<script>
	import { onMount, onDestroy, createEventDispatcher } from "svelte";
	import { EditorState } from "prosemirror-state";
	import { EditorView } from "prosemirror-view";
	import { Schema } from "prosemirror-model";
	import { schema } from "prosemirror-schema-basic";
	import { addListNodes } from "prosemirror-schema-list";
	import { baseKeymap } from "prosemirror-commands";
	import { keymap } from "prosemirror-keymap";
	import { history } from "prosemirror-history";
	import {
		ySyncPlugin,
		yCursorPlugin,
		yUndoPlugin,
		undo,
		redo,
	} from "y-prosemirror";
	import * as Y from "yjs";
	import { listen } from "@tauri-apps/api/event";
	import EditorToolbar from "./EditorToolbar.svelte";
	import { editorInstance } from "./utils/editor.ts";

	const dispatch = createEventDispatcher();
	let element;
	let view;

	// Define the schema
	const nodes = addListNodes(schema.spec.nodes, "paragraph block*", "block");
	const marks = {
		...schema.spec.marks,
		textColor: {
			attrs: { color: { default: "" } },
			parseDOM: [
				{
					style: "color",
					getAttrs: (value) => ({ color: value }),
				},
			],
			toDOM: (mark) => ["span", { style: `color: ${mark.attrs.color}` }, 0],
		},
	};
	const editorSchema = new Schema({ nodes, marks });

	function createDefaultDoc() {
		return editorSchema.node("doc", null, [
			editorSchema.node("paragraph", null, []),
		]);
	}

	function createEditorState(ytype, awareness) {
		return EditorState.create({
			doc: createDefaultDoc(),
			schema: editorSchema,
			plugins: [
				history(),
				keymap(baseKeymap),
				ySyncPlugin(ytype),
				yCursorPlugin(awareness),
				yUndoPlugin(),
				keymap({
					"Mod-z": undo,
					"Mod-y": redo,
					"Mod-Shift-z": redo,
				}),
			],
		});
	}

	function createEditorView(element, state) {
		const { ydoc } = editorInstance.getYjsDoc();

		const dispatchTransaction = (tr) => {
			if (!view) return;

			const newState = view.state.apply(tr);
			view.updateState(newState);

			if (tr.docChanged && ydoc) {
				const update = Y.encodeStateAsUpdate(ydoc);
				dispatch("collaboration-update", {
					update: Array.from(update),
					clientID: editorInstance.getYjsDoc().clientID,
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
		if (!element) return;

		try {
			const { type, awareness } = editorInstance.getYjsDoc();

			// Create editor state
			const state = createEditorState(type, awareness);

			// Create editor view
			view = createEditorView(element, state);

			// Setup update listener
			unsubscribe = await listen("sync-update-be", (event) => {
				try {
					console.log("Received event payload:", event.payload);
					const parsed = JSON.parse(event.payload);
					console.log("Parsed event payload:", parsed);
					const { update, clientID: remoteClientID } = JSON.parse(parsed);
					editorInstance.applyUpdate(update, remoteClientID);
				} catch (err) {
					console.error("Error handling update:", err);
				}
			});
		} catch (err) {
			console.error("Error during editor initialization:", err);
		}
	});

	onDestroy(() => {
		if (unsubscribe) {
			unsubscribe();
		}
		if (view) {
			view.destroy();
		}
	});

	function handleChange(event) {
		editorInstance.handleCollaborationUpdate(event.detail);
	}
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
