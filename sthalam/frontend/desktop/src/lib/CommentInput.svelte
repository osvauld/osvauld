<script lang="ts">
	import { marked } from 'marked';

	type Props = {
		onSubmit: (content: string, mode: string, css?: string) => void;
		placeholder?: string;
		onCancel?: () => void;
		showCancel?: boolean;
	};

	let { onSubmit, placeholder = "Write a comment...", onCancel, showCancel = false }: Props = $props();

	let content = $state("");
	let customCss = $state("");
	let mode = $state<'markdown' | 'html' | 'preview'>('markdown');

	function handleSubmit() {
		if (!content.trim()) return;
		onSubmit(content, mode === 'html' ? 'html' : 'markdown', customCss);
		// Reset form
		content = "";
		customCss = "";
		mode = 'markdown';
	}

	function handleCancel() {
		content = "";
		customCss = "";
		mode = 'markdown';
		if (onCancel) onCancel();
	}

	// Parse markdown to HTML for preview
	const previewHtml = $derived(() => {
		if (mode === 'html') {
			return content;
		} else if (mode === 'markdown') {
			try {
				return marked.parse(content || '');
			} catch (error) {
				return '<p>Error parsing markdown</p>';
			}
		}
		return content;
	});
</script>

<div class="comment-input">
	<div class="input-header">
		<div class="mode-tabs">
			<button
				class="tab"
				class:active={mode === 'markdown'}
				onclick={() => mode = 'markdown'}
			>
				📝 Markdown
			</button>
			<button
				class="tab"
				class:active={mode === 'html'}
				onclick={() => mode = 'html'}
			>
				🌐 HTML/CSS
			</button>
			<button
				class="tab"
				class:active={mode === 'preview'}
				onclick={() => mode = 'preview'}
			>
				👁️ Preview
			</button>
		</div>
	</div>

	<div class="input-body">
		{#if mode === 'markdown'}
			<textarea
				class="textarea"
				placeholder={placeholder}
				bind:value={content}
			></textarea>
		{:else if mode === 'html'}
			<div class="html-editor">
				<div class="editor-column">
					<label>HTML</label>
					<textarea
						class="textarea html-textarea"
						placeholder="&lt;div&gt;Your HTML here...&lt;/div&gt;"
						bind:value={content}
					></textarea>
				</div>
				<div class="editor-column">
					<label>CSS</label>
					<textarea
						class="textarea css-textarea"
						placeholder=".custom &#123; color: #667eea; &#125;"
						bind:value={customCss}
					></textarea>
				</div>
			</div>
		{:else if mode === 'preview'}
			<div class="preview-area">
				<style>
					{customCss}
				</style>
				{#if content.trim()}
					<div class="preview-content">
						{@html previewHtml()}
					</div>
				{:else}
					<div class="preview-empty">
						<p>Nothing to preview yet...</p>
					</div>
				{/if}
			</div>
		{/if}
	</div>

	<div class="input-footer">
		<div class="help-text">
			{#if mode === 'markdown'}
				<span>Markdown supported: **bold**, *italic*, [links](url), etc.</span>
			{:else if mode === 'html'}
				<span>Write custom HTML and CSS</span>
			{:else}
				<span>Preview your comment before posting</span>
			{/if}
		</div>
		<div class="actions">
			{#if showCancel}
				<button class="btn btn-cancel" onclick={handleCancel}>
					Cancel
				</button>
			{/if}
			<button
				class="btn btn-submit"
				onclick={handleSubmit}
				disabled={!content.trim()}
			>
				Post Comment
			</button>
		</div>
	</div>
</div>

<style>
	.comment-input {
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 6px;
		overflow: hidden;
	}

	.input-header {
		padding: 0.75rem 1rem;
		border-bottom: 1px solid #30363d;
		background: #0d1117;
	}

	.mode-tabs {
		display: flex;
		gap: 0.5rem;
	}

	.tab {
		padding: 0.375rem 0.75rem;
		background: transparent;
		border: 1px solid #30363d;
		border-radius: 4px;
		color: #8b949e;
		cursor: pointer;
		font-size: 0.75rem;
		transition: all 0.2s;
	}

	.tab:hover {
		background: #21262d;
		color: #c9d1d9;
	}

	.tab.active {
		background: #667eea;
		border-color: #667eea;
		color: white;
	}

	.input-body {
		padding: 0;
	}

	.textarea {
		width: 100%;
		min-height: 120px;
		background: #0d1117;
		border: none;
		color: #c9d1d9;
		padding: 1rem;
		font-family: 'SF Mono', 'Monaco', monospace;
		font-size: 0.875rem;
		line-height: 1.6;
		resize: vertical;
	}

	.textarea::placeholder {
		color: #484f58;
	}

	.textarea:focus {
		outline: none;
	}

	.html-editor {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 1px;
		background: #30363d;
	}

	.editor-column {
		background: #0d1117;
		display: flex;
		flex-direction: column;
	}

	.editor-column label {
		padding: 0.5rem 1rem;
		color: #8b949e;
		font-size: 0.75rem;
		font-weight: 500;
		background: #161b22;
		border-bottom: 1px solid #30363d;
	}

	.html-textarea,
	.css-textarea {
		min-height: 150px;
	}

	.preview-area {
		min-height: 120px;
		padding: 1rem;
		background: #0d1117;
	}

	.preview-content {
		color: #c9d1d9;
		line-height: 1.6;
	}

	.preview-content :global(h1),
	.preview-content :global(h2),
	.preview-content :global(h3) {
		color: #c9d1d9;
		margin-bottom: 0.5rem;
	}

	.preview-content :global(p) {
		margin-bottom: 0.5rem;
	}

	.preview-content :global(code) {
		background: #161b22;
		padding: 0.2rem 0.4rem;
		border-radius: 3px;
		font-family: 'SF Mono', monospace;
		font-size: 0.875rem;
	}

	.preview-content :global(a) {
		color: #667eea;
		text-decoration: none;
	}

	.preview-empty {
		color: #8b949e;
		text-align: center;
		padding: 2rem;
	}

	.preview-empty p {
		margin: 0;
		font-size: 0.875rem;
	}

	.input-footer {
		padding: 0.75rem 1rem;
		border-top: 1px solid #30363d;
		background: #161b22;
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.help-text {
		color: #8b949e;
		font-size: 0.75rem;
	}

	.actions {
		display: flex;
		gap: 0.5rem;
	}

	.btn {
		padding: 0.5rem 1rem;
		border-radius: 6px;
		font-size: 0.875rem;
		cursor: pointer;
		transition: all 0.2s;
		border: 1px solid;
	}

	.btn-cancel {
		background: transparent;
		border-color: #30363d;
		color: #8b949e;
	}

	.btn-cancel:hover {
		background: #21262d;
		color: #c9d1d9;
	}

	.btn-submit {
		background: #667eea;
		border-color: #667eea;
		color: white;
	}

	.btn-submit:hover:not(:disabled) {
		background: #5568d3;
	}

	.btn-submit:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
