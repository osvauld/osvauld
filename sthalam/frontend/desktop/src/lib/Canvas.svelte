<script lang="ts">
	import Block from "./Block.svelte";
	import TreeNode from "./TreeNode.svelte";
	import TreeGuideLines from "./TreeGuideLines.svelte";
	import ConnectionLines from "./ConnectionLines.svelte";
	import { layoutHierarchical, type LayoutConfig } from "./layoutEngine";
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
	let zoomUpdateTimeout: number | null = null;

	function handleConnectionSelectInternal(blockId: string, connectionId: string) {
		if (onConnectionSelect) {
			onConnectionSelect(blockId, connectionId);
		} else {
			// Fallback: just select the block
			onBlockSelect(blockId);
		}
	}

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
		// A point at canvas coordinates (cx, cy) appears on screen at:
		//   screenX = (cx * zoom) + viewport.x
		//   screenY = (cy * zoom) + viewport.y
		// So to reverse:
		//   cx = (screenX - viewport.x) / zoom
		//   cy = (screenY - viewport.y) / zoom

		const canvasX = (relativeX - viewport.x) / viewport.zoom;
		const canvasY = (relativeY - viewport.y) / viewport.zoom;

		// Call the drop handler
		onBlockDrop(blockType, canvasX, canvasY);
	}

	// Fit all blocks in view
	function fitToContent() {
		if (blocksArray.length === 0) return;

		// Find bounding box of all blocks
		let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;

		for (const block of blocksArray) {
			minX = Math.min(minX, block.x);
			minY = Math.min(minY, block.y);
			maxX = Math.max(maxX, block.x + (block.width || 300));
			maxY = Math.max(maxY, block.y + (block.height || 100));
		}

		// Add padding
		const padding = 100;
		minX -= padding;
		minY -= padding;
		maxX += padding;
		maxY += padding;

		// Calculate required zoom to fit content
		const contentWidth = maxX - minX;
		const contentHeight = maxY - minY;
		const containerWidth = canvasContainer?.clientWidth || 800;
		const containerHeight = canvasContainer?.clientHeight || 600;

		const zoomX = containerWidth / contentWidth;
		const zoomY = containerHeight / contentHeight;
		const zoom = Math.min(zoomX, zoomY, 1); // Don't zoom in beyond 100%

		// Calculate viewport offset to center content
		const x = -(minX * zoom);
		const y = -(minY * zoom);

		onViewportChange({ x, y, zoom });
	}

	// View mode: 'blocks' (old) or 'tree' (new POC)
	let viewMode = $state<'blocks' | 'tree'>('tree');

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

	// Calculate tree layouts when in tree mode
	let treeLayouts = $derived.by(() => {
		if (viewMode !== 'tree') return new Map();

		const screens = getScreenBlocks(blocks);
		return layoutAllScreens(screens, blocks, collapsedBlocks, DEFAULT_TREE_CONFIG);
	});

	// Flatten tree layouts for rendering
	let flatTreeNodes = $derived.by(() => {
		if (viewMode !== 'tree') return [];

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

	// Auto-layout all blocks
	let layoutMode = $state<'spacious' | 'compact'>('spacious');

	function autoLayout() {
		if (blocks.size === 0) return;

		const config: Partial<LayoutConfig> = {
			mode: layoutMode
		};

		const positions = layoutHierarchical(blocks, config);

		// Apply positions only - let containers size themselves based on CSS and content
		for (const result of positions) {
			const block = blocks.get(result.blockId);
			const updates: any = { x: result.x, y: result.y };

			// For containers, set width but NOT height - let them grow with content
			if (block?.type === 'screen-container' || block?.type === 'section-container') {
				if ((result as any).width) {
					updates.width = (result as any).width;
				}
				// Don't set height - let CSS and content determine it
			} else {
				// For non-containers, set both dimensions if calculated
				if ((result as any).width) {
					updates.width = (result as any).width;
				}
				if ((result as any).height) {
					updates.height = (result as any).height;
				}
			}

			onBlockUpdate(result.blockId, updates);
		}

		// After layout, fit to view
		setTimeout(() => fitToContent(), 100);
	}

	function toggleLayoutMode() {
		layoutMode = layoutMode === 'spacious' ? 'compact' : 'spacious';
	}

	// Convert blocks Map to array - only render TOP-LEVEL blocks (no parentId)
	// Children are rendered by their parent container's ContainerPreview
	$effect(() => {
		blocksArray = Array.from(blocks.values()).filter(block => !block.parentId);
	});

	let blocksArray = $state<any[]>([]);
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
		<!-- Connection lines layer (only for navigation, not parent-child) -->
		{#if viewMode === 'blocks'}
			<ConnectionLines {blocks} {viewport} {selectedBlockId} onConnectionSelect={handleConnectionSelectInternal} />
		{/if}

		<!-- Canvas rendering: Tree view or Blocks view -->
		{#if viewMode === 'tree'}
			<!-- Tree View: Render tree nodes in a container -->
			<div class="tree-container">
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
			</div>
		{:else}
			<!-- Blocks View: Original rendering -->
			{#each blocksArray as block (block.id)}
				<Block
					{block}
					{blocks}
					isSelected={selectedBlockId === block.id}
					{selectedBlockId}
					readonly={false}
					onUpdate={onBlockUpdate}
					onSelect={onBlockSelect}
				/>
			{/each}
		{/if}
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
			class="reset-view-btn"
			onclick={fitToContent}
			title="Fit All Blocks in View"
		>
			⊡
		</button>
		<button
			class="zoom-btn"
			onclick={() => onViewportChange({ zoom: 1 })}
			title="Reset Zoom"
		>
			⊙
		</button>
	</div>

	<!-- Layout controls -->
	<div class="layout-controls">
		<button
			class="layout-btn view-toggle"
			onclick={() => (viewMode = viewMode === 'tree' ? 'blocks' : 'tree')}
			title="Toggle between Tree View and Blocks View"
		>
			{viewMode === 'tree' ? '📁 Tree' : '📦 Blocks'}
		</button>
		{#if viewMode === 'blocks'}
			<button
				class="layout-btn auto-layout"
				onclick={autoLayout}
				title="Auto-arrange blocks in hierarchical layout"
			>
				Auto Layout
			</button>
			<button
				class="layout-btn mode-toggle"
				onclick={toggleLayoutMode}
				title="Toggle between compact and spacious layout"
			>
				{layoutMode === 'spacious' ? '⊟' : '⊞'}
			</button>
		{/if}
	</div>
</div>

<style>
	.canvas-container {
		width: 100%;
		height: 100%;
		overflow: hidden;
		position: relative;
		background: #0d0e13;
		/* No grid background */
		user-select: none;
		-webkit-user-select: none;
		-moz-user-select: none;
		-ms-user-select: none;
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
		/* Optimize for Tauri WebView */
		-webkit-font-smoothing: antialiased;
		image-rendering: -webkit-optimize-contrast;
		image-rendering: crisp-edges;
		transform-style: preserve-3d;
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

	.reset-view-btn {
		width: 32px;
		height: 32px;
		border: 1px solid #30363d;
		border-radius: 6px;
		background: #0d0e13;
		color: #89b4fa;
		font-size: 18px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all 0.2s;
		padding: 0;
		margin-left: 4px;
	}

	.reset-view-btn:hover {
		background: #89b4fa;
		color: #010409;
		border-color: #89b4fa;
		transform: translateY(-1px);
		box-shadow: 0 2px 8px rgba(137, 180, 250, 0.3);
	}

	.reset-view-btn:active {
		transform: translateY(0);
	}

	/* Layout controls */
	.layout-controls {
		position: absolute;
		bottom: 20px;
		left: 20px;
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

	.layout-btn {
		height: 32px;
		padding: 0 12px;
		border: 1px solid #30363d;
		border-radius: 6px;
		background: #0d0e13;
		color: #c9d1d9;
		font-size: 13px;
		font-weight: 600;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all 0.2s;
		white-space: nowrap;
	}

	.layout-btn:hover {
		background: #a6e3a1;
		color: #010409;
		border-color: #a6e3a1;
		transform: translateY(-1px);
		box-shadow: 0 2px 8px rgba(166, 227, 161, 0.3);
	}

	.layout-btn:active {
		transform: translateY(0);
	}

	.layout-btn.mode-toggle {
		width: 32px;
		padding: 0;
		font-size: 16px;
	}

	.layout-btn.auto-layout {
		font-weight: 600;
	}

	.layout-btn.view-toggle {
		font-weight: 700;
		background: #667eea;
		color: white;
		border-color: #667eea;
	}

	.layout-btn.view-toggle:hover {
		background: #89b4fa;
		border-color: #89b4fa;
	}

	.tree-container {
		position: relative;
		min-height: 100%;
		min-width: 100%;
		pointer-events: none;
	}

	.tree-node-wrapper {
		pointer-events: none;
		width: fit-content;
		max-width: 500px;
	}

	.tree-node-wrapper :global(.tree-node-container) {
		pointer-events: auto;
	}

	.tree-node-wrapper :global(.tree-node) {
		width: max-content;
		max-width: 500px;
		min-width: 300px;
	}
</style>
