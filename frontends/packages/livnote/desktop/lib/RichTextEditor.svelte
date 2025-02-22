<script>
	import { onMount, onDestroy, createEventDispatcher } from "svelte";
	import { EditorState, Plugin, PluginKey } from "prosemirror-state";
	import { EditorView } from "prosemirror-view";
	import { Schema } from "prosemirror-model";
	import { schema } from "prosemirror-schema-basic";
	import { addListNodes } from "prosemirror-schema-list";
	import { baseKeymap } from "prosemirror-commands";
	import { keymap } from "prosemirror-keymap";
	import { history, undo, redo } from "prosemirror-history";
	import { listen } from "@tauri-apps/api/event";
	import {
		splitListItem,
		liftListItem,
		sinkListItem,
	} from "prosemirror-schema-list";
	import {
		collab,
		sendableSteps,
		receiveTransaction,
	} from "prosemirror-collab";
	import { Step } from "prosemirror-transform";
	import EditorToolbar from "./EditorToolbar.svelte";
	export let content = null;
	export let placeholder = "Start writing...";
	export let readonly = false;
	export let clientID = Math.floor(Math.random() * 0xffffffff);
	export let version = 0;

	export function receiveSteps(steps, remoteClientID, newVersion) {
		if (!view) return;

		try {
			// Convert the JSON steps back to actual Step instances
			const convertedSteps = steps.map((stepJSON) =>
				Step.fromJSON(view.state.schema, stepJSON),
			);

			// Create a transaction that applies these steps
			const tr = view.state.tr;
			convertedSteps.forEach((step) => {
				tr.step(step);
			});

			// Update version and apply the transaction
			view.dispatch(
				tr.setMeta("collab", {
					version: newVersion,
					clientID: remoteClientID,
				}),
			);
		} catch (err) {
			console.error("Error applying collaborative steps:", err);
		}
	}
	let isDarkMode = false;
	const dispatch = createEventDispatcher();
	let element;
	let view;

	const editorSchema = new Schema({
		nodes: addListNodes(schema.spec.nodes, "paragraph block*", "block"),
		marks: {
			...schema.spec.marks,
			textColor: {
				attrs: { color: {} },
				parseDOM: [
					{
						style: "color",
						getAttrs: (value) => ({ color: value }),
					},
				],
				toDOM: (mark) => ["span", { style: `color: ${mark.attrs.color}` }, 0],
			},
			strong: {
				parseDOM: [
					{ tag: "strong" },
					{ tag: "b" },
					{ style: "font-weight", getAttrs: (value) => value === "bold" },
				],
				toDOM: () => ["strong", 0],
			},
			em: {
				parseDOM: [{ tag: "i" }, { tag: "em" }, { style: "font-style=italic" }],
				toDOM: () => ["em", 0],
			},
		},
	});

	const selectionPluginKey = new PluginKey("selection");
	const selectionPlugin = new Plugin({
		key: selectionPluginKey,
		view(editorView) {
			return {
				update: (view, prevState) => {
					if (view.state.selection !== prevState.selection) {
						view.dispatch(view.state.tr);
					}
				},
			};
		},
	});

	function createEditorState(initialContent = null) {
		let doc = null;

		if (initialContent) {
			try {
				doc = editorSchema.nodeFromJSON(initialContent);
			} catch (err) {
				console.error("Error parsing initial content:", err);
			}
		}

		const listKeymap = {
			Enter: splitListItem(editorSchema.nodes.list_item),
			Tab: sinkListItem(editorSchema.nodes.list_item),
			"Shift-Tab": liftListItem(editorSchema.nodes.list_item),
			"Shift-Enter": baseKeymap.Enter,
		};

		return EditorState.create({
			schema: editorSchema,
			doc: doc || undefined,
			plugins: [
				collab({ version }),
				history(),
				keymap({
					"Mod-z": undo,
					"Mod-y": redo,
					"Mod-Shift-z": redo,
				}),
				keymap(listKeymap),
				keymap(baseKeymap),
				selectionPlugin,
			],
		});
	}
	function getSendableSteps(state) {
		const plugin = state.plugins.find((plugin) =>
			plugin.key.startsWith("collab$"),
		);
		if (!plugin) return null;

		const sendable = {
			version: plugin.getState(state).version,
			steps: [],
			clientID: clientID,
		};

		// Get only the new steps since last sync
		console.log("getting steps....");
		const steps = sendableSteps(state);
		console.log("step", steps);
		if (steps) {
			sendable.version = steps.version;
			sendable.steps = steps.steps;
		}

		return sendable.steps.length ? sendable : null;
	}

	onMount(async () => {
		view = new EditorView(element, {
			state: createEditorState(content),
			dispatchTransaction(transaction) {
				const newState = view.state.apply(transaction);
				view.updateState(newState);

				if (view.dom.hasAttribute("data-placeholder")) {
					view.dom.removeAttribute("data-placeholder");
				}

				// Handle collaborative editing updates
				if (transaction.docChanged) {
					const sendableSteps = getSendableSteps(newState);
					console.log("sendable steps", sendableSteps);
					if (sendableSteps) {
						console.log("sending steps");
						dispatch("collaboration-update", {
							version: sendableSteps.version,
							steps: sendableSteps.steps,
							clientID: clientID,
						});
					}
				}
			},
			attributes: {
				"data-placeholder": placeholder,
			},
			editable: () => !readonly,
		});
		let unsubscribe = await listen("sync-update-be", (event) => {
			try {
				const {
					steps,
					clientID: remoteClientID,
					version: newVersion,
				} = JSON.parse(JSON.parse(event.payload));

				console.log(steps, "asdfasdfsdafdsaf");
				// Don't apply our own steps
				if (remoteClientID === clientID) return;

				if (steps && view) {
					receiveSteps(steps, remoteClientID, newVersion);
				}
				// if (newContent && view) {
				// 	setContent(newContent);
				// }
			} catch (err) {
				console.error("Error handling status-update-be event:", err);
			}
		});
	});

	onDestroy(() => {
		if (view) {
			view.destroy();
		}
	});

	export function getContent() {
		if (view) {
			return view.state.doc.toJSON();
		}
		return null;
	}

	export function setContent(newContent) {
		if (view && newContent) {
			try {
				const newState = EditorState.create({
					schema: editorSchema,
					doc: editorSchema.nodeFromJSON(newContent),
					plugins: view.state.plugins,
				});
				view.updateState(newState);
			} catch (err) {
				console.error("Error setting new content:", err);
			}
		}
	}

	$: if (view && content) {
		setContent(content);
	}
