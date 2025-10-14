<script lang="ts">
	import {
		Maximize,
		FolderIcon,
		FileText,
		EditIcon,
		CommentIcon,
		MobileNote,
		Tick,
		RightArrow
	} from "@osvauld/icons";

	interface Props {
		onAddBlock: (type: string) => void;
	}

	let { onAddBlock }: Props = $props();

	const blockTypes = [
		{ type: "screen-container", label: "Screen Container", icon: Maximize, description: "Main page container (responsive root)" },
		{ type: "section-container", label: "Section Container", icon: FolderIcon, description: "Layout section with CSS" },
		{ type: "heading", label: "Heading", icon: FileText, description: "Large title text" },
		{ type: "text", label: "Text", icon: EditIcon, description: "Paragraph text" },
		{ type: "image", label: "Image", icon: FileText, description: "Image or GIF" },
		{ type: "thread", label: "Thread", icon: CommentIcon, description: "Collaborative comment thread" },
		{ type: "form", label: "Form", icon: MobileNote, description: "Form metadata (invisible in viewer)" },
		{ type: "form-field-text", label: "Text Input", icon: EditIcon, description: "Single-line text field" },
		{ type: "form-field-textarea", label: "Text Area", icon: MobileNote, description: "Multi-line text field" },
		{ type: "form-field-checkbox", label: "Checkbox", icon: Tick, description: "Checkbox field" },
		{ type: "form-submit-button", label: "Submit Button", icon: RightArrow, description: "Form submit button" },
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
				<span class="icon">
					<svelte:component this={blockType.icon} size={20} color="#1e1e2e" />
				</span>
				<span class="label">{blockType.label}</span>
			</button>
		{/each}
	</div>
</div>

<style>
	.palette {
		width: 200px;
		height: 100%;
		background: #010409;
		border-right: 1px solid #21262d;
		display: flex;
		flex-direction: column;
	}

	.palette-header {
		padding: 1rem;
		border-bottom: 1px solid #21262d;
	}

	.palette-header h3 {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: #c9d1d9;
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.palette-content {
		flex: 1;
		padding: 1rem;
		padding-bottom: 2rem; /* Add extra padding at bottom */
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		min-height: 0; /* Enable flex scrolling */
	}

	.palette-content::-webkit-scrollbar {
		width: 8px;
	}

	.palette-content::-webkit-scrollbar-track {
		background: transparent;
	}

	.palette-content::-webkit-scrollbar-thumb {
		background: #30363d;
		border-radius: 4px;
	}

	.palette-content::-webkit-scrollbar-thumb:hover {
		background: #484f58;
	}

	.block-type-btn {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem;
		border: 1px solid #30363d;
		border-radius: 6px;
		background: #0d0e13;
		cursor: pointer;
		transition: all 0.2s;
		text-align: left;
		width: 100%;
	}

	.block-type-btn:hover {
		background: #161b22;
		border-color: #667eea;
		box-shadow: 0 2px 8px rgba(102, 126, 234, 0.2);
	}

	.icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		min-width: 32px;
		background: #89b4fa;
		color: #1e1e2e;
		border-radius: 6px;
		font-weight: 600;
		font-size: 16px;
	}

	.icon :global(svg) {
		width: 20px;
		height: 20px;
	}

	.label {
		font-size: 0.875rem;
		color: #c9d1d9;
		font-weight: 500;
	}
</style>
