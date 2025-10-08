<script lang="ts">
	import Block from "./Block.svelte";

	interface Props {
		blocks: Map<string, any>;
		viewport: { x: number; y: number; zoom: number };
		selectedBlockId: string | null;
		onViewportChange: (viewport: { x: number; y: number }) => void;
		onBlockUpdate: (blockId: string, updates: any) => void;
		onBlockSelect: (blockId: string) => void;
	}

	let { blocks, viewport, selectedBlockId, onViewportChange, onBlockUpdate, onBlockSelect }: Props = $props();

	let canvasContainer = $state<HTMLDivElement>();
	let isPanning = $state(false);
	let panStart = $state({ x: 0, y: 0 });

	function handleMouseDown(e: MouseEvent) {
		// Only pan if clicking on the canvas background (not on a block)
		if (e.target === canvasContainer) {
			isPanning = true;
			panStart = {
				x: e.clientX - viewport.x,
				y: e.clientY - viewport.y,
			};
		}
	}

	function handleMouseMove(e: MouseEvent) {
		if (isPanning) {
			const newX = e.clientX - panStart.x;
			const newY = e.clientY - panStart.y;
			onViewportChange({ x: newX, y: newY });
		}
	}

	function handleMouseUp() {
		isPanning = false;
	}

	// Convert blocks Map to array for iteration
	$effect(() => {
		blocksArray = Array.from(blocks.values());
	});

	let blocksArray = $state<any[]>([]);
</script>

<svelte:window onmousemove={handleMouseMove} onmouseup={handleMouseUp} />

<div
	class="canvas-container"
	bind:this={canvasContainer}
	onmousedown={handleMouseDown}
	style:cursor={isPanning ? "grabbing" : "grab"}
>
	<div
		class="canvas"
		style:transform="translate({viewport.x}px, {viewport.y}px) scale({viewport.zoom})"
	>
		{#each blocksArray as block (block.id)}
			<Block
				{block}
				isSelected={selectedBlockId === block.id}
				onUpdate={onBlockUpdate}
				onSelect={onBlockSelect}
			/>
		{/each}
	</div>
</div>

<style>
	.canvas-container {
		width: 100%;
		height: 100%;
		overflow: hidden;
		position: relative;
		background: #ffffff;
		background-image: radial-gradient(circle, #e0e0e0 1px, transparent 1px);
		background-size: 20px 20px;
	}

	.canvas {
		position: relative;
		width: 100%;
		height: 100%;
		transform-origin: 0 0;
	}
</style>
