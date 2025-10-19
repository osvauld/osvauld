<script lang="ts">
	import { onMount, onDestroy, untrack } from "svelte";
	import { dataState, uiState } from "../state";
	import Canvas from "./Canvas.svelte";
	import BlockPalette from "./BlockPalette.svelte";
	import PropertiesPanel from "./PropertiesPanel.svelte";
	import KeyboardShortcuts from "./KeyboardShortcuts.svelte";
	import NavigationPanel from "../components/NavigationPanel.svelte";
	import NavigationToggle from "../components/NavigationToggle.svelte";
	import type { YjsDocuments } from "./yjsManager";
	import type { BlocksuiteStore } from "./blocksuiteStore";

	let yDocs: YjsDocuments | null = null;
	let blocks = $state<Map<string, any>>(new Map());
	let viewport = $state({ x: 0, y: 0, zoom: 1 });
	let selectedBlockId = $state<string | null>(null);
	let autoSaveInterval: number | null = null;
	let blocksuiteStore: BlocksuiteStore | null = null;
	let blocksuiteUnsubscribe: (() => void) | null = null;

	const selectedBlock = $derived(
		selectedBlockId ? blocks.get(selectedBlockId) || null : null
	);

	// Track if we have a resource selected
	const hasResource = $derived(!!dataState.currentResourceId);

	onMount(async () => {
		console.log("🚀 Initializing Website Builder...");
		console.log("✅ Website Builder initialized!");
	});

	// React to resource changes (like livnote's pattern)
	// Only track resourceId - don't track blocks/viewport changes
	$effect(() => {
		const effectStartTime = performance.now();
		const resourceId = dataState.currentResourceId;
		console.log("🔄 [EFFECT START] WebsiteBuilder $effect triggered at", effectStartTime, "- resourceId:", resourceId);

		// Clear existing autosave interval when resource changes
		if (autoSaveInterval !== null) {
			clearInterval(autoSaveInterval);
			autoSaveInterval = null;
		}

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
		console.log("✅ [EFFECT] Resource selected - setting up workspace at", performance.now() - effectStartTime, "ms");

		// Use untracked to avoid infinite loops
		untrack(() => {
			console.log("⏱️ [EFFECT] Inside untrack at", performance.now() - effectStartTime, "ms");
			const coordinator = dataState.getBlocksuiteCoordinator();
			if (!coordinator) {
				console.error("❌ No coordinator available");
				return;
			}

			console.log("⏱️ [EFFECT] Got coordinator, getting documents at", performance.now() - effectStartTime, "ms");
			// Get the FRESH Yjs documents (after loadBlocksuite was called)
			const docs = coordinator.getDocuments();
			if (!docs) {
				console.error("❌ No Yjs documents available");
				return;
			}

			console.log("⏱️ [EFFECT] Got documents, setting yDocs at", performance.now() - effectStartTime, "ms");
			yDocs = docs;

			console.log("📦 Yjs documents received:", {
				blocks: docs.blocks.size,
				viewport: {
					x: docs.viewport.get("x"),
					y: docs.viewport.get("y"),
					zoom: docs.viewport.get("zoom")
				}
			});

			console.log("⏱️ [EFFECT] Setting up BlocksuiteStore subscription at", performance.now() - effectStartTime, "ms");

			// Get BlocksuiteStore and subscribe to it
			const store = coordinator.getBlocksuiteStore();
			if (store) {
				// Clean up previous subscription
				if (blocksuiteUnsubscribe) {
					blocksuiteUnsubscribe();
				}

				blocksuiteStore = store;

				// Subscribe to blocks changes via store
				blocksuiteUnsubscribe = blocksuiteStore.subscribe(() => {
					blocks = blocksuiteStore!.getAllBlocks();
				});

				// Initial load from store
				blocks = blocksuiteStore.getAllBlocks();
				console.log("✅ Subscribed to BlocksuiteStore, got", blocks.size, "blocks");
			}

			// Subscribe to viewport changes (still direct Yjs - no ViewportStore needed)
			const viewportObserver = () => {
				if (!yDocs) return;
				viewport = {
					x: yDocs.viewport.get("x") || 0,
					y: yDocs.viewport.get("y") || 0,
					zoom: yDocs.viewport.get("zoom") || 1,
				};
			};

			console.log("⏱️ [EFFECT] Attaching viewport observer at", performance.now() - effectStartTime, "ms");
			docs.viewport.observe(viewportObserver);

			console.log("⏱️ [EFFECT] Running initial viewport observer at", performance.now() - effectStartTime, "ms");
			// Initial viewport load
			viewportObserver();
			console.log("⏱️ [EFFECT] Initial setup complete at", performance.now() - effectStartTime, "ms");

			console.log("⏱️ [EFFECT] Starting autosave timer at", performance.now() - effectStartTime, "ms");
			// Start autosave timer for this resource (60 second interval)
			autoSaveInterval = window.setInterval(async () => {
				const currentResourceId = dataState.currentResourceId;
				if (currentResourceId === resourceId) {
					try {
						console.log("💾 Auto-saving resource:", currentResourceId);
						await dataState.saveCurrentResource(currentResourceId);
						console.log("✅ Auto-save completed");
					} catch (error) {
						console.error("❌ Auto-save failed:", error);
					}
				}
			}, 60000); // 60 seconds
			console.log("⏰ Auto-save started for resource:", resourceId);
			console.log("🏁 [EFFECT COMPLETE] WebsiteBuilder $effect finished at", performance.now() - effectStartTime, "ms");
		});

		// Cleanup function for this effect
		return () => {
			// Clear autosave interval on cleanup
			if (autoSaveInterval !== null) {
				clearInterval(autoSaveInterval);
				autoSaveInterval = null;
			}

			// Unsubscribe from BlocksuiteStore
			if (blocksuiteUnsubscribe) {
				blocksuiteUnsubscribe();
				blocksuiteUnsubscribe = null;
			}
		};
	});

	// Track loading state changes for debugging
	$effect(() => {
		const isLoading = dataState.isResourceLoading;
		console.log("🔄 [LOADING STATE CHANGE] isResourceLoading =", isLoading, "at", performance.now());
		if (!isLoading) {
			console.log("✅ [LOADING STATE] Loading complete - UI should be interactive now");
		}
	});

	onDestroy(() => {
		// Clear auto-save interval
		if (autoSaveInterval !== null) {
			clearInterval(autoSaveInterval);
			autoSaveInterval = null;
		}

		// Unsubscribe from BlocksuiteStore
		if (blocksuiteUnsubscribe) {
			blocksuiteUnsubscribe();
		}

		// Note: Coordinator cleanup is handled by dataState.clearAllState() when needed
	});

	function updateViewport(newViewport: { x?: number; y?: number; zoom?: number }) {
		if (!yDocs) return;
		if (newViewport.x !== undefined) yDocs.viewport.set("x", newViewport.x);
		if (newViewport.y !== undefined) yDocs.viewport.set("y", newViewport.y);
		if (newViewport.zoom !== undefined) yDocs.viewport.set("zoom", newViewport.zoom);
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

	function deleteBlock(blockId: string) {
		if (!yDocs) return;
		yDocs.blocks.delete(blockId);
		// Clear selection if the deleted block was selected
		if (selectedBlockId === blockId) {
			selectedBlockId = null;
		}
	}

	function addBlock(type: string, x?: number, y?: number) {
		if (!yDocs) return;
		const id = `block-${Date.now()}`;

		let centerX: number;
		let centerY: number;

		// If x and y are provided (from drag-drop), use them directly
		if (x !== undefined && y !== undefined) {
			centerX = x;
			centerY = y;
		} else {
			// Calculate center of current viewport (click mode)
			// The canvas has: transform: translate(viewport.x, viewport.y) scale(viewport.zoom)
			// BlockPalette takes 200px on the left, PropertiesPanel takes 300px on the right if open
			const paletteWidth = 200;
			const propertiesPanelWidth = selectedBlockId ? 300 : 0;

			// Center of visible canvas area in screen coordinates (pixels on screen)
			const screenCenterX = paletteWidth + (window.innerWidth - paletteWidth - propertiesPanelWidth) / 2;
			const screenCenterY = window.innerHeight / 2;

			// Convert screen coordinates to canvas coordinates
			// Screen position = (canvas position * zoom) + viewport offset + palette offset
			// So: canvas position = (screen position - palette offset - viewport offset) / zoom
			centerX = (screenCenterX - paletteWidth - viewport.x) / viewport.zoom;
			centerY = (screenCenterY - viewport.y) / viewport.zoom;
		}

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
			case "form-container":
				newBlock.width = 500;
				newBlock.height = 400;
				newBlock.content = "";
				newBlock.label = "Form";
				newBlock.styles = {
					backgroundColor: "rgba(255,255,255,0.9)",
					border: "2px solid #667eea",
					padding: "20px"
				};
				break;
			case "form-field-text":
				newBlock.width = 350;
				newBlock.height = 80;
				newBlock.label = "Text Field";
				newBlock.placeholder = "Enter text...";
				newBlock.required = false;
				newBlock.fieldName = `field_${Date.now()}`;
				break;
			case "form-field-email":
				newBlock.width = 350;
				newBlock.height = 80;
				newBlock.label = "Email";
				newBlock.placeholder = "Enter email...";
				newBlock.required = false;
				newBlock.fieldName = `email_${Date.now()}`;
				break;
			case "form-field-number":
				newBlock.width = 350;
				newBlock.height = 80;
				newBlock.label = "Number";
				newBlock.placeholder = "Enter number...";
				newBlock.required = false;
				newBlock.fieldName = `number_${Date.now()}`;
				break;
			case "form-field-textarea":
				newBlock.width = 350;
				newBlock.height = 120;
				newBlock.label = "Message";
				newBlock.placeholder = "Enter message...";
				newBlock.required = false;
				newBlock.fieldName = `message_${Date.now()}`;
				break;
			case "form-field-checkbox":
				newBlock.width = 350;
				newBlock.height = 60;
				newBlock.label = "I agree to terms";
				newBlock.required = false;
				newBlock.fieldName = `checkbox_${Date.now()}`;
				break;
			case "form-submit-button":
				newBlock.width = 150;
				newBlock.height = 50;
				newBlock.content = "Submit";
				newBlock.styles = {
					backgroundColor: "#667eea",
					color: "#ffffff"
				};
				break;
			case "branching-question":
				newBlock.width = 600;
				newBlock.height = 200;
				newBlock.question = "Are you a new user?";
				newBlock.yesLabel = "Yes";
				newBlock.noLabel = "No";
				newBlock.styles = {
					backgroundColor: "rgba(255,255,255,0.95)",
					border: "2px solid #667eea",
					borderRadius: "12px"
				};
				break;
			case "nav-button":
				newBlock.width = 200;
				newBlock.height = 60;
				newBlock.content = "Next";
				newBlock.questionId = ""; // Will be configured in properties panel
				newBlock.branches = {
					yes: { x: 0, y: -1000, zoom: 1 },
					no: { x: 0, y: -1000, zoom: 1 }
				};
				newBlock.styles = {
					backgroundColor: "#48bb78",
					color: "#ffffff"
				};
				break;
			case "screen-container":
				newBlock.width = 800;
				newBlock.height = 600;
				newBlock.name = "New Screen";
				newBlock.isEntryPoint = false;
				newBlock.css = "padding: 2rem; background: #f5f5f5;";
				break;
			case "section-container":
				newBlock.width = 600;
				newBlock.height = 400;
				newBlock.name = "New Section";
				newBlock.css = "padding: 1rem; background: #ffffff; border: 1px solid #e0e0e0;";
				newBlock.visible = true;
				newBlock.isModal = false;
				break;
			case "markdown-text":
				newBlock.width = 500;
				newBlock.height = 300;
				newBlock.content = "# New Markdown\n\nStart typing...";
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
		{#if !uiState.showNavigationPanel}
			<div class="floating-toggle">
				<NavigationToggle />
			</div>
		{/if}
		<div class="empty-message">
			<h2>No resource selected</h2>
			<p>Select a resource from the sidebar or create a new one to get started.</p>
		</div>
	</div>
{:else}
	<!-- Resource selected - show builder -->
	<div class="builder-container">
		<!-- Loading overlay for heavy documents -->
		{#if dataState.isResourceLoading}
			<div class="loading-overlay">
				<div class="loading-spinner">
					<div class="spinner"></div>
					<p>Loading resource...</p>
					<span class="loading-hint">Heavy documents may take a moment</span>
				</div>
			</div>
		{/if}
		<NavigationPanel />
		{#if !uiState.showNavigationPanel}
			<div class="floating-toggle">
				<NavigationToggle />
			</div>
		{/if}
		<BlockPalette onAddBlock={addBlock} />
		<Canvas
			{blocks}
			{viewport}
			{selectedBlockId}
			onViewportChange={updateViewport}
			onBlockUpdate={updateBlock}
			onBlockSelect={(id) => (selectedBlockId = id)}
			onBlockDrop={addBlock}
		/>
		<PropertiesPanel
			{selectedBlock}
			{blocks}
			onUpdateBlock={updateBlock}
			onBringForward={bringForward}
			onSendBackward={sendBackward}
			onBringToFront={bringToFront}
			onSendToBack={sendToBack}
			onDeleteBlock={deleteBlock}
		/>
		<KeyboardShortcuts />
	</div>
{/if}

<style>
	.builder-container {
		width: 100%;
		height: 100%;
		overflow: hidden;
		background: #010409;
		display: flex;
	}

	.empty-state {
		width: 100%;
		height: 100%;
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

	.floating-toggle {
		position: absolute;
		top: 1rem;
		left: 1rem;
		z-index: 1000;
	}

	/* Loading overlay for heavy documents */
	.loading-overlay {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: rgba(1, 4, 9, 0.95);
		backdrop-filter: blur(8px);
		z-index: 9999;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.loading-spinner {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.5rem;
	}

	.spinner {
		width: 48px;
		height: 48px;
		border: 4px solid #2f303e;
		border-top-color: #667eea;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	.loading-spinner p {
		color: #f2f2f0;
		font-size: 1.125rem;
		font-weight: 500;
		margin: 0;
	}

	.loading-hint {
		color: #8b949e;
		font-size: 0.875rem;
	}
</style>
