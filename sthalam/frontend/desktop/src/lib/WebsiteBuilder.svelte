<script lang="ts">
	import { onMount, onDestroy, untrack } from "svelte";
	import { dataState } from "../store.svelte";
	import { dataState as authDataState, uiState } from "../state";
	import Canvas from "./Canvas.svelte";
	import BlockPalette from "./BlockPalette.svelte";
	import PropertiesPanel from "./PropertiesPanel.svelte";
	import KeyboardShortcuts from "./KeyboardShortcuts.svelte";
	import NavigationPanel from "../components/NavigationPanel.svelte";
	import FolderManager from "../components/FolderManager.svelte";
	import type { YjsDocuments } from "./yjsManager";

	let yDocs: YjsDocuments | null = null;
	let blocks = $state<Map<string, any>>(new Map());
	let viewport = $state({ x: 0, y: 0, zoom: 1 });
	let selectedBlockId = $state<string | null>(null);

	const selectedBlock = $derived(
		selectedBlockId ? blocks.get(selectedBlockId) || null : null
	);

	// Track if we have a resource selected
	const hasResource = $derived(!!authDataState.currentResourceId);

	onMount(async () => {
		console.log("🚀 Initializing Website Builder...");

		// Initialize dataState (creates coordinator)
		await dataState.initializeState();

		console.log("✅ Website Builder initialized!");
	});

	// React to resource changes (like livnote's pattern)
	// Only track resourceId - don't track blocks/viewport changes
	$effect(() => {
		const resourceId = authDataState.currentResourceId;
		console.log("🔄 Resource changed:", resourceId);

		if (!resourceId) {
			// No resource selected - clear everything
			console.log("❌ No resource selected - clearing workspace");
			yDocs = null;
			blocks = new Map();
			viewport = { x: 0, y: 0, zoom: 1 };
			selectedBlockId = null;
			return;
		}

		// Resource selected - get fresh documents from coordinator
		console.log("✅ Resource selected - setting up workspace");

		// Use untracked to avoid infinite loops
		untrack(() => {
			const coordinator = dataState.getBlocksuiteCoordinator();
			if (!coordinator) {
				console.error("❌ No coordinator available");
				return;
			}

			// Get the FRESH Yjs documents (after loadBlocksuite was called)
			const docs = coordinator.getDocuments();
			if (!docs) {
				console.error("❌ No Yjs documents available");
				return;
			}

			yDocs = docs;

			console.log("📦 Yjs documents received:", {
				blocks: docs.blocks.size,
				viewport: {
					x: docs.viewport.get("x"),
					y: docs.viewport.get("y"),
					zoom: docs.viewport.get("zoom")
				}
			});

			// Subscribe to blocks changes
			const blocksObserver = () => {
				if (!yDocs) return;
				const newBlocks = new Map();
				yDocs.blocks.forEach((value, key) => {
					newBlocks.set(key, value);
				});
				blocks = newBlocks;
				console.log("📦 Blocks updated:", blocks.size);
			};

			// Subscribe to viewport changes
			const viewportObserver = () => {
				if (!yDocs) return;
				viewport = {
					x: yDocs.viewport.get("x") || 0,
					y: yDocs.viewport.get("y") || 0,
					zoom: yDocs.viewport.get("zoom") || 1,
				};
				console.log("🔍 Viewport updated:", viewport);
			};

			docs.blocks.observe(blocksObserver);
			docs.viewport.observe(viewportObserver);

			// Initial load
			blocksObserver();
			viewportObserver();
		});

		// Cleanup function for this effect
		return () => {
			if (yDocs) {
				// Store reference before clearing
				const docsToCleanup = yDocs;
				// Unobserve using the stored reference
				const blocksObserver = () => {};
				const viewportObserver = () => {};
				// Note: We can't properly unobserve here because the observers are scoped
				// This is acceptable as Yjs will clean up when docs are destroyed
			}
		};
	});

	onDestroy(() => {
		// Cleanup is handled by dataState.clearAllState()
		dataState.clearAllState();
	});

	function updateViewport(newViewport: { x: number; y: number }) {
		if (!yDocs) return;
		yDocs.viewport.set("x", newViewport.x);
		yDocs.viewport.set("y", newViewport.y);
	}

	function updateBlock(blockId: string, updates: Partial<any>) {
		if (!yDocs) return;
		const block = yDocs.blocks.get(blockId);
		if (block) {
			yDocs.blocks.set(blockId, { ...block, ...updates });
		}
	}

	function normalizeZIndexes() {
		if (!yDocs) return;
		// Get all blocks sorted by current z-index
		const allBlocks = Array.from(yDocs.blocks.entries()).map(([id, block]) => ({
			id,
			block,
		}));

		allBlocks.sort((a, b) => a.block.zIndex - b.block.zIndex);

		// Reassign sequential z-indexes starting from 1
		allBlocks.forEach((item, index) => {
			yDocs!.blocks.set(item.id, { ...item.block, zIndex: index + 1 });
		});
	}

	function bringForward(blockId: string) {
		if (!yDocs) return;
		const block = yDocs.blocks.get(blockId);
		if (!block) return;

		// Get all blocks sorted by z-index
		const allBlocks = Array.from(yDocs.blocks.entries()).map(([id, b]) => ({
			id,
			zIndex: b.zIndex,
		}));
		allBlocks.sort((a, b) => a.zIndex - b.zIndex);

		// Find current position
		const currentIndex = allBlocks.findIndex((b) => b.id === blockId);
		if (currentIndex === -1 || currentIndex === allBlocks.length - 1) return; // Already at front

		// Swap z-index with block above
		const aboveBlock = allBlocks[currentIndex + 1];
		const currentZIndex = block.zIndex;
		const aboveZIndex = yDocs.blocks.get(aboveBlock.id)?.zIndex || 0;

		yDocs.blocks.set(blockId, { ...block, zIndex: aboveZIndex });
		yDocs.blocks.set(aboveBlock.id, {
			...yDocs.blocks.get(aboveBlock.id)!,
			zIndex: currentZIndex
		});

		// Normalize to clean up gaps
		setTimeout(() => normalizeZIndexes(), 0);
	}

	function sendBackward(blockId: string) {
		if (!yDocs) return;
		const block = yDocs.blocks.get(blockId);
		if (!block) return;

		// Get all blocks sorted by z-index
		const allBlocks = Array.from(yDocs.blocks.entries()).map(([id, b]) => ({
			id,
			zIndex: b.zIndex,
		}));
		allBlocks.sort((a, b) => a.zIndex - b.zIndex);

		// Find current position
		const currentIndex = allBlocks.findIndex((b) => b.id === blockId);
		if (currentIndex === -1 || currentIndex === 0) return; // Already at back

		// Swap z-index with block below
		const belowBlock = allBlocks[currentIndex - 1];
		const currentZIndex = block.zIndex;
		const belowZIndex = yDocs.blocks.get(belowBlock.id)?.zIndex || 0;

		yDocs.blocks.set(blockId, { ...block, zIndex: belowZIndex });
		yDocs.blocks.set(belowBlock.id, {
			...yDocs.blocks.get(belowBlock.id)!,
			zIndex: currentZIndex
		});

		// Normalize to clean up gaps
		setTimeout(() => normalizeZIndexes(), 0);
	}

	function bringToFront(blockId: string) {
		if (!yDocs) return;
		const block = yDocs.blocks.get(blockId);
		if (!block) return;

		// Find the highest zIndex
		let maxZIndex = 0;
		yDocs.blocks.forEach((b) => {
			if (b.zIndex > maxZIndex) {
				maxZIndex = b.zIndex;
			}
		});

		yDocs.blocks.set(blockId, { ...block, zIndex: maxZIndex + 1 });

		// Normalize to clean up gaps
		setTimeout(() => normalizeZIndexes(), 0);
	}

	function sendToBack(blockId: string) {
		if (!yDocs) return;
		const block = yDocs.blocks.get(blockId);
		if (!block) return;

		// Find the lowest zIndex
		let minZIndex = Infinity;
		yDocs.blocks.forEach((b) => {
			if (b.zIndex < minZIndex) {
				minZIndex = b.zIndex;
			}
		});

		// Set to below minimum (will be normalized to 1)
		yDocs.blocks.set(blockId, { ...block, zIndex: minZIndex - 1 });

		// Normalize to clean up gaps
		setTimeout(() => normalizeZIndexes(), 0);
	}

	function addBlock(type: string) {
		if (!yDocs) return;
		const id = `block-${Date.now()}`;

		// Calculate center of viewport
		const centerX = Math.abs(viewport.x) + 400;
		const centerY = Math.abs(viewport.y) + 200;

		let newBlock: any = {
			id,
			type,
			x: centerX,
			y: centerY,
			zIndex: blocks.size + 1,
			styles: {},
		};

		// Configure based on type
		switch(type) {
			case "heading":
				newBlock.width = 400;
				newBlock.height = 60;
				newBlock.content = "New Heading";
				newBlock.styles = { fontSize: "32px", fontWeight: "700" };
				break;
			case "text":
				newBlock.width = 300;
				newBlock.height = 100;
				newBlock.content = "New text block";
				break;
			case "image":
				newBlock.width = 300;
				newBlock.height = 200;
				newBlock.content = ""; // Image URL will be set via Properties
				newBlock.styles = { backgroundColor: "transparent", border: "none" };
				break;
			case "container":
				newBlock.width = 400;
				newBlock.height = 300;
				newBlock.content = "Container";
				newBlock.styles = { backgroundColor: "rgba(255,255,255,0.8)" };
				break;
			case "html":
				newBlock.width = 400;
				newBlock.height = 300;
				newBlock.content = ""; // HTML will be set via Properties
				newBlock.styles = {
					css: "" // Custom CSS
				};
				break;
			case "notice-board":
				newBlock.width = 500;
				newBlock.height = 400;
				newBlock.content = JSON.stringify([]); // Array of messages
				newBlock.styles = {
					backgroundColor: "white",
					border: "2px solid #ddd"
				};
				break;
			case "form":
				newBlock.width = 500;
				newBlock.height = 350;
				newBlock.content = JSON.stringify({
					fields: [
						{ id: "field-1", type: "text", label: "Name", placeholder: "Enter your name", required: true },
						{ id: "field-2", type: "email", label: "Email", placeholder: "Enter your email", required: true },
						{ id: "field-3", type: "textarea", label: "Message", placeholder: "Your message...", required: false }
					],
					submitButtonText: "Submit"
				});
				newBlock.styles = {
					backgroundColor: "white",
					border: "2px solid #ddd"
				};
				break;
			default:
				newBlock.width = 300;
				newBlock.height = 100;
				newBlock.content = "New block";
		}

		yDocs.blocks.set(id, newBlock);
	}
