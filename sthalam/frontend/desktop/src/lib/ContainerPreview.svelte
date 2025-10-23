<script lang="ts">
	interface Props {
		containerId: string;
		blocks: Map<string, any>;
		containerCss?: string;
		readonly?: boolean;
		selectedBlockId?: string | null;
		onSelect?: (blockId: string) => void;
		onUpdate?: (blockId: string, updates: any) => void;
	}

	let { containerId, blocks, containerCss = "", readonly = false, selectedBlockId = null, onSelect, onUpdate }: Props = $props();

	// Get container position to calculate relative positions
	const container = $derived(blocks.get(containerId));
	const containerX = $derived(container?.x || 0);
	const containerY = $derived(container?.y || 0);

	// Get all descendants of this container (recursive) with depth tracking
	function getAllDescendants(parentId: string, depth: number = 0): Array<{block: any, depth: number}> {
		const descendants: Array<{block: any, depth: number}> = [];
		const directChildren = Array.from(blocks.values())
			.filter(b => b.parentId === parentId);

		for (const child of directChildren) {
			descendants.push({ block: child, depth });
			// Get children of this child
			const grandchildren = getAllDescendants(child.id, depth + 1);
			descendants.push(...grandchildren);
		}

		return descendants;
	}

	const childrenWithDepth = $derived(getAllDescendants(containerId, 1));

	// Sort by depth so parents render first (lower z-index), children render on top
	const sortedChildren = $derived(
		childrenWithDepth.sort((a, b) => a.depth - b.depth)
	);

	// Render block content based on type
	function renderBlockContent(block: any): string {
		switch (block.type) {
			case 'heading':
				return block.content || 'Heading';
			case 'text':
				return block.content || 'Text';
			case 'markdown-text':
				return block.content || 'Markdown text';
			case 'nav-button':
				return block.content || 'Button';
			default:
				return '';
		}
	}

	// Get styles for a block with proper z-index and custom CSS
	// Positions are RELATIVE to the container, not absolute to canvas
	function getBlockStyles(block: any, depth: number): string {
		const styles = block.styles || {};

		// Calculate position relative to container
		const relativeX = block.x - containerX;
		const relativeY = block.y - containerY;

		const base = [
			`position: absolute`,
			`left: ${relativeX}px`,
			`top: ${relativeY}px`,
			`width: ${block.width}px`,
			`height: ${block.height}px`,
			`z-index: ${depth}`,
			`box-sizing: border-box`
		];

		// Add block.styles properties
		if (styles.fontSize) base.push(`font-size: ${styles.fontSize}`);
		if (styles.fontWeight) base.push(`font-weight: ${styles.fontWeight}`);
		if (styles.color) base.push(`color: ${styles.color}`);
		if (styles.backgroundColor) base.push(`background-color: ${styles.backgroundColor}`);
		if (styles.padding) base.push(`padding: ${styles.padding}`);
		if (styles.border) base.push(`border: ${styles.border}`);
		if (styles.borderRadius) base.push(`border-radius: ${styles.borderRadius}`);

		// Combine with block.css for full styling
		const baseStyles = base.join('; ');
		const customCss = block.css || '';

		return customCss ? `${baseStyles}; ${customCss}` : baseStyles;
	}
</script>

<div class="container-preview" style={containerCss}>
	{#each sortedChildren as { block: child, depth } (child.id)}
		{#if child.type === 'section-container'}
			<!-- Render section container as a div with its styles -->
			<div
				class="preview-section"
				class:selected={selectedBlockId === child.id}
				style={getBlockStyles(child, depth) + '; ' + (child.css || '')}
				onclick={(e) => {
					e.stopPropagation();
					if (onSelect && !readonly) onSelect(child.id);
				}}
				role="button"
				tabindex="0"
			>
				{child.name || 'Section'}
			</div>
		{:else if child.type === 'heading'}
			<h1
				class:selected={selectedBlockId === child.id}
				style={getBlockStyles(child, depth)}
				onclick={(e) => {
					e.stopPropagation();
					if (onSelect && !readonly) onSelect(child.id);
				}}
				role="button"
				tabindex="0"
			>{renderBlockContent(child)}</h1>
		{:else if child.type === 'text'}
			<p
				class:selected={selectedBlockId === child.id}
				style={getBlockStyles(child, depth)}
				onclick={(e) => {
					e.stopPropagation();
					if (onSelect && !readonly) onSelect(child.id);
				}}
				role="button"
				tabindex="0"
			>{renderBlockContent(child)}</p>
		{:else if child.type === 'nav-button'}
			<button
				class:selected={selectedBlockId === child.id}
				style={getBlockStyles(child, depth)}
				onclick={(e) => {
					e.stopPropagation();
					if (onSelect && !readonly) onSelect(child.id);
				}}
			>{renderBlockContent(child)}</button>
		{:else if child.type === 'image' && child.content}
			<img
				class:selected={selectedBlockId === child.id}
				src={child.content}
				alt="Content"
				style={getBlockStyles(child, depth)}
				onclick={(e) => {
					e.stopPropagation();
					if (onSelect && !readonly) onSelect(child.id);
				}}
				role="button"
				tabindex="0"
			/>
		{:else}
			<div
				class:selected={selectedBlockId === child.id}
				style={getBlockStyles(child, depth)}
				onclick={(e) => {
					e.stopPropagation();
					if (onSelect && !readonly) onSelect(child.id);
				}}
				role="button"
				tabindex="0"
			>{renderBlockContent(child)}</div>
		{/if}
	{/each}
</div>

<style>
	.container-preview {
		position: relative;
		width: 100%;
		height: auto; /* Grow with content */
		overflow: visible; /* Show all content */
		pointer-events: auto; /* Enable clicking */
	}

	.preview-section {
		position: relative;
		box-sizing: border-box;
		pointer-events: auto;
	}

	h1, p, button, div, img {
		margin: 0;
		box-sizing: border-box;
		pointer-events: auto; /* Make all elements clickable */
		cursor: pointer; /* Show all elements are clickable in builder */
		transition: all 0.15s;
	}

	h1:hover, p:hover, div:hover, img:hover, .preview-section:hover {
		box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.3);
		transform: translateY(-1px);
	}

	button {
		cursor: pointer; /* Make buttons look clickable */
	}

	button:hover {
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
	}

	.selected {
		outline: 3px solid #89b4fa !important;
		outline-offset: 2px;
		box-shadow: 0 0 0 1px #89b4fa, 0 0 20px rgba(137, 180, 250, 0.4) !important;
	}
</style>
