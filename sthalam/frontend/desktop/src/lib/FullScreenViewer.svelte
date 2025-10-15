<script lang="ts">
	import { untrack } from "svelte";
	import { marked } from 'marked';
	import ThreadBlock from './blocks/ThreadBlock.svelte';
	import FormField from './blocks/FormField.svelte';
	import FormSubmitButton from './blocks/FormSubmitButton.svelte';
	import NavButton from './blocks/NavButton.svelte';
	import BranchingQuestion from './blocks/BranchingQuestion.svelte';
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

			// Set initial screen if not set
			if (sortedScreens.length > 0 && !currentScreenId) {
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

	// Get all blocks for current screen (hierarchical)
	const screenBlocks = $derived(() => {
		const screen = currentScreen();
		if (!screen) return [];

		function collectBlocks(parentId: string): any[] {
			const children = buildTree(parentId);
			let result: any[] = [];

			for (const child of children) {
				result.push(child);
				result.push(...collectBlocks(child.id));
			}

			return result;
		}

		return [screen, ...collectBlocks(screen.id)];
	});

	// Navigate to a screen
	function navigateToScreen(screenId: string) {
		console.log("🎯 Navigating to screen:", screenId);
		currentScreenId = screenId;
	}
</script>

<div class="fullscreen-viewer">
	{#if currentScreen()}
		<div class="viewer-content">
			<div class="container-screen-container" style={currentScreen().css || ""}>
				{#each screenBlocks() as block (block.id)}
					{#if block.type === 'screen-container'}
						<!-- Skip screen container itself, we rendered it above -->
					{:else if block.type === 'section-container'}
						<div class="container-section-container" style={block.css || ""} data-block-id={block.id}>
							<!-- Children will be rendered in next iteration -->
						</div>
					{:else if block.type === 'thread'}
						<ThreadBlock blockId={block.id} {ydoc} {commentsDoc} />
					{:else if block.type.startsWith('form-field-')}
						<FormField blockId={block.id} blockData={block} />
					{:else if block.type === 'form-submit-button'}
						<FormSubmitButton blockId={block.id} blockData={block} {ydoc} {submissionsDoc} allBlocks={blocks} onNavigate={navigateToScreen} />
					{:else if block.type === 'nav-button'}
						<NavButton blockId={block.id} blockData={block} allBlocks={blocks} onNavigate={navigateToScreen} />
					{:else if block.type === 'branching-question'}
						<BranchingQuestion blockId={block.id} blockData={block} />
					{:else if block.type === 'heading'}
						<h1 style={block.css || ""} data-block-id={block.id}>{block.content || ''}</h1>
					{:else if block.type === 'text'}
						<p style={block.css || ""} data-block-id={block.id}>{block.content || ''}</p>
					{:else if block.type === 'image'}
						<img src={block.content || ''} alt="" style={block.css || ""} data-block-id={block.id} />
					{:else if block.type === 'form'}
						<!-- Form metadata is invisible -->
					{:else if block.type === 'html'}
						<div class="html-block" style={block.css || ""} data-block-id={block.id}>
							{@html block.content || ''}
						</div>
					{/if}
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
		overflow: auto;
		background: #ffffff;
	}

	.viewer-content {
		width: 100%;
		min-height: 100%;
		padding: 2rem;
		box-sizing: border-box;
	}

	/* Custom scrollbar for viewer */
	.fullscreen-viewer::-webkit-scrollbar {
		width: 10px;
	}

	.fullscreen-viewer::-webkit-scrollbar-track {
		background: #f1f3f5;
	}

	.fullscreen-viewer::-webkit-scrollbar-thumb {
		background: #adb5bd;
		border-radius: 5px;
	}

	.fullscreen-viewer::-webkit-scrollbar-thumb:hover {
		background: #868e96;
	}

	.viewer-content :global(.container-screen-container),
	.viewer-content :global(.container-section-container) {
		width: 100%;
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
</style>
