<!-- ProseMirror.svelte -->
<script>
	import { onMount, onDestroy } from "svelte";
	import { EditorState } from "prosemirror-state";
	import { EditorView } from "prosemirror-view";
	import { Schema } from "prosemirror-model";
	import { schema } from "prosemirror-schema-basic";
	import { addListNodes } from "prosemirror-schema-list";
	import { baseKeymap } from "prosemirror-commands";
	import { keymap } from "prosemirror-keymap";
	import { history, undo, redo } from "prosemirror-history";

	// Extend the basic schema with list support
	const mySchema = new Schema({
		nodes: addListNodes(schema.spec.nodes, "paragraph block*", "block"),
		marks: schema.spec.marks,
	});

	let element;
	let view;

	onMount(() => {
		// Create the initial editor state
		const state = EditorState.create({
			schema: mySchema,
			plugins: [
				history(),
				keymap({ "Mod-z": undo, "Mod-y": redo }),
				keymap(baseKeymap),
			],
		});

		// Initialize the editor view
		view = new EditorView(element, {
			state,
			dispatchTransaction(transaction) {
				// Update the editor's state with each transaction
				let newState = view.state.apply(transaction);
				view.updateState(newState);
			},
		});
	});

	onDestroy(() => {
		if (view) {
			view.destroy();
		}
	});

	// Function to get editor content (can be exposed as a prop)
	export function getContent() {
		if (view) {
			return view.state.doc.toJSON();
		}
		return null;
	}

	// Function to set editor content (can be exposed as a prop)
	export function setContent(content) {
		if (view && content) {
			const state = EditorState.create({
				schema: mySchema,
				doc: mySchema.nodeFromJSON(content),
				plugins: view.state.plugins,
			});
			view.updateState(state);
		}
	}
</script>

<style>
	.editor-wrapper {
		position: relative;
		width: 100%;
		max-width: 800px;
		margin: 0 auto;
	}

	.editor {
		background: white;
		padding: 1.5rem;
		border: 1px solid #e2e8f0;
		border-radius: 0.5rem;
		min-height: 300px;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
	}

	/* Core editor styling */
	.editor :global(.ProseMirror) {
		min-height: inherit;
		outline: none;
		line-height: 1.6;
		color: #1a1a1a;
		font-size: 1rem;
	}

	/* Placeholder styling */
	.editor :global(.ProseMirror[data-placeholder]::before) {
		content: attr(data-placeholder);
		color: #94a3b8;
		pointer-events: none;
		position: absolute;
		opacity: 0.6;
		font-style: italic;
	}

	.editor :global(.ProseMirror-focused[data-placeholder]::before) {
		display: none;
	}

	/* Typography and spacing */
	.editor :global(.ProseMirror p) {
		margin: 0.8em 0;
		line-height: 1.6;
	}

	.editor :global(.ProseMirror h1) {
		font-size: 2em;
		margin: 1em 0 0.5em;
		font-weight: 600;
		color: #1a1a1a;
	}

	.editor :global(.ProseMirror h2) {
		font-size: 1.5em;
		margin: 1em 0 0.5em;
		font-weight: 600;
		color: #1a1a1a;
	}

	/* Lists */
	.editor :global(.ProseMirror ul) {
		padding-left: 1.5em;
		margin: 0.8em 0;
	}

	.editor :global(.ProseMirror li) {
		margin: 0.3em 0;
	}

	/* Blockquotes */
	.editor :global(.ProseMirror blockquote) {
		border-left: 3px solid #e2e8f0;
		margin: 1em 0;
		padding: 0.5em 0 0.5em 1em;
		color: #4a5568;
		font-style: italic;
	}

	/* Selection */
	.editor :global(.ProseMirror-selectednode) {
		outline: 2px solid #60a5fa;
	}

	/* Links */
	.editor :global(.ProseMirror a) {
		color: #2563eb;
		text-decoration: underline;
		text-underline-offset: 2px;
	}

	/* Code blocks */
	.editor :global(.ProseMirror pre) {
		background: #f8f9fa;
		padding: 0.75em 1em;
		border-radius: 0.25em;
		font-family: monospace;
		margin: 0.8em 0;
	}

	/* Inline code */
	.editor :global(.ProseMirror code) {
		background: #f1f5f9;
		padding: 0.2em 0.4em;
		border-radius: 0.25em;
		font-size: 0.9em;
		font-family: monospace;
	}
</style>

<div bind:this={element} class="editor"></div>
