<script lang="ts">
	import TreeNode from "./TreeNode.svelte";
	import TreeGuideLines from "./TreeGuideLines.svelte";
	import {
		layoutAllScreens,
		getScreenBlocks,
		DEFAULT_TREE_CONFIG,
		type TreeLayout
	} from "./treeLayoutEngine";

	interface Props {
		blocks: Map<string, any>;
		viewport: { x: number; y: number; zoom: number };
		selectedBlockId: string | null;
		readonly?: boolean;
		onViewportChange: (viewport: { x?: number; y?: number; zoom?: number }) => void;
		onBlockUpdate: (blockId: string, updates: any) => void;
		onBlockSelect: (blockId: string) => void;
		onConnectionSelect?: (blockId: string, connectionId: string) => void;
		onBlockContextMenu?: (blockId: string) => void;
		onBlockDrop?: (blockType: string, x: number, y: number) => void;
	}

	let { blocks, viewport, selectedBlockId, readonly = false, onViewportChange, onBlockUpdate, onBlockSelect, onConnectionSelect, onBlockContextMenu, onBlockDrop }: Props = $props();

	let canvasContainer = $state<HTMLDivElement>();
	let canvasElement = $state<HTMLDivElement>();
	let isPanning = $state(false);
	let panStart = $state({ x: 0, y: 0 });
	let viewportUpdateTimeout: number | null = null;

	function handleMouseDown(e: MouseEvent) {
		// In viewer mode (readonly), allow panning anywhere on canvas
		// In builder mode, only pan if clicking on canvas background
		const canPan = readonly || e.target === canvasContainer || e.target === canvasElement;

		if (canPan) {
			isPanning = true;
			panStart = {
				x: e.clientX - viewport.x,
				y: e.clientY - viewport.y,
			};
			e.preventDefault(); // Prevent text selection while panning
		}
	}

	// Track local viewport during pan for smooth rendering
	let localViewport = $state({ x: 0, y: 0 });

	$effect(() => {
		// Sync local viewport with prop viewport when not panning
		if (!isPanning) {
			localViewport = { x: viewport.x, y: viewport.y };
		}
	});

	function handleMouseMove(e: MouseEvent) {
		if (isPanning) {
			const newX = e.clientX - panStart.x;
			const newY = e.clientY - panStart.y;

			// Update local viewport for smooth visual feedback
			localViewport = { x: newX, y: newY };

			// Throttle updates to parent to prevent excessive Yjs updates
			if (viewportUpdateTimeout !== null) {
				clearTimeout(viewportUpdateTimeout);
			}

			viewportUpdateTimeout = window.setTimeout(() => {
				onViewportChange({ x: newX, y: newY });
				viewportUpdateTimeout = null;
			}, 50); // Update every 50ms while panning
		}
	}

	function handleMouseUp(e: MouseEvent) {
		if (isPanning) {
			// Final update when done panning
			onViewportChange({ x: localViewport.x, y: localViewport.y });
			if (viewportUpdateTimeout !== null) {
				clearTimeout(viewportUpdateTimeout);
				viewportUpdateTimeout = null;
			}
		}
		isPanning = false;
	}

	// Zoom handler (Ctrl/Cmd + scroll)
	function handleWheel(e: WheelEvent) {
		// Only zoom if Ctrl or Cmd key is pressed
		if (e.ctrlKey || e.metaKey) {
			e.preventDefault();

			const delta = -e.deltaY;
			const zoomFactor = delta > 0 ? 1.1 : 0.9;
			const newZoom = Math.max(0.1, Math.min(3, viewport.zoom * zoomFactor));

			// Zoom towards mouse position
			const rect = canvasContainer?.getBoundingClientRect();
			if (rect) {
				const mouseX = e.clientX - rect.left;
				const mouseY = e.clientY - rect.top;

				// Calculate the point under the mouse in canvas space
				const canvasX = (mouseX - localViewport.x) / viewport.zoom;
				const canvasY = (mouseY - localViewport.y) / viewport.zoom;

				// Calculate new viewport position to keep point under mouse
				const newX = mouseX - canvasX * newZoom;
				const newY = mouseY - canvasY * newZoom;

				localViewport = { x: newX, y: newY };
				onViewportChange({ x: newX, y: newY, zoom: newZoom });
			}
		}
	}

	// Handle drag and drop from palette
	function handleDragOver(e: DragEvent) {
		// Prevent default to allow drop
		e.preventDefault();
		if (e.dataTransfer) {
			e.dataTransfer.dropEffect = 'copy';
		}
	}

	function handleDrop(e: DragEvent) {
		e.preventDefault();

		// Get the block type from drag data
		const blockType = e.dataTransfer?.getData('application/block-type');
		if (!blockType || !onBlockDrop || !canvasContainer) return;

		// Get mouse position in screen coordinates
		const mouseScreenX = e.clientX;
		const mouseScreenY = e.clientY;

		// Get canvas container's position
		const rect = canvasContainer.getBoundingClientRect();

		// Mouse position relative to canvas container
		const relativeX = mouseScreenX - rect.left;
		const relativeY = mouseScreenY - rect.top;

		// The canvas element has transform: translate(viewport.x, viewport.y) scale(viewport.zoom)
		// To convert from screen position to canvas coordinates:
		const canvasX = (relativeX - viewport.x) / viewport.zoom;
		const canvasY = (relativeY - viewport.y) / viewport.zoom;

		// Call the drop handler
		onBlockDrop(blockType, canvasX, canvasY);
	}

	// Tree view state
	let collapsedBlocks = $state<Set<string>>(new Set());

	function toggleCollapse(blockId: string) {
		if (collapsedBlocks.has(blockId)) {
			collapsedBlocks.delete(blockId);
		} else {
			collapsedBlocks.add(blockId);
		}
		// Trigger reactivity
		collapsedBlocks = new Set(collapsedBlocks);
	}

	// Calculate tree layouts
	let treeLayouts = $derived.by(() => {
		const screens = getScreenBlocks(blocks);
		return layoutAllScreens(screens, blocks, collapsedBlocks, DEFAULT_TREE_CONFIG);
	});

	// Flatten tree layouts for rendering
	let flatTreeNodes = $derived.by(() => {
		const nodes: Array<{ blockId: string; layout: TreeLayout; block: any }> = [];

		for (const [screenId, screenLayout] of treeLayouts.entries()) {
			for (const [blockId, layout] of screenLayout.entries()) {
				const block = blocks.get(blockId);
				if (block) {
					nodes.push({ blockId, layout, block });
				}
			}
		}

		return nodes;
	});

	// Get all blocks that are in the tree (to exclude them from orphaned list)
	let blocksInTree = $derived(new Set(flatTreeNodes.map(n => n.blockId)));

	// Get orphaned blocks (blocks without parents that aren't screens)
	let orphanedBlocks = $derived.by(() => {
		const orphaned: Array<{ blockId: string; block: any }> = [];

		for (const [blockId, block] of blocks.entries()) {
			// Skip if already in tree
			if (blocksInTree.has(blockId)) continue;

			// Add blocks that don't have a parent and aren't screen containers
			if (!block.parentId) {
				orphaned.push({ blockId, block });
			}
		}

		return orphaned;
	});
