<script lang="ts">
	interface Props {
		blockId: string;
		blockData: any;
	}

	let { blockId, blockData }: Props = $props();

	let selectedAnswer = $state<'yes' | 'no' | null>(null);

	function handleAnswer(answer: 'yes' | 'no') {
		selectedAnswer = answer;
		// Store answer in sessionStorage for nav-button to use
		sessionStorage.setItem(`question_${blockId}`, answer);

		// Dispatch event
		const event = new CustomEvent('question-answered', {
			detail: {
				questionId: blockId,
				answer
			},
			bubbles: true,
			composed: true
		});
		document.dispatchEvent(event);
	}

	// Check if already answered
	$effect(() => {
		const stored = sessionStorage.getItem(`question_${blockId}`);
		if (stored === 'yes' || stored === 'no') {
			selectedAnswer = stored;
		}
	});
</script>

<div class="branching-question" data-block-id={blockId}>
	<h3 class="question-text">{blockData.question || 'Question'}</h3>
	<div class="answer-buttons">
		<button
			class="answer-btn yes-btn"
			class:selected={selectedAnswer === 'yes'}
			onclick={() => handleAnswer('yes')}
		>
			{blockData.yesLabel || 'Yes'}
		</button>
		<button
			class="answer-btn no-btn"
			class:selected={selectedAnswer === 'no'}
			onclick={() => handleAnswer('no')}
		>
			{blockData.noLabel || 'No'}
		</button>
	</div>
	{#if selectedAnswer}
		<p class="selected-answer">
			You selected: <strong>{selectedAnswer === 'yes' ? (blockData.yesLabel || 'Yes') : (blockData.noLabel || 'No')}</strong>
		</p>
	{/if}
</div>

<style>
	.branching-question {
		padding: 2rem;
		background: white;
		border-radius: 12px;
		border: 2px solid #0366d6;
		text-align: center;
		margin: 1rem 0;
	}

	.question-text {
		font-size: 1.5rem;
		font-weight: 600;
		color: #24292e;
		margin: 0 0 1.5rem 0;
	}

	.answer-buttons {
		display: flex;
		gap: 1rem;
		justify-content: center;
	}

	.answer-btn {
		padding: 1rem 2rem;
		border: 2px solid #d1d5da;
		border-radius: 8px;
		background: white;
		font-size: 1.125rem;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.2s;
		min-width: 120px;
	}

	.answer-btn:hover {
		border-color: #0366d6;
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(3, 102, 214, 0.2);
	}

	.yes-btn.selected {
		background: #28a745;
		color: white;
		border-color: #28a745;
	}

	.no-btn.selected {
		background: #d73a49;
		color: white;
		border-color: #d73a49;
	}

	.selected-answer {
		margin-top: 1rem;
		color: #586069;
		font-size: 0.875rem;
	}

	.selected-answer strong {
		color: #24292e;
	}
</style>
