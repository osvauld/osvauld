<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { EditorView } from "prosemirror-view";
	import { EditorState } from "prosemirror-state";
	import { Schema } from "prosemirror-model";
	import { schema } from "prosemirror-schema-basic";
	import { addListNodes } from "prosemirror-schema-list";
	import * as Y from "yjs";

	export let content = "";
	export const title: string = "";
	export let editorState = null;
	export let yjsState = null;
	export let maxHeight = "180px";
	export let minHeight = "180px";

	let element: HTMLElement | null = null;
	let view;

	// Initialize the preview on mount
	onMount(() => {
		if (!element) return;

		try {
			// Create schema
			const editorSchema = new Schema({
				nodes: addListNodes(schema.spec.nodes, "paragraph block*", "block"),
				marks: schema.spec.marks,
			});

			let state;

			// Try to initialize from yjs state if available
			if (yjsState && Array.isArray(yjsState)) {
				const ydoc = new Y.Doc();
				const yXmlFragment = ydoc.getXmlFragment("prosemirror");

				// Apply YJS state
				Y.applyUpdate(ydoc, new Uint8Array(yjsState));

				// Create editor state from the YJS content
				state = EditorState.create({
					schema: editorSchema,
					doc: editorSchema.nodeFromJSON(
						editorState?.doc || {
							type: "doc",
							content: [{ type: "paragraph" }],
						},
					),
				});
			}
			// Fallback to direct editor state if available
			else if (editorState) {
				state = EditorState.create({
					schema: editorSchema,
					doc: editorSchema.nodeFromJSON(
						editorState.doc || {
							type: "doc",
							content: [{ type: "paragraph" }],
						},
					),
				});
			}
			// Last resort: try to parse HTML content
			else if (content) {
				// This is a simplified approach - in a real implementation,
				// you would use a proper HTML parser for ProseMirror
				state = EditorState.create({
					schema: editorSchema,
					doc: editorSchema.node("doc", {}, [
						editorSchema.node("paragraph", {}, [
							editorSchema.text(content.replace(/<[^>]+>/g, " ")),
						]),
					]),
				});
			}
			// Create an empty document if nothing else works
			else {
				state = EditorState.create({
					schema: editorSchema,
					doc: editorSchema.node("doc", {}, [editorSchema.node("paragraph")]),
				});
			}

			// Create a read-only view
			view = new EditorView(element, {
				state,
				editable: () => false, // Make it read-only
				dispatchTransaction: () => {}, // No-op since it's read-only
			});
		} catch (err) {
			console.error("Error initializing note preview:", err);
		}
	});

	onDestroy(() => {
		if (view) {
			view.destroy();
		}
	});
</script>

<style>
	.preview-container {
		width: 100%;
		overflow: hidden;
		border-radius: 0.25rem;
		position: relative;
	}

	.preview-container::after {
		content: "";
		position: absolute;
		bottom: 0;
		left: 0;
		right: 0;
		height: 40px;
		background: linear-gradient(transparent, rgba(22, 23, 31, 0.9));
		pointer-events: none;
	}

	/* ProseMirror styles */
	:global(.note-preview .ProseMirror) {
		position: relative;
		outline: none;
		line-height: 1.5;
		color: white;
		background: transparent;
		word-break: break-word;
		white-space: pre-wrap;
		padding: 0;
		overflow: hidden;
	}

	:global(.note-preview .ProseMirror p) {
		margin: 0 0 0.5em 0;
	}

	:global(.note-preview .ProseMirror h1) {
		font-size: 1.5em;
		margin: 0.5em 0;
		color: white;
	}

	:global(.note-preview .ProseMirror h2) {
		font-size: 1.3em;
		margin: 0.4em 0;
		color: white;
	}

	:global(.note-preview .ProseMirror h3) {
		font-size: 1.2em;
		margin: 0.3em 0;
		color: white;
	}

	:global(.note-preview .ProseMirror ul),
	:global(.note-preview .ProseMirror ol) {
		padding-left: 1.2em;
		margin: 0.5em 0;
	}

	:global(.note-preview .ProseMirror blockquote) {
		border-left: 3px solid #8c9eff;
		margin-left: 0;
		padding-left: 0.8em;
		color: #bfc0cc;
	}

	:global(.note-preview .ProseMirror code) {
		background: #2a2b2f;
		padding: 0.1em 0.3em;
		border-radius: 3px;
		font-family: monospace;
	}

	:global(.note-preview .ProseMirror pre) {
		background: #2a2b2f;
		padding: 0.5em;
		border-radius: 3px;
		overflow-x: auto;
	}
</style>

<div
	class="preview-container note-preview"
	style="max-height: {maxHeight}; min-height: {minHeight}">
	<div bind:this="{element}"></div>
</div>
