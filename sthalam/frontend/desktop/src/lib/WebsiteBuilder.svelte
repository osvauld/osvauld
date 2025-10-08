<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import * as Y from "yjs";
	import { editorStore } from "../store.svelte";
	import Canvas from "./Canvas.svelte";
	import BlockPalette from "./BlockPalette.svelte";
	import PropertiesPanel from "./PropertiesPanel.svelte";

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

	function bringForward(blockId: string) {
		const block = yBlocks.get(blockId);
		if (block) {
			// Find the highest zIndex
			let maxZIndex = 0;
			yBlocks.forEach((b) => {
				if (b.zIndex > maxZIndex) {
					maxZIndex = b.zIndex;
				}
			});
			yBlocks.set(blockId, { ...block, zIndex: maxZIndex + 1 });
		}
	}

	function sendBackward(blockId: string) {
		const block = yBlocks.get(blockId);
		if (block && block.zIndex > 1) {
			yBlocks.set(blockId, { ...block, zIndex: block.zIndex - 1 });
		}
	}

	function bringToFront(blockId: string) {
		bringForward(blockId);
	}

	function sendToBack(blockId: string) {
		const block = yBlocks.get(blockId);
		if (block) {
			yBlocks.set(blockId, { ...block, zIndex: 0 });
		}
	}

	function addBlock(type: string) {
		const id = `block-${Date.now()}`;

		// Calculate center of viewport
		const centerX = Math.abs(viewport.x) + 400;
		const centerY = Math.abs(viewport.y) + 200;

		const newBlock = {
			id,
			type,
			x: centerX,
			y: centerY,
			width: type === "heading" ? 400 : 300,
			height: type === "heading" ? 60 : 100,
			zIndex: blocks.size + 1,
			content: type === "heading" ? "New Heading" : type === "text" ? "New text block" : "Container",
			styles: type === "heading"
				? { fontSize: "32px", fontWeight: "700" }
				: {},
		};
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
	/>
</div>

<style>
	.builder-container {
		width: 100%;
		height: 100vh;
		overflow: hidden;
		background: #f5f5f5;
		display: flex;
	}
</style>
