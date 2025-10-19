<script lang="ts">
	import type * as Y from 'yjs';

	interface Props {
		blockId: string;
		blockData: any;
		allBlocks: Map<string, any>;
		ydoc?: Y.Doc;
		onNavigate?: (screenId: string) => void;
	}

	let { blockId, blockData, allBlocks, ydoc, onNavigate }: Props = $props();

	function handleClick() {
		const action = blockData.action || 'navigate'; // Default to navigate for backwards compatibility
		const targetId = blockData.targetContainerId;

		if (!targetId) {
			console.warn('⚠️ Nav button has no target configured');
			return;
		}

		console.log(`🧭 Nav button clicked - Action: ${action}, Target: ${targetId}`);

		// Handle different actions
		switch (action) {
			case 'show':
				updateContainerVisibility(targetId, true);
				break;

			case 'hide':
				updateContainerVisibility(targetId, false);
				break;

			case 'toggle':
				toggleContainerVisibility(targetId);
				break;

			case 'navigate':
				// Navigate to a screen
				if (onNavigate) {
					// Check for branching logic
					const resolvedTargetId = resolveBranchingTarget(targetId);
					onNavigate(resolvedTargetId);
				}
				break;

			default:
				console.warn(`⚠️ Unknown navigation action: ${action}`);
		}
	}

	function resolveBranchingTarget(defaultTargetId: string): string {
		// Check if there's branching logic
		if (blockData.questionId && blockData.yesTargetId && blockData.noTargetId) {
			const question = allBlocks.get(blockData.questionId);

			if (question) {
				const answer = sessionStorage.getItem(`question_${blockData.questionId}`);

				if (answer === 'yes') {
					return blockData.yesTargetId;
				} else if (answer === 'no') {
					return blockData.noTargetId;
				} else {
					alert(`Please answer the question: "${question.question}"`);
					return defaultTargetId;
				}
			}
		}

		return defaultTargetId;
	}

	function updateContainerVisibility(containerId: string, visible: boolean) {
		if (!ydoc) {
			console.error('❌ No Yjs document available');
			return;
		}

		const blocks = ydoc.getMap('blocks');
		const container = blocks.get(containerId);

		if (!container) {
			console.error(`❌ Container not found: ${containerId}`);
			return;
		}

		// Update visibility in Yjs
		ydoc.transact(() => {
			container.visible = visible;
			blocks.set(containerId, container);
		});

		console.log(`✅ Container ${containerId} visibility set to: ${visible}`);
	}

	function toggleContainerVisibility(containerId: string) {
		if (!ydoc) {
			console.error('❌ No Yjs document available');
			return;
		}

		const blocks = ydoc.getMap('blocks');
		const container = blocks.get(containerId);

		if (!container) {
			console.error(`❌ Container not found: ${containerId}`);
			return;
		}

		// Toggle visibility
		const newVisibility = !container.visible;
		updateContainerVisibility(containerId, newVisibility);
	}
</script>

<button class="nav-button" style={blockData.css || ""} onclick={handleClick} data-block-id={blockId}>
	{blockData.content || 'Next'}
</button>

<style>
	.nav-button {
		/* Default fallback styles - can be overridden by custom CSS */
		padding: 0.75rem 1.5rem;
		background: #89b4fa;
		color: #010409;
		border: none;
		border-radius: 8px;
		font-weight: 600;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s;
	}

	.nav-button:hover {
		opacity: 0.9;
		transform: translateY(-2px);
	}

	.nav-button:active {
		transform: translateY(0);
	}
</style>
