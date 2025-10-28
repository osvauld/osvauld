<script lang="ts">
	import { marked } from 'marked';

	interface Props {
		blockId: string;
		blockData: any;
	}

	let { blockId, blockData }: Props = $props();

	// Render markdown content
	const markdownHtml = $derived.by(() => {
		if (!blockData.content) return '';
		try {
			const raw = marked.parse(blockData.content);
			return raw;
		} catch (error) {
			console.error('Error rendering markdown:', error);
			return '<p>Error rendering markdown</p>';
		}
	});
</script>

<div class="markdown-text" style={blockData.css || ""} data-block-id={blockId}>
	{@html markdownHtml}
</div>

<style>
	.markdown-text {
		width: 100%;
		color: #c9d1d9;
		line-height: 1.6;
	}

	.markdown-text :global(h1) {
		font-size: 2rem;
		font-weight: 600;
		margin-bottom: 1rem;
		color: #c9d1d9;
	}

	.markdown-text :global(h2) {
		font-size: 1.5rem;
		font-weight: 600;
		margin-bottom: 0.75rem;
		margin-top: 1.5rem;
		color: #c9d1d9;
	}

	.markdown-text :global(h3) {
		font-size: 1.25rem;
		font-weight: 600;
		margin-bottom: 0.5rem;
		margin-top: 1rem;
		color: #c9d1d9;
	}

	.markdown-text :global(p) {
		margin-bottom: 1rem;
	}

	.markdown-text :global(a) {
		color: #89b4fa;
		text-decoration: none;
	}

	.markdown-text :global(a:hover) {
		text-decoration: underline;
	}

	.markdown-text :global(ul),
	.markdown-text :global(ol) {
		margin-bottom: 1rem;
		padding-left: 2rem;
	}

	.markdown-text :global(li) {
		margin-bottom: 0.5rem;
	}

	.markdown-text :global(code) {
		background: #0d0e13;
		padding: 0.2em 0.4em;
		border-radius: 3px;
		font-family: monospace;
		font-size: 0.9em;
		color: #a6e3a1;
	}

	.markdown-text :global(pre) {
		background: #0d0e13;
		padding: 1rem;
		border-radius: 8px;
		overflow-x: auto;
		margin-bottom: 1rem;
		border: 1px solid #30363d;
	}

	.markdown-text :global(pre code) {
		background: transparent;
		padding: 0;
	}

	.markdown-text :global(blockquote) {
		border-left: 4px solid #89b4fa;
		padding-left: 1rem;
		margin-left: 0;
		margin-bottom: 1rem;
		color: #8b949e;
	}

	.markdown-text :global(img) {
		max-width: 100%;
		height: auto;
		border-radius: 8px;
		margin: 1rem 0;
	}

	.markdown-text :global(hr) {
		border: none;
		border-top: 1px solid #30363d;
		margin: 2rem 0;
	}

	.markdown-text :global(table) {
		width: 100%;
		border-collapse: collapse;
		margin-bottom: 1rem;
	}

	.markdown-text :global(th),
	.markdown-text :global(td) {
		border: 1px solid #30363d;
		padding: 0.5rem;
		text-align: left;
	}

	.markdown-text :global(th) {
		background: #0d0e13;
		font-weight: 600;
	}

	.markdown-text :global(strong) {
		font-weight: 600;
		color: #c9d1d9;
	}

	.markdown-text :global(em) {
		font-style: italic;
	}
</style>
