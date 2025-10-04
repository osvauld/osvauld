<script lang="ts">
	// Props using Svelte 5 syntax
	import { fade, fly } from "svelte/transition";
	interface Props {
		previewHTML: string;
		maxHeight?: string;
		minHeight?: string;
	}

	let {
		previewHTML = "",
		maxHeight = "130px",
		minHeight = "130px",
	}: Props = $props();
</script>

<style>
	.preview-container {
		width: 100%;
		overflow: hidden;
		border-radius: 0.25rem;
		position: relative;
		line-height: 1.5;
		color: white;
		word-break: break-word;
		white-space: pre-wrap;
		padding: 5px;
	}

	/* Style the HTML content to match ProseMirror styles */
	:global(.note-preview p) {
		font-size: 12px;
		margin: 0 0 0.5em 0;
		color: white;
	}

	:global(.note-preview h1) {
		font-size: 18px; /* 1.5em of 12px */
		margin: 0.5em 0;
		color: white;
		font-weight: 600;
	}

	:global(.note-preview h2) {
		font-size: 15.6px; /* 1.3em of 12px */
		margin: 0.4em 0;
		color: white;
		font-weight: 600;
	}

	:global(.note-preview h3) {
		font-size: 14.4px; /* 1.2em of 12px */
		margin: 0.3em 0;
		color: white;
		font-weight: 600;
	}

	:global(.note-preview ul),
	:global(.note-preview ol) {
		font-size: 12px;
		padding-left: 1.2em;
		margin: 0.5em 0;
		color: white;
	}

	:global(.note-preview li) {
		font-size: 12px;
		margin: 0.2em 0;
		color: white;
	}

	:global(.note-preview blockquote) {
		font-size: 12px;
		border-left: 3px solid #8c9eff;
		margin-left: 0;
		padding-left: 0.8em;
		color: #bfc0cc;
		margin: 0.5em 0;
	}

	:global(.note-preview code) {
		font-size: 11px; /* Slightly smaller for inline code */
		background: #2a2b2f;
		padding: 0.1em 0.3em;
		border-radius: 3px;
		font-family: monospace;
		color: #e6e6e6;
	}

	:global(.note-preview pre) {
		font-size: 11px; /* Slightly smaller for code blocks */
		background: #2a2b2f;
		padding: 0.5em;
		border-radius: 3px;
		overflow-x: auto;
		margin: 0.5em 0;
	}

	:global(.note-preview pre code) {
		font-size: inherit; /* Inherit from pre */
		background: transparent;
		padding: 0;
	}

	:global(.note-preview strong) {
		font-size: inherit; /* Inherit from parent */
		font-weight: 600;
		color: white;
	}

	:global(.note-preview em) {
		font-size: inherit; /* Inherit from parent */
		font-style: italic;
		color: white;
	}

	:global(.note-preview a) {
		font-size: inherit; /* Inherit from parent */
		color: #8c9eff;
		text-decoration: underline;
	}

	:global(.note-preview img) {
		max-width: 100%;
		height: auto;
		border-radius: 4px;
		margin: 0.5em 0;
	}

	/* Handle empty content */
	:global(.note-preview:empty::before) {
		content: "No content available";
		font-size: 14px;
		color: #a3a4b5;
		font-style: italic;
	}
	:global(.note-preview table) {
		font-size: 12px;
		border-collapse: collapse;
		width: 100%;
		margin: 0.5em 0;
		background: rgba(42, 43, 47, 0.5);
		border-radius: 4px;
		overflow: hidden;
	}

	:global(.note-preview th),
	:global(.note-preview td) {
		font-size: inherit; /* Inherit from table */
		border: 1px solid rgba(255, 255, 255, 0.1);
		padding: 6px 9px; /* Reduced padding proportionally */
		text-align: left;
		color: white;
	}

	:global(.note-preview th) {
		background: rgba(140, 158, 255, 0.1);
		font-weight: 600;
		color: #e6e6e6;
	}

	:global(.note-preview td) {
		background: rgba(42, 43, 47, 0.3);
	}

	:global(.note-preview tr:hover td) {
		background: rgba(140, 158, 255, 0.05);
	}

	/* Remove paragraph margins inside table cells */
	:global(.note-preview td p),
	:global(.note-preview th p) {
		margin: 0;
	}
</style>

<div
	class="preview-container note-preview"
	in:fly={{ y: 10, duration: 200, delay: 0 }}
	style="max-height: {maxHeight}; min-height: {minHeight}"
>
	{@html previewHTML}
</div>
