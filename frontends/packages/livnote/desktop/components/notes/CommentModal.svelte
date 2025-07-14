<script lang="ts">
	import { notesInstance } from "./notes";
	// Props
	interface Props {
		isVisible: boolean;
		selectedText?: string;
		onSave: (content: string) => void;
		onCancel: () => void;
	}

	const { isVisible, selectedText = "", onSave, onCancel }: Props = $props();

	// State
	let commentText = $state("");
	let textareaRef = $state<HTMLTextAreaElement>();

	function sanitize(text: string): string {
		const div = document.createElement('div');
		div.textContent = text;
		return div.innerHTML;
	}

	function handleSave() {
		if (commentText.trim()) {
			onSave(sanitize(commentText.trim()));
			commentText = "";
		}
	}

	function handleCancel() {
		onCancel();
		commentText = "";
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Escape") {
			handleCancel();
		} else if (event.key === "Enter") {
			event.preventDefault();
			handleSave();
		}
	}

	// Focus textarea when modal becomes visible
	$effect(() => {
		if (isVisible && textareaRef) {
			setTimeout(() => textareaRef?.focus(), 100);
		}
	});
</script>

<style>
	.modal-overlay {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: rgba(0, 0, 0, 0.5);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		backdrop-filter: blur(2px);
	}

	.modal-content {
		background: #16171f;
		border: 1px solid #2a2b2f;
		border-radius: 8px;
		padding: 20px;
		width: 90%;
		max-width: 400px;
		box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
	}

	.modal-header {
		margin-bottom: 16px;
	}

	.modal-title {
		font-size: 16px;
		font-weight: 300;
		color: #fff;
		margin: 0 0 8px 0;
	}

	.selected-text {
		font-size: 12px;
		color: #85889c;
		font-style: italic;
		padding: 8px 12px;
		background: #1a1b23;
		border-left: 3px solid var(--color-livnotePink);
		margin-bottom: 16px;
	}

	.comment-textarea {
		width: 100%;
		min-height: 100px;
		padding: 12px;
		border: 1px solid #2a2b2f;
		border-radius: 6px;
		background: #1a1b23;
		color: #bfc0cc;
		font-size: 14px;
		line-height: 1.4;
		resize: vertical;
		margin-bottom: 16px;
	}

	.comment-textarea:focus {
		outline: none;
		border-color: var(--color-livnotePink);
		box-shadow: 0 0 0 2px rgba(255, 215, 0, 0.1);
	}

	.comment-textarea::placeholder {
		color: #85889c;
	}

	.modal-actions {
		display: flex;
		gap: 12px;
		justify-content: space-between;
		align-items: center;
	}

	.modal-buttons {
		display: flex;
		gap: 12px;
	}

	.modal-btn {
		padding: 8px 16px;
		border: none;
		border-radius: 4px;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.modal-btn.primary {
		background: var(--color-livnotePink);
		color: #16171f;
	}

	.modal-btn.primary:hover {
		background: var(--color-livnotePink);
	}



	.modal-btn.secondary {
		background: transparent;
		color: #85889c;
		border: 1px solid #2a2b2f;
	}

	.modal-btn.secondary:hover {
		background: #2a2b2f;
		color: #bfc0cc;
	}

	.shortcut-hint {
		font-size: 11px;
		color: #85889c;
		text-align: center;
		margin-top: 8px;
	}
</style>

{#if isVisible}
	<div class="modal-overlay"  role="presentation" onclick={handleCancel}>
		<div class="modal-content" role="dialog" aria-modal="true" aria-labelledby="modal-title" tabindex="0" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
			<div class="modal-header">
				<h3 class="modal-title">Add Comment</h3>
			</div>

			{#if selectedText}
				<div class="selected-text">
					"{selectedText}"
				</div>
			{/if}

			<textarea
				bind:this={textareaRef}
				bind:value={commentText}
				class="comment-textarea"
				placeholder="Write your comment..."
				autocapitalize="off"
				spellcheck="false"
				onkeydown={handleKeydown}
				maxlength="150"></textarea>

			<div class="modal-actions">
				<span class="shortcut-hint">{commentText.length}/150</span>
				<div class="modal-buttons">
					<button class="modal-btn secondary" onclick={handleCancel}>
						Cancel
					</button>
					<button
						class="modal-btn primary"
						onclick={handleSave}
						disabled={!commentText.trim()}>
						Add Comment
					</button>
				</div>
			</div>

			<div class="shortcut-hint">Press Enter to save, Esc to cancel</div>
		</div>
	</div>
{/if}