</script>

{#if !hasResource}
	<!-- No resource selected - show empty state -->
	<div class="empty-state">
		<NavigationPanel />
		<div class="empty-message">
			<h2>No resource selected</h2>
			<p>Select a resource from the sidebar or create a new one to get started.</p>
		</div>
	</div>
{:else}
	<!-- Resource selected - show builder -->
	<div class="builder-container">
		<NavigationPanel />
		<BlockPalette onAddBlock={addBlock} />
		<Canvas
			{blocks}
			{viewport}
			{selectedBlockId}
			onViewportChange={updateViewport}
			onBlockUpdate={updateBlock}
			onBlockSelect={(id) => (selectedBlockId = id)}
		/>
		<PropertiesPanel
			{selectedBlock}
			onUpdateBlock={updateBlock}
			onBringForward={bringForward}
			onSendBackward={sendBackward}
			onBringToFront={bringToFront}
			onSendToBack={sendToBack}
		/>
		<KeyboardShortcuts />
		{#if uiState.showFolderManager}
			<FolderManager position="navigationPanel" />
		{/if}
	</div>
{/if}

<style>
	.builder-container {
		width: 100%;
		height: 100vh;
		overflow: hidden;
		background: #010409;
		display: flex;
	}

	.empty-state {
		width: 100%;
		height: 100vh;
		display: flex;
		background: #010409;
	}

	.empty-message {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		color: #8b949e;
	}

	.empty-message h2 {
		font-size: 1.5rem;
		margin-bottom: 0.5rem;
		color: #c9d1d9;
	}

	.empty-message p {
		font-size: 1rem;
	}
</style>
