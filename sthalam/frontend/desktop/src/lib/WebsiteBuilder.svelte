<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import * as Y from "yjs";
	import { editorStore } from "../store.svelte";
	import Canvas from "./Canvas.svelte";
	import BlockPalette from "./BlockPalette.svelte";
	import PropertiesPanel from "./PropertiesPanel.svelte";
	import KeyboardShortcuts from "./KeyboardShortcuts.svelte";

	let doc: Y.Doc;
	let yBlocks: Y.Map<any>;
	let yViewport: Y.Map<any>;
	let blocks = $state<Map<string, any>>(new Map());
	let viewport = $state({ x: 0, y: 0, zoom: 1 });
	let selectedBlockId = $state<string | null>(null);

	const selectedBlock = $derived(
		selectedBlockId ? blocks.get(selectedBlockId) || null : null
	);

	onMount(() => {
		console.log("🚀 Initializing Website Builder with Yjs...");

		// Create Yjs document
		doc = new Y.Doc();

		// Create Y.Map for blocks
		yBlocks = doc.getMap("blocks");

		// Create Y.Map for viewport
		yViewport = doc.getMap("viewport");

		// Initialize viewport if empty
		if (yViewport.size === 0) {
			yViewport.set("x", 0);
			yViewport.set("y", 0);
			yViewport.set("zoom", 1);
		}

		// Add some initial blocks for testing
		if (yBlocks.size === 0) {
			const block1 = {
				id: "block-1",
				type: "heading",
				x: 100,
				y: 100,
				width: 400,
				height: 60,
				zIndex: 1,
				content: "Welcome to Website Builder",
				styles: {
					fontSize: "32px",
					fontWeight: "700",
					color: "#333",
				},
			};

			const block2 = {
				id: "block-2",
				type: "text",
				x: 100,
				y: 200,
				width: 400,
				height: 100,
				zIndex: 1,
				content: "Click and drag blocks to move them around the canvas.",
				styles: {
					fontSize: "16px",
					color: "#666",
				},
			};

			yBlocks.set(block1.id, block1);
			yBlocks.set(block2.id, block2);
		}

		// Subscribe to blocks changes
		yBlocks.observe(() => {
			const newBlocks = new Map();
			yBlocks.forEach((value, key) => {
				newBlocks.set(key, value);
			});
			blocks = newBlocks;
			console.log("📦 Blocks updated:", blocks.size);
		});

		// Subscribe to viewport changes
		yViewport.observe(() => {
			viewport = {
				x: yViewport.get("x") || 0,
				y: yViewport.get("y") || 0,
				zoom: yViewport.get("zoom") || 1,
			};
			console.log("🔍 Viewport updated:", viewport);
		});

		// Initial load
		const newBlocks = new Map();
		yBlocks.forEach((value, key) => {
			newBlocks.set(key, value);
		});
		blocks = newBlocks;

		viewport = {
			x: yViewport.get("x") || 0,
			y: yViewport.get("y") || 0,
			zoom: yViewport.get("zoom") || 1,
		};

		// Update store
		editorStore.setDoc(doc);

		// For debugging
		if (typeof window !== "undefined") {
			(window as any).doc = doc;
			(window as any).yBlocks = yBlocks;
			(window as any).yViewport = yViewport;
			(window as any).addBlock = (
				type: string,
				x: number,
				y: number,
			) => {
				const id = `block-${Date.now()}`;
				const newBlock = {
					id,
					type,
					x,
					y,
					width: 300,
					height: 100,
					zIndex: blocks.size + 1,
					content: "New block",
					styles: {},
				};
				yBlocks.set(id, newBlock);
				return id;
			};
			(window as any).bringForward = bringForward;
			(window as any).sendBackward = sendBackward;
			(window as any).bringToFront = bringToFront;
			(window as any).sendToBack = sendToBack;
		}

		console.log("✅ Website Builder initialized!");
	});

	onDestroy(() => {
		if (doc) {
			doc.destroy();
		}
	});

	function updateViewport(newViewport: { x: number; y: number }) {
		yViewport.set("x", newViewport.x);
		yViewport.set("y", newViewport.y);
	}

	function updateBlock(blockId: string, updates: Partial<any>) {
		const block = yBlocks.get(blockId);
		if (block) {
			yBlocks.set(blockId, { ...block, ...updates });
		}
	}

	function normalizeZIndexes() {
		// Get all blocks sorted by current z-index
		const allBlocks = Array.from(yBlocks.entries()).map(([id, block]) => ({
			id,
			block,
		}));

		allBlocks.sort((a, b) => a.block.zIndex - b.block.zIndex);

		// Reassign sequential z-indexes starting from 1
		allBlocks.forEach((item, index) => {
			yBlocks.set(item.id, { ...item.block, zIndex: index + 1 });
		});
	}

	function bringForward(blockId: string) {
		const block = yBlocks.get(blockId);
		if (!block) return;

		// Get all blocks sorted by z-index
		const allBlocks = Array.from(yBlocks.entries()).map(([id, b]) => ({
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
		const aboveZIndex = yBlocks.get(aboveBlock.id)?.zIndex || 0;

		yBlocks.set(blockId, { ...block, zIndex: aboveZIndex });
		yBlocks.set(aboveBlock.id, {
			...yBlocks.get(aboveBlock.id)!,
			zIndex: currentZIndex
		});

		// Normalize to clean up gaps
		setTimeout(() => normalizeZIndexes(), 0);
	}

	function sendBackward(blockId: string) {
		const block = yBlocks.get(blockId);
		if (!block) return;

		// Get all blocks sorted by z-index
		const allBlocks = Array.from(yBlocks.entries()).map(([id, b]) => ({
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
		const belowZIndex = yBlocks.get(belowBlock.id)?.zIndex || 0;

		yBlocks.set(blockId, { ...block, zIndex: belowZIndex });
		yBlocks.set(belowBlock.id, {
			...yBlocks.get(belowBlock.id)!,
			zIndex: currentZIndex
		});

		// Normalize to clean up gaps
		setTimeout(() => normalizeZIndexes(), 0);
	}

	function bringToFront(blockId: string) {
		const block = yBlocks.get(blockId);
		if (!block) return;

		// Find the highest zIndex
		let maxZIndex = 0;
		yBlocks.forEach((b) => {
			if (b.zIndex > maxZIndex) {
				maxZIndex = b.zIndex;
			}
		});

		yBlocks.set(blockId, { ...block, zIndex: maxZIndex + 1 });

		// Normalize to clean up gaps
		setTimeout(() => normalizeZIndexes(), 0);
	}

	function sendToBack(blockId: string) {
		const block = yBlocks.get(blockId);
		if (!block) return;

		// Find the lowest zIndex
		let minZIndex = Infinity;
		yBlocks.forEach((b) => {
			if (b.zIndex < minZIndex) {
				minZIndex = b.zIndex;
			}
		});

		// Set to below minimum (will be normalized to 1)
		yBlocks.set(blockId, { ...block, zIndex: minZIndex - 1 });

		// Normalize to clean up gaps
		setTimeout(() => normalizeZIndexes(), 0);
	}

	function addBlock(type: string) {
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
			default:
				newBlock.width = 300;
				newBlock.height = 100;
				newBlock.content = "New block";
		}

		yBlocks.set(id, newBlock);
	}
</script>

<div class="builder-container">
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
</div>

<style>
	.builder-container {
		width: 100%;
		height: 100vh;
		overflow: hidden;
		background: var(--bg-secondary, #f5f5f5);
		display: flex;
	}
</style>