</script>

<style>
	.editor-fullscreen {
		width: 100vw;
		height: 100vh;
		background: gray;
		display: flex;
		justify-content: center;
		align-items: center;
	}

	.editor-container {
		height: 100%;
		width: 100%;
		max-width: 1200px;
		display: flex;
		flex-direction: column;
	}

	.editor {
		flex-grow: 1;
		background: #fafafa;
		border: 1px solid #e2e8f0;
		padding: 3rem;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
	}

	.editor.dark {
		background-color: #212121;
	}

	.editor :global(.ProseMirror) {
		height: 100%;
		outline: none;
		line-height: 1.6;
		color: #1a1a1a;
		font-size: 1rem;
	}

	.editor.dark :global(.ProseMirror) {
		color: #e2e8f0;
	}

	.editor :global(.ProseMirror pre) {
		background: #1e293b;
		color: #e2e8f0;
		padding: 1rem;
		border-radius: 0.5rem;
		font-family: "Menlo", "Monaco", "Courier New", monospace;
		margin: 1rem 0;
		overflow-x: auto;
	}

	.editor :global(.ProseMirror code) {
		font-family: "Menlo", "Monaco", "Courier New", monospace;
	}

	.editor :global(.ProseMirror ul) {
		padding-left: 1.5em;
		list-style-type: disc;
		margin: 0.5em 0;
	}

	.editor :global(.ProseMirror ul ul) {
		list-style-type: circle;
	}

	.editor :global(.ProseMirror ul ul ul) {
		list-style-type: square;
	}

	.editor :global(.ProseMirror ol) {
		padding-left: 1.5em;
		list-style-type: decimal;
		margin: 0.5em 0;
	}

	.editor :global(.ProseMirror ol ol) {
		list-style-type: lower-alpha;
	}

	.editor :global(.ProseMirror ol ol ol) {
		list-style-type: lower-roman;
	}

	.editor :global(.ProseMirror li) {
		margin: 0.2em 0;
		position: relative;
	}

	.editor :global(.ProseMirror li p) {
		margin: 0;
	}

	.editor :global(.ProseMirror li:hover) {
		background-color: rgba(0, 0, 0, 0.02);
	}

	.editor :global(.ProseMirror[data-placeholder]::before) {
		content: attr(data-placeholder);
		color: #94a3b8;
		pointer-events: none;
		position: absolute;
		opacity: 0.6;
	}
</style>

<div class="editor-fullscreen">
	<div class="editor-container">
		<div bind:this={element} class="editor" class:dark={isDarkMode}></div>
		{#if view}
			<EditorToolbar editorView={view} bind:isDarkMode />
		{/if}
	</div>
</div>
