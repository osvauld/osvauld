<script lang="ts">
	interface Props {
		block: {
			id: string;
			type: string;
			x: number;
			y: number;
			width: number;
			height: number;
			zIndex: number;
			content: string;
			styles: Record<string, string>;
		};
		isSelected: boolean;
		onUpdate: (blockId: string, updates: any) => void;
		onSelect: (blockId: string) => void;
	}

	let { block, isSelected, onUpdate, onSelect }: Props = $props();

	let isDragging = $state(false);
	let isResizing = $state(false);
	let resizeHandle = $state<string | null>(null);
	let dragStart = $state({ x: 0, y: 0 });
	let blockStart = $state({ x: 0, y: 0, width: 0, height: 0 });

	function handleMouseDown(e: MouseEvent) {
		// Don't start drag if clicking on contenteditable or resize handle
		const target = e.target as HTMLElement;
		if (target.contentEditable === "true" || target.classList.contains("resize-handle")) {
			return;
		}

		// Select the block
		onSelect(block.id);

		isDragging = true;
		dragStart = { x: e.clientX, y: e.clientY };
		blockStart = { x: block.x, y: block.y, width: block.width, height: block.height };
		e.stopPropagation(); // Prevent canvas panning
	}

	function handleResizeStart(e: MouseEvent, handle: string) {
		e.stopPropagation();
		isResizing = true;
		resizeHandle = handle;
		dragStart = { x: e.clientX, y: e.clientY };
		blockStart = { x: block.x, y: block.y, width: block.width, height: block.height };
	}

	function handleMouseMove(e: MouseEvent) {
		if (isDragging) {
			const dx = e.clientX - dragStart.x;
			const dy = e.clientY - dragStart.y;

			onUpdate(block.id, {
				x: blockStart.x + dx,
				y: blockStart.y + dy,
			});
		} else if (isResizing && resizeHandle) {
			const dx = e.clientX - dragStart.x;
			const dy = e.clientY - dragStart.y;

			let updates: any = {};

			switch (resizeHandle) {
				case "nw": // Top-left
					updates = {
						x: blockStart.x + dx,
						y: blockStart.y + dy,
						width: Math.max(50, blockStart.width - dx),
						height: Math.max(30, blockStart.height - dy),
					};
					break;
				case "n": // Top
					updates = {
						y: blockStart.y + dy,
						height: Math.max(30, blockStart.height - dy),
					};
					break;
				case "ne": // Top-right
					updates = {
						y: blockStart.y + dy,
						width: Math.max(50, blockStart.width + dx),
						height: Math.max(30, blockStart.height - dy),
					};
					break;
				case "e": // Right
					updates = {
						width: Math.max(50, blockStart.width + dx),
					};
					break;
				case "se": // Bottom-right
					updates = {
						width: Math.max(50, blockStart.width + dx),
						height: Math.max(30, blockStart.height + dy),
					};
					break;
				case "s": // Bottom
					updates = {
						height: Math.max(30, blockStart.height + dy),
					};
					break;
				case "sw": // Bottom-left
					updates = {
						x: blockStart.x + dx,
						width: Math.max(50, blockStart.width - dx),
						height: Math.max(30, blockStart.height + dy),
					};
					break;
				case "w": // Left
					updates = {
						x: blockStart.x + dx,
						width: Math.max(50, blockStart.width - dx),
					};
					break;
			}

			onUpdate(block.id, updates);
		}
	}

	function handleMouseUp() {
		isDragging = false;
		isResizing = false;
		resizeHandle = null;
	}

	// Compute dynamic styles
	const computedStyles = $derived(() => {
		const baseStyles = {
			fontSize: block.styles.fontSize || "16px",
			fontWeight: block.styles.fontWeight || "400",
			color: block.styles.color || "#333",
			backgroundColor: block.styles.backgroundColor || "transparent",
			padding: block.styles.padding || "12px",
			border: block.styles.border || "2px solid #ddd",
			borderRadius: block.styles.borderRadius || "4px",
		};

		return Object.entries(baseStyles)
			.map(([key, value]) => `${key}: ${value}`)
			.join("; ");
	});
