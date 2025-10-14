<script lang="ts">
	import { untrack } from "svelte";
	import { marked } from 'marked';

	interface Props {
		blocks: Map<string, any>;
	}

	let { blocks }: Props = $props();

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
	function renderBlock(block: any): string {
		const children = buildTree(block.id);
		const childrenHtml = children.map(child => renderBlock(child)).join('');

		// Get CSS for this block
		const css = block.css || '';

		// Render based on block type
		if (block.type === 'screen-container' || block.type === 'section-container') {
			return `<div class="container-${block.type}" style="${css}" data-block-id="${block.id}">${childrenHtml}</div>`;
		} else if (block.type === 'heading') {
			return `<h1 style="${css}" data-block-id="${block.id}">${block.content || ''}</h1>`;
		} else if (block.type === 'text') {
			return `<p style="${css}" data-block-id="${block.id}">${block.content || ''}</p>`;
		} else if (block.type === 'image') {
			return `<img src="${block.content || ''}" style="${css}" data-block-id="${block.id}" />`;
		} else if (block.type === 'thread') {
			// Render thread block content (just display the main post for now)
			let threadContent = '';
			try {
				if (block.mode === 'html') {
					threadContent = block.content || '';
				} else {
					// Default to markdown
					threadContent = marked.parse(block.content || '');
				}
			} catch (error) {
				threadContent = '<p>Error rendering thread</p>';
			}

			const customCss = block.css ? `<style>${block.css}</style>` : '';

			return `
				<div class="thread-block" style="${css}" data-block-id="${block.id}">
					${customCss}
					<div class="thread-content">${threadContent}</div>
					${childrenHtml}
				</div>
			`;
		} else if (block.type === 'form-field-text') {
			const label = block.label || 'Text Field';
			const required = block.required ? '*' : '';
			return `
				<div class="form-field" style="${css}" data-block-id="${block.id}">
					<label>${label}${required}</label>
					<input type="text" placeholder="${block.placeholder || ''}" data-field-id="${block.id}" ${block.required ? 'required' : ''} />
				</div>
			`;
		} else if (block.type === 'form-field-password') {
			const label = block.label || 'Password';
			const required = block.required ? '*' : '';
			return `
				<div class="form-field" style="${css}" data-block-id="${block.id}">
					<label>${label}${required}</label>
					<input type="password" placeholder="${block.placeholder || ''}" data-field-id="${block.id}" ${block.required ? 'required' : ''} />
				</div>
			`;
		} else if (block.type === 'form-field-email') {
			const label = block.label || 'Email';
			const required = block.required ? '*' : '';
			return `
				<div class="form-field" style="${css}" data-block-id="${block.id}">
					<label>${label}${required}</label>
					<input type="email" placeholder="${block.placeholder || ''}" data-field-id="${block.id}" ${block.required ? 'required' : ''} />
				</div>
			`;
		} else if (block.type === 'form-submit-button') {
			return `
				<button class="submit-button" style="${css}" data-block-id="${block.id}" data-form-id="${block.formId || ''}" data-target="${block.targetContainerId || ''}">${block.content || 'Submit'}</button>
			`;
		} else if (block.type === 'form') {
			// Form metadata is invisible
			return '';
		}

		return `<div style="${css}" data-block-id="${block.id}">${childrenHtml}</div>`;
	}

	// Get current screen
	const currentScreen = $derived(() => {
		if (!currentScreenId) return null;
		return blocks.get(currentScreenId);
	});

	// Rendered HTML for current screen
	const renderedHtml = $derived(() => {
		const screen = currentScreen();
		if (!screen) return '';
		return renderBlock(screen);
	});

	// Navigate to a screen
	function navigateToScreen(screenId: string) {
		console.log("🎯 Navigating to screen:", screenId);
		currentScreenId = screenId;
	}

	// Handle form submission
	function handleFormSubmission(event: Event) {
		const button = event.target as HTMLButtonElement;
		const formId = button.getAttribute('data-form-id');
		const targetScreenId = button.getAttribute('data-target');

		console.log("🚀 Form submit triggered:", { formId, targetScreenId });

		if (!formId) {
			console.warn("⚠️ Submit button has no formId");
			return;
		}

		// Collect form data
		const formData: Record<string, any> = {};
		const allBlocks = Array.from(blocks.values());

		for (const block of allBlocks) {
			if (block.formId === formId && block.type?.startsWith('form-field-')) {
				const fieldName = block.fieldName || block.label || block.id;
				const inputElement = document.querySelector(`[data-field-id="${block.id}"]`) as HTMLInputElement;

				if (inputElement) {
					formData[fieldName] = inputElement.value;
					console.log(`  ✓ Field "${fieldName}":`, formData[fieldName]);
				}
			}
		}

		// Get form metadata
		const formBlock = allBlocks.find(b => b.id === formId);
		const eventName = formBlock?.eventName;

		console.log(`✅ Form submission:`, {
			eventName,
			formId,
			data: formData,
			timestamp: Date.now()
		});

		// Navigate to target screen if specified
		if (targetScreenId) {
			navigateToScreen(targetScreenId);
		}
	}

	// Set up click listener for submit buttons
	function setupListeners(node: HTMLElement) {
		function onClick(event: Event) {
			const target = event.target as HTMLElement;
			if (target.classList.contains('submit-button')) {
				handleFormSubmission(event);
			}
		}

		node.addEventListener('click', onClick);

		return {
			destroy() {
				node.removeEventListener('click', onClick);
			}
		};
	}
</script>

<div class="fullscreen-viewer">
	{#if currentScreen()}
		<div class="viewer-content" use:setupListeners>
			{@html renderedHtml()}
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
	}

	.viewer-content :global(.container-screen-container),
	.viewer-content :global(.container-section-container) {
		width: 100%;
	}

	.viewer-content :global(.form-field) {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.viewer-content :global(.form-field label) {
		font-size: 0.875rem;
		font-weight: 500;
		color: #333;
	}

	.viewer-content :global(.form-field input) {
		padding: 0.75rem;
		border: 1px solid #ddd;
		border-radius: 6px;
		font-size: 0.875rem;
	}

	.viewer-content :global(.form-field input:focus) {
		outline: none;
		border-color: #667eea;
		box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
	}

	.viewer-content :global(.submit-button) {
		cursor: pointer;
		transition: all 0.2s;
	}

	.viewer-content :global(.submit-button:hover) {
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
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

	/* Thread block styling */
	.viewer-content :global(.thread-block) {
		width: 100%;
		margin: 1rem 0;
		padding: 1.5rem;
		background: #f6f8fa;
		border-radius: 8px;
		border: 1px solid #e1e4e8;
	}

	.viewer-content :global(.thread-content) {
		color: #24292e;
		line-height: 1.6;
	}

	.viewer-content :global(.thread-content h1),
	.viewer-content :global(.thread-content h2),
	.viewer-content :global(.thread-content h3) {
		color: #24292e;
		margin-bottom: 0.75rem;
	}

	.viewer-content :global(.thread-content p) {
		margin-bottom: 1rem;
	}

	.viewer-content :global(.thread-content a) {
		color: #0366d6;
		text-decoration: none;
	}

	.viewer-content :global(.thread-content a:hover) {
		text-decoration: underline;
	}
</style>
