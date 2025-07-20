<script lang="ts">
	// Props using Svelte 5 syntax
	interface Props {
		previewHTML: string;
		maxHeight?: string;
		minHeight?: string;
	}

	let {
		previewHTML = "",
		maxHeight = "180px",
		minHeight = "180px",
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

	/* Style the HTML content to match ProseMirror styles */
	:global(.note-preview p) {
		margin: 0 0 0.5em 0;
		color: white;
	}

	:global(.note-preview h1) {
		font-size: 1.5em;
		margin: 0.5em 0;
		color: white;
		font-weight: 600;
	}

	:global(.note-preview h2) {
		font-size: 1.3em;
		margin: 0.4em 0;
		color: white;
		font-weight: 600;
	}

	:global(.note-preview h3) {
		font-size: 1.2em;
		margin: 0.3em 0;
		color: white;
		font-weight: 600;
	}

	:global(.note-preview ul),
	:global(.note-preview ol) {
		padding-left: 1.2em;
		margin: 0.5em 0;
		color: white;
	}

	:global(.note-preview li) {
		margin: 0.2em 0;
		color: white;
	}

	:global(.note-preview blockquote) {
		border-left: 3px solid #8c9eff;
		margin-left: 0;
		padding-left: 0.8em;
		color: #bfc0cc;
		margin: 0.5em 0;
	}

	:global(.note-preview code) {
		background: #2a2b2f;
		padding: 0.1em 0.3em;
		border-radius: 3px;
		font-family: monospace;
		color: #e6e6e6;
	}

	:global(.note-preview pre) {
		background: #2a2b2f;
		padding: 0.5em;
		border-radius: 3px;
		overflow-x: auto;
		margin: 0.5em 0;
	}

	:global(.note-preview pre code) {
		background: transparent;
		padding: 0;
	}

	:global(.note-preview strong) {
		font-weight: 600;
		color: white;
	}

	:global(.note-preview em) {
		font-style: italic;
		color: white;
	}

	:global(.note-preview a) {
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
		color: #666;
		font-style: italic;
	}
</style>

<div
	class="preview-container note-preview"
	style="max-height: {maxHeight}; min-height: {minHeight}">
	{@html previewHTML}
</div>
