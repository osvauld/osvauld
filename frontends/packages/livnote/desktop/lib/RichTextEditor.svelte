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
	import { Awareness } from "y-protocols/awareness";
	import EditorToolbar from "./EditorToolbar.svelte";
	import { listen } from "@tauri-apps/api/event";

	export let clientID = Math.floor(Math.random() * 0xffffffff);
	const dispatch = createEventDispatcher();
	let element;
	let view;
	let ydoc;
	let type;
	let awareness;

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

	// Create initial doc if needed
	const createDefaultDoc = () => {
		return editorSchema.node("doc", null, [
			editorSchema.node("paragraph", null, []),
		]);
	};

	// Initialize Yjs document and awareness
	function initYjs() {
		const ydoc = new Y.Doc();
		const type = ydoc.getXmlFragment("prosemirror");
		const awareness = new Awareness(ydoc);

		awareness.setLocalState({
			user: {
				name: `User ${clientID}`,
				color: `#${Math.floor(Math.random() * 16777215).toString(16)}`,
			},
		});

		return { ydoc, type, awareness };
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

	function createEditorView(element, state, yDoc) {
		let editorView;

		const dispatchTransaction = (tr) => {
			if (!editorView) return;

			const newState = editorView.state.apply(tr);
			editorView.updateState(newState);

			if (tr.docChanged && yDoc) {
				const update = Y.encodeStateAsUpdate(yDoc);
				dispatch("collaboration-update", {
					update: Array.from(update),
					clientID,
				});
			}
		};

		editorView = new EditorView(element, {
			state,
			dispatchTransaction,
		});

		return editorView;
	}

	// Apply incoming updates from other clients
	export function applyUpdate(update, sender) {
		if (!ydoc || sender === clientID) return;

		try {
			const updateArray =
				update instanceof Uint8Array ? update : new Uint8Array(update);
			Y.applyUpdate(ydoc, updateArray);
		} catch (err) {
			console.error("Error applying update:", err);
		}
	}

	let unsubscribe;

	onMount(async () => {
		if (!element) return;

		try {
			// Initialize Yjs and create document
			const yjs = initYjs();
			ydoc = yjs.ydoc;
			type = yjs.type;
			awareness = yjs.awareness;

			// Create editor state
			const state = createEditorState(type, awareness);

			// Create editor view
			view = createEditorView(element, state, ydoc);

			// Setup update listener
			unsubscribe = await listen("sync-update-be", (event) => {
				try {
					console.log("Received event payload:", event.payload);
					const parsed = JSON.parse(event.payload);
					console.log("Parsed event payload:", parsed);
					const { update, clientID: remoteClientID } = JSON.parse(parsed);
					applyUpdate(update, remoteClientID);
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
		if (ydoc) {
			ydoc.destroy();
		}
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
