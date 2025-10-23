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
		onAddBlock: (type: string, x?: number, y?: number) => void;
		onImportTemplate?: () => void;
	}

	let { onAddBlock, onImportTemplate }: Props = $props();

	function handleDragStart(e: DragEvent, blockType: string) {
		// Set the block type in drag data
		e.dataTransfer!.effectAllowed = 'copy';
		e.dataTransfer!.setData('application/block-type', blockType);

		// Add visual feedback
		const target = e.currentTarget as HTMLElement;
		target.style.opacity = '0.5';
	}

	function handleDragEnd(e: DragEvent) {
		// Reset visual feedback
		const target = e.currentTarget as HTMLElement;
		target.style.opacity = '1';
	}

	const blockTypes = [
		{ type: "screen-container", label: "Screen Container", icon: Maximize, description: "Main page container (responsive root)" },
		{ type: "section-container", label: "Section Container", icon: FolderIcon, description: "Layout section with CSS" },
		{ type: "heading", label: "Heading", icon: FileText, description: "Large title text" },
		{ type: "text", label: "Text", icon: EditIcon, description: "Paragraph text" },
		{ type: "markdown-text", label: "Markdown Text", icon: FileText, description: "Rich text with markdown formatting" },
		{ type: "image", label: "Image", icon: FileText, description: "Image or GIF" },
		{ type: "thread", label: "Thread", icon: CommentIcon, description: "Collaborative comment thread" },
		{ type: "form", label: "Form", icon: MobileNote, description: "Form metadata (invisible in viewer)" },
		{ type: "form-field-text", label: "Text Input", icon: EditIcon, description: "Single-line text field" },
		{ type: "form-field-textarea", label: "Text Area", icon: MobileNote, description: "Multi-line text field" },
		{ type: "form-field-checkbox", label: "Checkbox", icon: Tick, description: "Checkbox field" },
		{ type: "nav-button", label: "Navigation Button", icon: RightArrow, description: "Navigate, submit forms, or set field values (replaces Submit Button + Branching Question)" },
		{ type: "form-submit-button", label: "Submit Button (Deprecated)", icon: RightArrow, description: "⚠️ Use Nav Button with formId instead" },
		{ type: "branching-question", label: "Branching Question (Deprecated)", icon: CommentIcon, description: "⚠️ Use Nav Button with formId + fieldName + value instead" },
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
				draggable="true"
				ondragstart={(e) => handleDragStart(e, blockType.type)}
				ondragend={handleDragEnd}
				onclick={() => onAddBlock(blockType.type)}
				title="Drag to canvas or click to add {blockType.label}"
			>
				<span class="icon">
					<svelte:component this={blockType.icon} size={20} color="#1e1e2e" />
				</span>
				<span class="label">{blockType.label}</span>
			</button>
		{/each}
	</div>

	{#if onImportTemplate}
		<div class="palette-footer">
			<button class="import-btn" onclick={onImportTemplate} title="Import HUML template">
				📥 Import Template
			</button>
		</div>
	{/if}
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
		cursor: grab;
		transition: all 0.2s;
		text-align: left;
		width: 100%;
	}

	.block-type-btn:active {
		cursor: grabbing;
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

	.palette-footer {
		padding: 1rem;
		border-top: 1px solid #21262d;
	}

	.import-btn {
		width: 100%;
		padding: 0.75rem;
		border: 1px solid #89b4fa;
		border-radius: 6px;
		background: rgba(137, 180, 250, 0.1);
		color: #89b4fa;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	.import-btn:hover {
		background: rgba(137, 180, 250, 0.2);
		box-shadow: 0 2px 8px rgba(137, 180, 250, 0.3);
	}
</style>
