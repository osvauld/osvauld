<script lang="ts">
	import Block from "./Block.svelte";

	interface Props {
		blocks: Map<string, any>;
		viewport: { x: number; y: number; zoom: number };
		selectedBlockId: string | null;
		readonly?: boolean;
		onViewportChange: (viewport: { x: number; y: number }) => void;
		onBlockUpdate: (blockId: string, updates: any) => void;
		onBlockSelect: (blockId: string) => void;
	}

	let { blocks, viewport, selectedBlockId, readonly = false, onViewportChange, onBlockUpdate, onBlockSelect }: Props = $props();

	// Form submission handler
	function handleFormSubmit(submitButtonId: string) {
		console.log("🚀 Form submit triggered by button:", submitButtonId);

		// Get the submit button block
		const submitButton = blocks.get(submitButtonId);
		if (!submitButton) {
			console.error("❌ Submit button not found:", submitButtonId);
			return;
		}

		// Find the form container this button belongs to (spatially)
		let formContainer = null;
		for (const [id, block] of blocks.entries()) {
			if (block.type === 'form-container') {
				// Check if submit button is spatially within this container
				const isInside =
					submitButton.x >= block.x &&
					submitButton.x + submitButton.width <= block.x + block.width &&
					submitButton.y >= block.y &&
					submitButton.y + submitButton.height <= block.y + block.height;

				if (isInside) {
					formContainer = block;
					console.log("📋 Found form container:", id);
					break;
				}
			}
		}

		if (!formContainer) {
			console.warn("⚠️ No form container found for submit button");
			return;
		}

		// Find all form field blocks within this container
		const formData: Record<string, any> = {};
		let fieldCount = 0;

		for (const [id, block] of blocks.entries()) {
			if (block.type && block.type.startsWith('form-field-')) {
				// Check if this field is spatially within the form container
				const isInside =
					block.x >= formContainer.x &&
					block.x + block.width <= formContainer.x + formContainer.width &&
					block.y >= formContainer.y &&
					block.y + block.height <= formContainer.y + formContainer.height;

				if (isInside) {
					// Get the field value from the DOM
					const fieldName = block.fieldName || block.id;
					const inputElement = document.querySelector(`[data-field-id="${id}"]`) as HTMLInputElement;

					if (inputElement) {
						if (block.type === 'form-field-checkbox') {
							formData[fieldName] = inputElement.checked;
						} else {
							formData[fieldName] = inputElement.value;
						}
						fieldCount++;
						console.log(`  ✓ Field "${fieldName}":`, formData[fieldName]);
					}
				}
			}
		}

		console.log(`✅ Form submission complete! Collected ${fieldCount} fields:`, formData);

		// TODO: Emit this data to the parent or send to backend
		// For now, just log it
	}

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
				{readonly}
				onUpdate={onBlockUpdate}
				onSelect={onBlockSelect}
				onFormSubmit={handleFormSubmit}
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
		background: var(--bg-secondary, #f5f5f5);
		background-image: radial-gradient(circle, var(--border-color, #e0e0e0) 1px, transparent 1px);
		background-size: 20px 20px;
	}

	.canvas {
		position: relative;
		width: 100%;
		height: 100%;
		transform-origin: 0 0;
	}
</style>
