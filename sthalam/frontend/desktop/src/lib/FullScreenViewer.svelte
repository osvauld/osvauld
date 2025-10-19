<script lang="ts">
	import { untrack } from "svelte";
	import { marked } from 'marked';
	import BlockRenderer from './blocks/BlockRenderer.svelte';
	import type * as Y from 'yjs';

	interface Props {
		blocks: Map<string, any>;
		ydoc?: Y.Doc;
		commentsDoc?: Y.Doc;
		submissionsDoc?: Y.Doc;
	}

	let { blocks, ydoc, commentsDoc, submissionsDoc }: Props = $props();

	// Current screen being viewed
	let currentScreenId = $state<string | null>(null);

	// All screen containers
	let screens = $state<any[]>([]);

	// Build hierarchy based on parentId relationships
	$effect(() => {
		const allBlocks = Array.from(blocks.values());

		// Find all screen containers
		const screenBlocks = allBlocks.filter(b => b.type === 'screen-container');

		// Sort screens by position (for navigation order)
		const sortedScreens = screenBlocks.sort((a, b) => {
			if (Math.abs(a.y - b.y) < 50) {
				return a.x - b.x; // Same row, sort by x
			}
			return a.y - b.y; // Sort by y
		});

		untrack(() => {
			screens = sortedScreens;

			// Reset currentScreenId if it doesn't exist in the new blocks (resource switch)
			const currentScreenExists = currentScreenId && sortedScreens.some(s => s.id === currentScreenId);

			// Set initial screen if not set OR if current screen no longer exists
			if (sortedScreens.length > 0 && (!currentScreenId || !currentScreenExists)) {
				// Check if any screen is marked as entry point
				const entryPointScreen = sortedScreens.find(s => s.isEntryPoint);
				currentScreenId = entryPointScreen ? entryPointScreen.id : sortedScreens[0].id;
				console.log("🎬 Starting with first screen:", currentScreenId);
			}
		});
	});

	// Build hierarchical tree based on parentId
	function buildTree(parentId: string | null = null): any[] {
		const allBlocks = Array.from(blocks.values());
		return allBlocks
			.filter(b => b.parentId === parentId)
			.sort((a, b) => (a.order || 0) - (b.order || 0));
	}

	// Recursively render a block and its children
	function renderBlockComponent(block: any) {
		const children = buildTree(block.id);
		return { block, children };
	}

	// Get current screen
	const currentScreen = $derived(() => {
		if (!currentScreenId) return null;
		return blocks.get(currentScreenId);
	});

	// Get children for a specific parent (with visibility filtering)
	function getChildren(parentId: string): any[] {
		return buildTree(parentId).filter(child => {
			// Filter out hidden section containers
			if (child.type === 'section-container' && child.visible === false) {
				return false;
			}
			return true;
		});
	}

	// Navigate to a screen
	function navigateToScreen(screenId: string) {
		console.log("🎯 Navigating to screen:", screenId);
		currentScreenId = screenId;
	}
</script>

<div class="fullscreen-viewer">
	{#if currentScreen()}
		<div class="viewer-content">
			<div class="container-screen-container" style={currentScreen().css || ""} data-block-id={currentScreen().id}>
				<!-- Render all top-level children of the screen recursively -->
				{#each getChildren(currentScreen().id) as child (child.id)}
					<BlockRenderer
						block={child}
						children={getChildren(child.id)}
						{ydoc}
						{commentsDoc}
						{submissionsDoc}
						allBlocks={blocks}
						onNavigate={navigateToScreen}
					/>
				{/each}
			</div>
		</div>
	{:else}
		<div class="empty-state">
			<div class="empty-icon">🖥️</div>
			<h2>No screens found</h2>
			<p>Add a Screen Container in builder mode to create pages</p>
		</div>
	{/if}

	<!-- Screen navigation indicators -->
	{#if screens.length > 1}
		<div class="screen-indicators">
			{#each screens as screen, index (screen.id)}
				<button
					class="indicator"
					class:active={screen.id === currentScreenId}
					onclick={() => navigateToScreen(screen.id)}
					title={screen.name || `Screen ${index + 1}`}
				></button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.fullscreen-viewer {
		width: 100%;
		height: 100%;
		position: relative;
		overflow: hidden;
		background: #010409;
	}

	.viewer-content {
		width: 100%;
		height: 100%;
		box-sizing: border-box;
		overflow-y: auto;
	}

	/* Scrollbar for viewer-content */
	.viewer-content::-webkit-scrollbar {
		width: 8px;
	}

	.viewer-content::-webkit-scrollbar-track {
		background: transparent;
	}

	.viewer-content::-webkit-scrollbar-thumb {
		background: #2f303e;
		border-radius: 4px;
	}

	.viewer-content::-webkit-scrollbar-thumb:hover {
		background: #4d4f60;
	}


	.viewer-content :global(.container-screen-container),
	.viewer-content :global(.container-section-container) {
		width: 100%;
		border: none;
		outline: none;
	}

	.viewer-content :global(.container-screen-container) {
		min-height: 100vh;
	}

	.html-block {
		width: 100%;
	}

	.empty-state {
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		padding: 2rem;
	}

	.empty-icon {
		font-size: 3rem;
		margin-bottom: 1rem;
		opacity: 0.3;
	}

	.empty-state h2 {
		font-size: 1.5rem;
		margin-bottom: 1rem;
		color: #1a1a1a;
	}

	.empty-state p {
		color: #666;
		line-height: 1.6;
	}

	.screen-indicators {
		position: fixed;
		bottom: 2rem;
		left: 50%;
		transform: translateX(-50%);
		display: flex;
		gap: 0.75rem;
		padding: 0.75rem 1.5rem;
		background: rgba(0, 0, 0, 0.8);
		backdrop-filter: blur(10px);
		border-radius: 2rem;
		z-index: 1000;
	}

	.indicator {
		width: 12px;
		height: 12px;
		border: none;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.4);
		cursor: pointer;
		transition: all 0.3s ease;
		padding: 0;
	}

	.indicator:hover {
		background: rgba(255, 255, 255, 0.6);
		transform: scale(1.2);
	}

	.indicator.active {
		background: #ffffff;
		width: 32px;
		border-radius: 1rem;
	}

	/* Modal styles */
	.modal-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.5);
		backdrop-filter: blur(4px);
	}

	.modal-container {
		/* Modal containers can use their own positioning */
		box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
	}
</style>
