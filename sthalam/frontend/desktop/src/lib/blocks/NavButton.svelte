<script lang="ts">
	interface Props {
		blockId: string;
		blockData: any;
		allBlocks: Map<string, any>;
		onNavigate?: (screenId: string) => void;
	}

	let { blockId, blockData, allBlocks, onNavigate }: Props = $props();

	function handleClick() {
		if (!onNavigate) return;

		let targetScreenId = blockData.targetContainerId;

		// Check if there's branching logic
		if (blockData.questionId && blockData.yesTargetId && blockData.noTargetId) {
			// Get the branching question
			const question = allBlocks.get(blockData.questionId);

			if (question) {
				// Check if user answered the question
				// For now, we'll need to look for the answer in localStorage or a state store
				// This would typically be stored when the user clicks yes/no
				const answer = sessionStorage.getItem(`question_${blockData.questionId}`);

				if (answer === 'yes') {
					targetScreenId = blockData.yesTargetId;
				} else if (answer === 'no') {
					targetScreenId = blockData.noTargetId;
				} else {
					// No answer yet - show prompt
					alert(`Please answer the question: "${question.question}"`);
					return;
				}
			}
		}

		if (targetScreenId) {
			onNavigate(targetScreenId);
		}
	}
</script>

<button class="nav-button" onclick={handleClick} data-block-id={blockId}>
	{blockData.content || 'Next'}
</button>

<style>
	.nav-button {
		padding: 0.75rem 1.5rem;
		background: #28a745;
		color: white;
		border: none;
		border-radius: 6px;
		font-weight: 600;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s;
	}

	.nav-button:hover {
		background: #218838;
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(40, 167, 69, 0.3);
	}

	.nav-button:active {
		transform: translateY(0);
	}
</style>
