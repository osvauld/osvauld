<script lang="ts">
	import Block from "./Block.svelte";
	import { untrack } from "svelte";

	interface Props {
		blocks: Map<string, any>;
	}

	let { blocks }: Props = $props();

	// Current container being viewed
	let currentContainerId = $state<string | null>(null);

	// Container hierarchy: Map of container ID -> array of child blocks
	let containerChildren = $state<Map<string, any[]>>(new Map());

	// All containers sorted by position
	let containers = $state<any[]>([]);

	// Build container hierarchy whenever blocks change
	$effect(() => {
		const allBlocks = Array.from(blocks.values());
		const containerBlocks = allBlocks.filter(b => b.type === 'form-container' || b.type === 'container');

		// Sort containers by position (top to bottom, left to right)
		const sortedContainers = containerBlocks.sort((a, b) => {
			if (Math.abs(a.y - b.y) < 50) {
				return a.x - b.x; // Same row, sort by x
			}
			return a.y - b.y; // Sort by y
		});

		// Build map of which blocks are inside which containers
		const newContainerChildren = new Map<string, any[]>();

		for (const container of sortedContainers) {
			const children: any[] = [];

			for (const block of allBlocks) {
				if (block.id === container.id) continue; // Skip the container itself

				// Check if block is spatially inside this container
				const isInside =
					block.x >= container.x &&
					block.x + block.width <= container.x + container.width &&
					block.y >= container.y &&
					block.y + block.height <= container.y + container.height;

				if (isInside) {
					children.push(block);
				}
			}

			newContainerChildren.set(container.id, children);
		}

		// Update all state without triggering reactivity (use untrack to prevent loop)
		untrack(() => {
			containers = sortedContainers;
			containerChildren = newContainerChildren;

			// Set initial container if not set
			if (sortedContainers.length > 0 && !currentContainerId) {
				currentContainerId = sortedContainers[0].id;
				console.log("🎬 Starting with first container:", currentContainerId);
			}
		});
	});

	// Get current container and its children
	const currentContainer = $derived(() => {
		if (!currentContainerId) return null;
		return blocks.get(currentContainerId);
	});

	const currentChildren = $derived(() => {
		if (!currentContainerId) return [];
		return containerChildren.get(currentContainerId) || [];
	});

	// Navigate to a specific container
	function navigateToContainer(containerId: string) {
		console.log("🎯 Navigating to container:", containerId);
		currentContainerId = containerId;
	}

	// Handle navigation from nav buttons
	function handleNavigation(navButtonId: string) {
		console.log("🧭 Navigation triggered by button:", navButtonId);

		const navButton = blocks.get(navButtonId);
		if (!navButton) {
			console.error("❌ Nav button not found:", navButtonId);
			return;
		}

		// Check if this is a simple navigation (direct targetContainerId)
		if (navButton.targetContainerId) {
			console.log("📍 Direct navigation to container:", navButton.targetContainerId);
			navigateToContainer(navButton.targetContainerId);
			return;
		}

		// Check if this is branching navigation (based on a question)
		const questionId = navButton.questionId;
		if (questionId) {
			handleBranchingNavigation(navButton, questionId);
			return;
		}

		// Default: go to next container in sequence
		console.log("➡️ No target specified, going to next container");
		const currentIndex = containers.findIndex(c => c.id === currentContainerId);
		if (currentIndex >= 0 && currentIndex < containers.length - 1) {
			navigateToContainer(containers[currentIndex + 1].id);
		} else {
			console.warn("⚠️ Already at last container");
		}
	}

	// Handle branching navigation based on question answer
	function handleBranchingNavigation(navButton: any, questionId: string) {
		console.log("🔀 Branching navigation for question:", questionId);

		// Read the answer from the DOM
		const questionElement = document.querySelector(`[data-question-id="${questionId}"]`);
		if (!questionElement) {
			console.error("❌ Question not found:", questionId);
			return;
		}

		// Get selected answer
		const selectedButton = questionElement.parentElement?.querySelector('.option-button.selected');
		if (!selectedButton) {
			console.warn("⚠️ No answer selected yet");
			return;
		}

		const answer = selectedButton.getAttribute('data-answer');
		console.log("📋 User answered:", answer);

		// Get target block ID based on answer
		const targetBlockId = answer === 'yes' ? navButton.yesTargetId : navButton.noTargetId;

		if (!targetBlockId) {
			console.error("❌ No target block configured for answer:", answer);
			return;
		}

		// Navigate to the container containing the target block
		navigateToTargetBlock(targetBlockId);
	}

	// Navigate to the container that contains a specific block
	function navigateToTargetBlock(targetBlockId: string) {
		const targetBlock = blocks.get(targetBlockId);
		if (!targetBlock) {
			console.error("❌ Target block not found:", targetBlockId);
			return;
		}

		// If target is a container, navigate to it directly
		if (targetBlock.type === 'form-container' || targetBlock.type === 'container') {
			navigateToContainer(targetBlockId);
			return;
		}

		// Otherwise, find which container contains the target block
		for (const [containerId, children] of containerChildren.entries()) {
			if (children.some(child => child.id === targetBlockId)) {
				navigateToContainer(containerId);
				return;
			}
		}

		console.error("❌ Could not find container for target block:", targetBlockId);
	}

	// Handle form submission (can also navigate to next container if configured)
	function handleFormSubmit(submitButtonId: string) {
		console.log("🚀 Form submit triggered by button:", submitButtonId);

		const submitButton = blocks.get(submitButtonId);
		if (!submitButton) {
			console.error("❌ Submit button not found:", submitButtonId);
			return;
		}

		// Collect form data from current container
		const formData: Record<string, any> = {};
		let fieldCount = 0;

		for (const block of currentChildren()) {
			if (block.type && block.type.startsWith('form-field-')) {
				const fieldName = block.fieldName || block.id;
				const inputElement = document.querySelector(`[data-field-id="${block.id}"]`) as HTMLInputElement;

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

		console.log(`✅ Form submission complete! Collected ${fieldCount} fields:`, formData);

		// Navigate to next container
		// Priority: targetContainerId > nextContainerId > next in sequence
		const targetId = submitButton.targetContainerId || submitButton.nextContainerId;

		if (targetId) {
			navigateToContainer(targetId);
		} else {
			// Default: go to next container in list
			const currentIndex = containers.findIndex(c => c.id === currentContainerId);
			if (currentIndex >= 0 && currentIndex < containers.length - 1) {
				navigateToContainer(containers[currentIndex + 1].id);
			} else {
				console.log("✅ Form submitted - at last container");
			}
		}
	}

	// Dummy handlers for blocks (they shouldn't be editable in viewer mode)
	function handleBlockUpdate() {}
	function handleBlockSelect() {}
</script>

<div class="fullscreen-viewer">
	{#if currentContainer()}
		<div class="screen-container" style:background-color={currentContainer().styles?.backgroundColor || '#ffffff'}>
			{#each currentChildren() as block (block.id)}
				<div
					class="block-wrapper"
					style:left="{((block.x - currentContainer().x) / currentContainer().width) * 100}%"
					style:top="{((block.y - currentContainer().y) / currentContainer().height) * 100}%"
					style:width="{(block.width / currentContainer().width) * 100}%"
					style:height="{(block.height / currentContainer().height) * 100}%"
				>
					<Block
						{block}
						isSelected={false}
						readonly={true}
						noPositioning={true}
						onUpdate={handleBlockUpdate}
						onSelect={handleBlockSelect}
						onFormSubmit={handleFormSubmit}
						onNavigate={handleNavigation}
					/>
				</div>
			{/each}
		</div>
	{:else}
		<div class="empty-state">
			<div class="empty-icon">📦</div>
			<h2>No containers found</h2>
			<p>Add containers in builder mode to create screens</p>
		</div>
	{/if}

	<!-- Navigation indicators -->
	{#if containers.length > 1}
		<div class="screen-indicators">
			{#each containers as container, index (container.id)}
				<button
					class="indicator"
					class:active={container.id === currentContainerId}
					onclick={() => navigateToContainer(container.id)}
					title="Screen {index + 1}"
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
	}

	.screen-container {
		width: 100%;
		height: 100%;
		position: relative;
		transition: opacity 0.3s ease;
	}

	.block-wrapper {
		position: absolute;
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
		color: #c9d1d9;
	}

	.empty-state p {
		color: #8b949e;
		line-height: 1.6;
	}

	.screen-indicators {
		position: absolute;
		bottom: 2rem;
		left: 50%;
		transform: translateX(-50%);
		display: flex;
		gap: 0.75rem;
		padding: 0.75rem 1.5rem;
		background: rgba(0, 0, 0, 0.5);
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
