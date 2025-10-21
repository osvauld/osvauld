<script lang="ts">
	import Block from "./Block.svelte";

	interface Props {
		blocks: Map<string, any>;
		viewport: { x: number; y: number; zoom: number };
		selectedBlockId: string | null;
		readonly?: boolean;
		onViewportChange: (viewport: { x?: number; y?: number; zoom?: number }) => void;
		onBlockUpdate: (blockId: string, updates: any) => void;
		onBlockSelect: (blockId: string) => void;
		onBlockDrop?: (blockType: string, x: number, y: number) => void;
	}

	let { blocks, viewport, selectedBlockId, readonly = false, onViewportChange, onBlockUpdate, onBlockSelect, onBlockDrop }: Props = $props();

	// Local state for branching form answers (not synced to Yjs)
	let formAnswers = $state<Map<string, string>>(new Map());

	// Viewport animation state
	let isAnimating = $state(false);

	// Form submission handler
	function handleFormSubmit(submitButtonId: string) {
		console.log("🚀 Form submit triggered by button:", submitButtonId);

		// Get the submit button block
		const submitButton = blocks.get(submitButtonId);
		if (!submitButton) {
			console.error("❌ Submit button not found:", submitButtonId);
			return;
		}

		// Get the formId from the submit button
		const formId = submitButton.formId;
		if (!formId) {
			console.warn("⚠️ Submit button has no formId assigned");
			return;
		}

		// Get the form metadata block
		const formBlock = blocks.get(formId);
		if (!formBlock) {
			console.error("❌ Form metadata block not found:", formId);
			return;
		}

		const formName = formBlock.name || formId;
		const eventName = formBlock.eventName;

		if (!eventName) {
			console.error("❌ Form has no eventName configured");
			return;
		}

		console.log(`📋 Submitting form: "${formName}" (ID: ${formId})`);
		console.log(`🏷️  Event name: "${eventName}"`);

		// Find all form field blocks with matching formId
		const formData: Record<string, any> = {};
		let fieldCount = 0;

		for (const [id, block] of blocks.entries()) {
			if (block.type && block.type.startsWith('form-field-') && block.formId === formId) {
				// Get the field value from the DOM
				const fieldName = block.fieldName || block.label || block.id;
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

		// Prepare the submission payload
		const submissionPayload = {
			eventName: eventName,
			formId: formId,
			formName: formName,
			data: formData,
			timestamp: Date.now()
		};

		console.log(`✅ Form submission complete! Collected ${fieldCount} fields from "${formName}":`, submissionPayload);

		// TODO: Emit this data to the parent or send to backend
		// emit("form-submission", submissionPayload);
	}

	// Navigation button handler with viewport animation
	function handleNavigation(navButtonId: string) {
		console.log("🧭 Navigation triggered by button:", navButtonId);

		const navButton = blocks.get(navButtonId);
		if (!navButton) {
			console.error("❌ Nav button not found:", navButtonId);
			return;
		}

		// Get the question this button is linked to
		const questionId = navButton.questionId;
		if (!questionId) {
			console.error("❌ No questionId configured for nav button");
			return;
		}

		// Read the answer from the DOM (since Block.svelte manages local state)
		const questionElement = document.querySelector(`[data-question-id="${questionId}"]`);
		if (!questionElement) {
			console.error("❌ Question not found:", questionId);
			return;
		}

		// Get selected answer from sibling button
		const selectedButton = questionElement.parentElement?.querySelector('.option-button.selected');
		if (!selectedButton) {
			console.warn("⚠️ No answer selected yet");
			return;
		}

		const answer = selectedButton.getAttribute('data-answer');
		console.log("📋 User answered:", answer);

		// Store answer locally
		formAnswers.set(questionId, answer!);

		// Get target block ID based on answer
		const targetBlockId = answer === 'yes' ? navButton.yesTargetId : navButton.noTargetId;

		if (!targetBlockId) {
			console.error("❌ No target block configured for answer:", answer);
			return;
		}

		// Look up the CURRENT position of the target block
		const targetBlock = blocks.get(targetBlockId);
		if (!targetBlock) {
			console.error("❌ Target block not found:", targetBlockId);
			return;
		}

		// Calculate viewport position to center the target block
		// Assuming ~1200px wide and ~800px tall viewport
		const viewportX = -(targetBlock.x - 600);
		const viewportY = -(targetBlock.y - 400);

		console.log("🎯 Navigating to block at:", { x: targetBlock.x, y: targetBlock.y });
		console.log("📍 Viewport position:", { x: viewportX, y: viewportY });

		// Animate viewport to target block's CURRENT position
		animateViewport(viewportX, viewportY, viewport.zoom);
	}

	// Smooth viewport animation
	function animateViewport(targetX: number, targetY: number, targetZoom: number = 1) {
		if (isAnimating) return;

		isAnimating = true;
		const duration = 500; // milliseconds
		const startTime = performance.now();
		const startX = localViewport.x;
		const startY = localViewport.y;
		const startZoom = viewport.zoom;

		function easeInOutCubic(t: number): number {
			return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
		}

		function animate(currentTime: number) {
			const elapsed = currentTime - startTime;
			const progress = Math.min(elapsed / duration, 1);
			const eased = easeInOutCubic(progress);

			const currentX = startX + (targetX - startX) * eased;
			const currentY = startY + (targetY - startY) * eased;
			const currentZoom = startZoom + (targetZoom - startZoom) * eased;

			// Update local viewport for smooth animation
			localViewport = { x: currentX, y: currentY };

			if (progress < 1) {
				requestAnimationFrame(animate);
			} else {
				// Final update to sync with parent when done
				onViewportChange({ x: currentX, y: currentY });
				isAnimating = false;
				console.log("✅ Navigation complete");
			}
		}

		requestAnimationFrame(animate);
	}

	let canvasContainer = $state<HTMLDivElement>();
	let canvasElement = $state<HTMLDivElement>();
	let isPanning = $state(false);
	let panStart = $state({ x: 0, y: 0 });
	let viewportUpdateTimeout: number | null = null;
	let zoomUpdateTimeout: number | null = null;

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

	function handleMouseUp() {
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
		{#each blocksArray as block (block.id)}
			<Block
				{block}
				{blocks}
				isSelected={selectedBlockId === block.id}
				{readonly}
				onUpdate={onBlockUpdate}
				onSelect={onBlockSelect}
				onFormSubmit={handleFormSubmit}
				onNavigate={handleNavigation}
			/>
		{/each}
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
</div>

<style>
	.canvas-container {
		width: 100%;
		height: 100%;
		overflow: hidden;
		position: relative;
		background: #0d0e13;
		background-image: radial-gradient(circle, #21262d 1px, transparent 1px);
		background-size: 20px 20px;
	}

	.canvas {
		position: relative;
		width: 100%;
		height: 100%;
		transform-origin: 0 0;
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
</style>
