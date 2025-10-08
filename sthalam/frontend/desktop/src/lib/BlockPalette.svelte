<script lang="ts">
	interface Props {
		onAddBlock: (type: string) => void;
	}

	let { onAddBlock }: Props = $props();

	const blockTypes = [
		{ type: "heading", label: "Heading", icon: "H", description: "Large title text" },
		{ type: "text", label: "Text", icon: "T", description: "Paragraph text" },
		{ type: "image", label: "Image", icon: "🖼️", description: "Image or GIF" },
		{ type: "container", label: "Container", icon: "□", description: "Layout box for grouping" },
		{ type: "html", label: "HTML/CSS", icon: "<>", description: "Custom HTML with CSS" },
	];
</script>

<div class="palette">
	<div class="palette-header">
		<h3>Blocks</h3>
	</div>

	<div class="palette-content">
		{#each blockTypes as blockType}
			<button
				class="block-type-btn"
				onclick={() => onAddBlock(blockType.type)}
				title="Add {blockType.label}"
			>
				<span class="icon">{blockType.icon}</span>
				<span class="label">{blockType.label}</span>
			</button>
		{/each}
	</div>
</div>

<style>
	.palette {
		width: 200px;
		height: 100%;
		background: var(--bg-primary, #ffffff);
		border-right: 1px solid var(--border-color, #e0e0e0);
		display: flex;
		flex-direction: column;
	}

	.palette-header {
		padding: 1rem;
		border-bottom: 1px solid var(--border-color, #e0e0e0);
	}

	.palette-header h3 {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--text-primary, #333);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.palette-content {
		flex: 1;
		padding: 1rem;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.block-type-btn {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem;
		border: 1px solid var(--border-color, #e0e0e0);
		border-radius: 6px;
		background: var(--bg-primary, white);
		cursor: pointer;
		transition: all 0.2s;
		text-align: left;
		width: 100%;
	}

	.block-type-btn:hover {
		background: var(--bg-hover, #f5f5ff);
		border-color: #667eea;
		box-shadow: 0 2px 8px rgba(102, 126, 234, 0.1);
	}

	.icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
		border-radius: 6px;
		font-weight: 600;
		font-size: 16px;
	}

	.label {
		font-size: 0.875rem;
		color: var(--text-primary, #333);
		font-weight: 500;
	}
</style>
