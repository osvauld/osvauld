<script lang="ts">
	import { marked } from 'marked';

	type Props = {
		post: any; // Block object
		onUpdate: (updates: any) => void;
	};

	let { post, onUpdate }: Props = $props();

	let editMode = $state<'markdown' | 'html' | 'preview'>(post?.mode || 'markdown');

	// Sync edit mode when post changes
	$effect(() => {
		if (post?.mode) {
			editMode = post.mode;
		}
	});

	function handleInput(e: Event) {
		const target = e.target as HTMLTextAreaElement;
		onUpdate({ content: target.value });
	}

	function handleCssInput(e: Event) {
		const target = e.target as HTMLTextAreaElement;
		onUpdate({ css: target.value });
	}

	function handleModeChange(newMode: 'markdown' | 'html' | 'preview') {
		editMode = newMode;
		if (newMode !== 'preview') {
			onUpdate({ mode: newMode });
		}
	}

	// Parse markdown to HTML for preview
	const previewHtml = $derived(() => {
		if (!post) return '';
		if (editMode === 'html' || post.mode === 'html') {
			return post.content || '';
		} else {
			try {
				return marked.parse(post.content || '');
			} catch (error) {
				return '<p>Error parsing markdown</p>';
			}
		}
	});
</script>

<div class="thread-editor">
	<div class="editor-header">
		<h2>Main Thread Post</h2>
		<div class="mode-tabs">
			<button
				class="tab"
				class:active={editMode === 'markdown'}
				onclick={() => handleModeChange('markdown')}
			>
				📝 Markdown
			</button>
			<button
				class="tab"
				class:active={editMode === 'html'}
				onclick={() => handleModeChange('html')}
			>
				🌐 HTML/CSS
			</button>
			<button
				class="tab"
				class:active={editMode === 'preview'}
				onclick={() => handleModeChange('preview')}
			>
				👁️ Preview
			</button>
		</div>
	</div>

	<div class="editor-body">
		{#if editMode === 'markdown'}
			<div class="editor-section">
				<textarea
					class="content-input"
					placeholder="Write your thread post in markdown...&#10;&#10;# Heading&#10;**Bold** and *italic*&#10;- List items&#10;[Links](url)&#10;&#10;etc."
					value={post?.content || ''}
					oninput={handleInput}
				></textarea>
			</div>
		{:else if editMode === 'html'}
			<div class="editor-section split">
				<div class="input-half">
					<label>HTML</label>
					<textarea
						class="content-input html-input"
						placeholder="&lt;div&gt;&#10;  &lt;h1&gt;Your HTML here&lt;/h1&gt;&#10;  &lt;p&gt;Rich content...&lt;/p&gt;&#10;&lt;/div&gt;"
						value={post?.content || ''}
						oninput={handleInput}
					></textarea>
				</div>
				<div class="input-half">
					<label>CSS</label>
					<textarea
						class="content-input css-input"
						placeholder="/* Custom CSS */&#10;.my-class &#123;&#10;  color: #667eea;&#10;  font-size: 18px;&#10;&#125;"
						value={post?.css || ''}
						oninput={handleCssInput}
					></textarea>
				</div>
			</div>
		{:else if editMode === 'preview'}
			<div class="preview-section">
				<style>
					{post?.css || ''}
				</style>
				<div class="preview-content">
					{@html previewHtml()}
				</div>
			</div>
		{/if}
	</div>

	<div class="editor-footer">
		<div class="help-text">
			{#if editMode === 'markdown'}
				<span>💡 Supports standard Markdown syntax</span>
			{:else if editMode === 'html'}
				<span>💡 Write HTML and CSS directly - viewers will see the rendered result</span>
			{:else}
				<span>💡 This is how viewers will see your post</span>
			{/if}
		</div>
	</div>
</div>

<style>
	.thread-editor {
		background: #161b22;
		border-radius: 8px;
		border: 1px solid #30363d;
		overflow: hidden;
		margin-bottom: 2rem;
	}

	.editor-header {
		padding: 1.5rem;
		border-bottom: 1px solid #30363d;
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.editor-header h2 {
		margin: 0;
		color: #c9d1d9;
		font-size: 1.25rem;
		font-weight: 600;
	}

	.mode-tabs {
		display: flex;
		gap: 0.5rem;
	}

	.tab {
		padding: 0.5rem 1rem;
		background: #21262d;
		border: 1px solid #30363d;
		border-radius: 6px;
		color: #8b949e;
		cursor: pointer;
		font-size: 0.875rem;
		transition: all 0.2s;
	}

	.tab:hover {
		background: #30363d;
		color: #c9d1d9;
	}

	.tab.active {
		background: #667eea;
		border-color: #667eea;
		color: white;
	}

	.editor-body {
		min-height: 400px;
	}

	.editor-section {
		padding: 1.5rem;
	}

	.editor-section.split {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 1rem;
		padding: 1.5rem;
	}

	.input-half {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.input-half label {
		color: #8b949e;
		font-size: 0.875rem;
		font-weight: 500;
	}

	.content-input {
		width: 100%;
		min-height: 350px;
		background: #0d1117;
		border: 1px solid #30363d;
		border-radius: 6px;
		color: #c9d1d9;
		padding: 1rem;
		font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Fira Code', 'Consolas', monospace;
		font-size: 0.875rem;
		line-height: 1.6;
		resize: vertical;
	}

	.content-input::placeholder {
		color: #484f58;
	}

	.content-input:focus {
		outline: none;
		border-color: #667eea;
	}

	.html-input,
	.css-input {
		min-height: 300px;
	}

	.preview-section {
		padding: 1.5rem;
		min-height: 350px;
		background: #0d1117;
	}

	.preview-content {
		color: #c9d1d9;
		line-height: 1.6;
	}

	.preview-content :global(h1) {
		color: #c9d1d9;
		font-size: 2rem;
		margin-bottom: 1rem;
		font-weight: 600;
	}

	.preview-content :global(h2) {
		color: #c9d1d9;
		font-size: 1.5rem;
		margin-bottom: 0.75rem;
		font-weight: 600;
	}

	.preview-content :global(h3) {
		color: #c9d1d9;
		font-size: 1.25rem;
		margin-bottom: 0.5rem;
		font-weight: 600;
	}

	.preview-content :global(p) {
		margin-bottom: 1rem;
	}

	.preview-content :global(ul),
	.preview-content :global(ol) {
		margin-left: 1.5rem;
		margin-bottom: 1rem;
	}

	.preview-content :global(code) {
		background: #161b22;
		padding: 0.2rem 0.4rem;
		border-radius: 3px;
		font-family: 'SF Mono', monospace;
		font-size: 0.875rem;
	}

	.preview-content :global(pre) {
		background: #161b22;
		padding: 1rem;
		border-radius: 6px;
		overflow-x: auto;
		margin-bottom: 1rem;
	}

	.preview-content :global(a) {
		color: #667eea;
		text-decoration: none;
	}

	.preview-content :global(a:hover) {
		text-decoration: underline;
	}

	.preview-content :global(blockquote) {
		border-left: 3px solid #30363d;
		padding-left: 1rem;
		color: #8b949e;
		margin: 1rem 0;
	}

	.editor-footer {
		padding: 1rem 1.5rem;
		border-top: 1px solid #30363d;
		background: #0d1117;
	}

	.help-text {
		color: #8b949e;
		font-size: 0.875rem;
	}

	.help-text span {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
</style>