</script>

<svelte:window onmousemove={handleMouseMove} onmouseup={handleMouseUp} />

<div
	class="canvas-container"
	bind:this={canvasContainer}
	onmousedown={handleMouseDown}
	onwheel={handleWheel}
	ondragover={handleDragOver}
	ondrop={handleDrop}
	style:cursor={isPanning ? "grabbing" : "grab"}
>
	<div
		class="canvas"
		bind:this={canvasElement}
		style:transform="translate({localViewport.x}px, {localViewport.y}px) scale({viewport.zoom})"
	>
		<!-- Tree View -->
		<div
			class="tree-container"
			ondragover={handleDragOver}
			ondrop={handleDrop}
		>
			<!-- Tree guide lines layer -->
			<TreeGuideLines {treeLayouts} {blocks} rowHeight={DEFAULT_TREE_CONFIG.rowHeight} indent={DEFAULT_TREE_CONFIG.indent} />

			<!-- Tree nodes -->
			{#each flatTreeNodes as { blockId, layout, block } (blockId)}
				<div
					class="tree-node-wrapper"
					style:position="absolute"
					style:left="{layout.x}px"
					style:top="{layout.y}px"
				>
					<TreeNode
						{block}
						depth={layout.depth}
						isExpanded={layout.isExpanded}
						isSelected={selectedBlockId === blockId}
						onToggle={(e) => toggleCollapse(blockId)}
						onSelect={() => onBlockSelect(blockId)}
					/>
				</div>
			{/each}

			<!-- Orphaned blocks (blocks without parents) -->
			{#each orphanedBlocks as { blockId, block } (blockId)}
				<div
					class="orphaned-block"
					style:position="absolute"
					style:left="{block.x}px"
					style:top="{block.y}px"
					style:width="{block.width}px"
					style:height="{block.height}px"
					onclick={() => onBlockSelect(blockId)}
				>
					<div class="orphaned-block-content" class:selected={selectedBlockId === blockId}>
						<div class="orphaned-block-header">
							<span class="orphaned-icon">{block.type === 'screen-container' ? '🖥️' : block.type === 'section-container' ? '📦' : '📄'}</span>
							<span class="orphaned-label">{block.name || block.content || block.type}</span>
						</div>
						<div class="orphaned-hint">No parent - use Properties to assign</div>
					</div>
				</div>
			{/each}
		</div>
	</div>

	<!-- Zoom controls -->
	<div class="zoom-controls">
		<button
			class="zoom-btn"
			onclick={() => onViewportChange({ zoom: Math.min(3, viewport.zoom * 1.2) })}
			title="Zoom In (Ctrl+Scroll Up)"
		>
			+
		</button>
		<div class="zoom-level">{Math.round(viewport.zoom * 100)}%</div>
		<button
			class="zoom-btn"
			onclick={() => onViewportChange({ zoom: Math.max(0.1, viewport.zoom / 1.2) })}
			title="Zoom Out (Ctrl+Scroll Down)"
		>
			−
		</button>
		<button
			class="zoom-btn"
			onclick={() => onViewportChange({ zoom: 1 })}
			title="Reset Zoom"
		>
			⊙
		</button>
	</div>
</div>

<style>
	.canvas-container {
		width: 100%;
		height: 100%;
		overflow: hidden;
		position: relative;
		background: #0d0e13;
		user-select: none;
		-webkit-user-select: none;
		-moz-user-select: none;
		-ms-user-select: none;
		/* Force GPU acceleration */
		transform: translateZ(0);
		will-change: contents;
	}

	.canvas {
		position: relative;
		width: 100%;
		height: 100%;
		transform-origin: 0 0;
		user-select: none;
		-webkit-user-select: none;
		-moz-user-select: none;
		-ms-user-select: none;
		/* Better text rendering when scaled */
		-webkit-font-smoothing: subpixel-antialiased;
		-moz-osx-font-smoothing: auto;
		font-smooth: always;
		text-rendering: geometricPrecision;
		/* Force hardware acceleration for smoother transforms */
		will-change: transform;
		backface-visibility: hidden;
		transform-style: preserve-3d;
		/* Remove image rendering that affects text */
	}

	/* Zoom controls */
	.zoom-controls {
		position: absolute;
		bottom: 20px;
		right: 20px;
		display: flex;
		align-items: center;
		gap: 8px;
		background: #161b22;
		padding: 8px 12px;
		border-radius: 8px;
		border: 1px solid #30363d;
		box-shadow: 0 2px 12px rgba(0, 0, 0, 0.5);
		z-index: 1000;
	}

	.zoom-btn {
		width: 32px;
		height: 32px;
		border: 1px solid #30363d;
		border-radius: 6px;
		background: #0d0e13;
		color: #c9d1d9;
		font-size: 18px;
		font-weight: 600;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all 0.2s;
		padding: 0;
	}

	.zoom-btn:hover {
		background: #667eea;
		color: white;
		border-color: #667eea;
		transform: translateY(-1px);
		box-shadow: 0 2px 8px rgba(102, 126, 234, 0.3);
	}

	.zoom-btn:active {
		transform: translateY(0);
	}

	.zoom-level {
		font-size: 13px;
		font-weight: 600;
		color: #8b949e;
		min-width: 50px;
		text-align: center;
		font-family: monospace;
	}

	.tree-container {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
	}

	.tree-node-wrapper {
		pointer-events: none;
		width: fit-content;
		max-width: 500px;
		/* Better text rendering */
		-webkit-font-smoothing: subpixel-antialiased;
		-moz-osx-font-smoothing: auto;
	}

	.tree-node-wrapper :global(.tree-node-container) {
		pointer-events: auto;
	}

	.tree-node-wrapper :global(.tree-node) {
		width: max-content;
		max-width: 500px;
		min-width: 300px;
	}

	/* Orphaned blocks */
	.orphaned-block {
		pointer-events: auto;
		cursor: pointer;
		/* Better text rendering */
		-webkit-font-smoothing: subpixel-antialiased;
		-moz-osx-font-smoothing: auto;
		text-rendering: geometricPrecision;
	}

	.orphaned-block-content {
		width: 100%;
		height: 100%;
		background: #161b22;
		border: 2px dashed #f9e2af;
		border-radius: 8px;
		padding: 12px;
		display: flex;
		flex-direction: column;
		gap: 8px;
		transition: all 0.2s;
	}

	.orphaned-block-content:hover {
		background: #21262d;
		border-color: #f9e2af;
		box-shadow: 0 4px 12px rgba(249, 226, 175, 0.3);
	}

	.orphaned-block-content.selected {
		background: #30363d;
		border-color: #89b4fa;
		border-style: solid;
		box-shadow: 0 4px 16px rgba(137, 180, 250, 0.4);
	}

	.orphaned-block-header {
		display: flex;
		align-items: center;
		gap: 8px;
		font-weight: 600;
		color: #c9d1d9;
	}

	.orphaned-icon {
		font-size: 16px;
	}

	.orphaned-label {
		font-size: 13px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.orphaned-hint {
		font-size: 11px;
		color: #f9e2af;
		font-style: italic;
	}
</style>