</script>

<svelte:window onmousemove={handleMouseMove} onmouseup={handleMouseUp} />

<div
	class="block block-{block.type}"
	style:left="{block.x}px"
	style:top="{block.y}px"
	style:width="{block.width}px"
	style:height="{block.height}px"
	style:z-index={block.zIndex}
	style={computedStyles()}
	onmousedown={handleMouseDown}
	class:dragging={isDragging}
	class:resizing={isResizing}
	class:selected={isSelected}
>
	{#if block.type === "heading"}
		<div class="block-heading" contenteditable="true">
			{block.content}
		</div>
	{:else if block.type === "text"}
		<div class="block-text" contenteditable="true">
			{block.content}
		</div>
	{:else if block.type === "container"}
		<div class="block-container">
			{block.content || "Container"}
		</div>
	{/if}

	<!-- Resize handles -->
	<div class="resize-handle resize-nw" onmousedown={(e) => handleResizeStart(e, "nw")}></div>
	<div class="resize-handle resize-n" onmousedown={(e) => handleResizeStart(e, "n")}></div>
	<div class="resize-handle resize-ne" onmousedown={(e) => handleResizeStart(e, "ne")}></div>
	<div class="resize-handle resize-e" onmousedown={(e) => handleResizeStart(e, "e")}></div>
	<div class="resize-handle resize-se" onmousedown={(e) => handleResizeStart(e, "se")}></div>
	<div class="resize-handle resize-s" onmousedown={(e) => handleResizeStart(e, "s")}></div>
	<div class="resize-handle resize-sw" onmousedown={(e) => handleResizeStart(e, "sw")}></div>
	<div class="resize-handle resize-w" onmousedown={(e) => handleResizeStart(e, "w")}></div>
</div>

<style>
	.block {
		position: absolute;
		cursor: move;
		transition: box-shadow 0.2s;
		user-select: none;
	}

	.block:hover {
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
	}

	.block:hover .resize-handle,
	.block.selected .resize-handle {
		opacity: 1;
	}

	.block.selected {
		box-shadow: 0 0 0 3px #667eea;
	}

	.block.dragging {
		opacity: 0.8;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
	}

	.block.resizing {
		opacity: 0.8;
	}

	.block-heading,
	.block-text,
	.block-container {
		width: 100%;
		height: 100%;
		outline: none;
		overflow: auto;
	}

	.block-heading {
		font-weight: bold;
	}

	[contenteditable="true"] {
		cursor: text;
	}

	/* Resize handles */
	.resize-handle {
		position: absolute;
		background: #667eea;
		border: 2px solid white;
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
		opacity: 0;
		transition: opacity 0.2s;
	}

	/* Corner handles */
	.resize-nw,
	.resize-ne,
	.resize-se,
	.resize-sw {
		width: 10px;
		height: 10px;
		border-radius: 50%;
	}

	/* Edge handles */
	.resize-n,
	.resize-s {
		width: 20px;
		height: 6px;
		left: 50%;
		transform: translateX(-50%);
		border-radius: 3px;
	}

	.resize-e,
	.resize-w {
		width: 6px;
		height: 20px;
		top: 50%;
		transform: translateY(-50%);
		border-radius: 3px;
	}

	/* Positioning */
	.resize-nw {
		top: -5px;
		left: -5px;
		cursor: nw-resize;
	}

	.resize-n {
		top: -3px;
		cursor: n-resize;
	}

	.resize-ne {
		top: -5px;
		right: -5px;
		cursor: ne-resize;
	}

	.resize-e {
		right: -3px;
		cursor: e-resize;
	}

	.resize-se {
		bottom: -5px;
		right: -5px;
		cursor: se-resize;
	}

	.resize-s {
		bottom: -3px;
		cursor: s-resize;
	}

	.resize-sw {
		bottom: -5px;
		left: -5px;
		cursor: sw-resize;
	}

	.resize-w {
		left: -3px;
		cursor: w-resize;
	}
</style>
