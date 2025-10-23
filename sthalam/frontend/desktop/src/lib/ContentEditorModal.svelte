<script lang="ts">
	interface Props {
		isOpen: boolean;
		blockId: string;
		content: string;
		blockType: string;
		onSave: (blockId: string, newContent: string) => void;
		onCancel: () => void;
	}

	let { isOpen, blockId, content, blockType, onSave, onCancel }: Props = $props();

	let editedContent = $state(content);
	let textareaRef: HTMLTextAreaElement | null = $state(null);

	// Reset edited content when modal opens or content changes
	$effect(() => {
		if (isOpen) {
			editedContent = content;
			// Focus textarea after a small delay to ensure it's rendered
			setTimeout(() => {
				textareaRef?.focus();
			}, 100);
		}
	});

	// Derived stats
	const charCount = $derived(editedContent.length);
	const lineCount = $derived(editedContent.split('\n').length);
	const wordCount = $derived(editedContent.trim() === '' ? 0 : editedContent.trim().split(/\s+/).length);

	function handleSave() {
		onSave(blockId, editedContent);
	}

	function handleCancel() {
		editedContent = content; // Reset to original
		onCancel();
	}

	function handleKeydown(e: KeyboardEvent) {
		// Save on Cmd/Ctrl + Enter
		if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
			e.preventDefault();
			handleSave();
		}
		// Cancel on Escape
		if (e.key === 'Escape') {
			e.preventDefault();
			handleCancel();
		}
	}

	// Get modal title based on block type
	const modalTitle = $derived.by(() => {
		switch (blockType) {
			case 'heading':
				return '✏️ Edit Heading';
			case 'text':
				return '✏️ Edit Text';
			case 'markdown-text':
				return '✏️ Edit Markdown';
			default:
				return '✏️ Edit Content';
		}
	});
</script>

{#if isOpen}
	<div class="modal-overlay" onclick={handleCancel}>
		<div class="modal-container" onclick={(e) => e.stopPropagation()}>
			<!-- Header -->
			<div class="modal-header">
				<h2>{modalTitle}</h2>
				<button class="close-btn" onclick={handleCancel} title="Close (Esc)">
					✕
				</button>
			</div>

			<!-- Editor -->
			<div class="modal-body">
				<textarea
					bind:this={textareaRef}
					bind:value={editedContent}
					onkeydown={handleKeydown}
					placeholder={blockType === 'markdown-text'
						? 'Enter markdown content...\n\n## Heading\n- List item\n**Bold** _Italic_'
						: blockType === 'heading'
						? 'Enter heading text...'
						: 'Enter text content...'}
					spellcheck="true"
				></textarea>
			</div>

			<!-- Footer -->
			<div class="modal-footer">
				<div class="stats">
					<span class="stat">
						<span class="stat-label">Characters:</span>
						<span class="stat-value">{charCount}</span>
					</span>
					<span class="stat">
						<span class="stat-label">Words:</span>
						<span class="stat-value">{wordCount}</span>
					</span>
					<span class="stat">
						<span class="stat-label">Lines:</span>
						<span class="stat-value">{lineCount}</span>
					</span>
				</div>

				<div class="actions">
					<button class="cancel-btn" onclick={handleCancel}>
						Cancel <span class="shortcut">Esc</span>
					</button>
					<button class="save-btn" onclick={handleSave}>
						Save <span class="shortcut">⌘ + Enter</span>
					</button>
				</div>
			</div>

			{#if blockType === 'markdown-text'}
				<div class="markdown-hint">
					💡 <strong>Markdown supported:</strong> **bold**, _italic_, # headings, - lists,
					> quotes, `code`, ```code blocks```
				</div>
			{/if}
		</div>
	</div>
{/if}

<style>
	.modal-overlay {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: rgba(0, 0, 0, 0.8);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 10000;
		padding: 2rem;
		backdrop-filter: blur(4px);
	}

	.modal-container {
		background: #0d0e13;
		border: 1px solid #30363d;
		border-radius: 12px;
		width: 90%;
		max-width: 1200px;
		height: 85vh;
		display: flex;
		flex-direction: column;
		box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 1.5rem;
		border-bottom: 1px solid #30363d;
		background: #161b22;
		border-radius: 12px 12px 0 0;
	}

	.modal-header h2 {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 600;
		color: #c9d1d9;
	}

	.close-btn {
		width: 32px;
		height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: 1px solid #30363d;
		border-radius: 6px;
		color: #8b949e;
		font-size: 1.25rem;
		cursor: pointer;
		transition: all 0.2s;
	}

	.close-btn:hover {
		background: #f38ba8;
		color: white;
		border-color: #f38ba8;
		transform: scale(1.1);
	}

	.modal-body {
		flex: 1;
		padding: 1.5rem;
		overflow: hidden;
		display: flex;
	}

	textarea {
		flex: 1;
		width: 100%;
		padding: 1.5rem;
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 8px;
		color: #c9d1d9;
		font-size: 1rem;
		line-height: 1.6;
		font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Fira Code', 'Courier New', monospace;
		resize: none;
		outline: none;
		transition: border-color 0.2s;
	}

	textarea:focus {
		border-color: #89b4fa;
		box-shadow: 0 0 0 3px rgba(137, 180, 250, 0.1);
	}

	textarea::placeholder {
		color: #6e7681;
		opacity: 1;
	}

	.modal-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 1.5rem;
		border-top: 1px solid #30363d;
		background: #161b22;
		border-radius: 0 0 12px 12px;
	}

	.stats {
		display: flex;
		gap: 1.5rem;
	}

	.stat {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.stat-label {
		font-size: 0.75rem;
		color: #8b949e;
		font-weight: 500;
	}

	.stat-value {
		font-size: 0.875rem;
		color: #89b4fa;
		font-weight: 600;
		font-family: monospace;
	}

	.actions {
		display: flex;
		gap: 0.75rem;
	}

	.cancel-btn,
	.save-btn {
		padding: 0.75rem 1.5rem;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.2s;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.cancel-btn {
		background: transparent;
		border: 1px solid #30363d;
		color: #c9d1d9;
	}

	.cancel-btn:hover {
		background: #161b22;
		border-color: #8b949e;
	}

	.save-btn {
		background: #89b4fa;
		border: none;
		color: #1e1e2e;
	}

	.save-btn:hover {
		background: #74c7ec;
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(137, 180, 250, 0.4);
	}

	.shortcut {
		font-size: 0.75rem;
		opacity: 0.7;
		font-family: monospace;
	}

	.markdown-hint {
		padding: 0.75rem 1.5rem;
		background: rgba(137, 180, 250, 0.1);
		border-top: 1px solid rgba(137, 180, 250, 0.2);
		font-size: 0.75rem;
		color: #89b4fa;
		line-height: 1.5;
	}

	.markdown-hint strong {
		color: #c9d1d9;
	}
</style>
