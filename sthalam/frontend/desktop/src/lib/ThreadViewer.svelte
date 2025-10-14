<script lang="ts">
	import { marked } from 'marked';
	import DOMPurify from 'dompurify';

	interface Comment {
		id: string;
		author: string;
		content: string;
		timestamp: number;
	}

	interface Props {
		block: any;
		onUpdate: (updates: any) => void;
	}

	let { block, onUpdate }: Props = $props();

	// Local state for new comment
	let newCommentText = $state('');

	// Render main post content (owner's content - NOT sanitized, trust model)
	const mainPostHtml = $derived(() => {
		if (!block) return '';
		try {
			if (block.mode === 'html') {
				return block.content || '';
			} else {
				// Default to markdown
				return marked.parse(block.content || '');
			}
		} catch (error) {
			return '<p>Error rendering post</p>';
		}
	});

	// Get comments array (initialize if doesn't exist)
	const comments = $derived<Comment[]>(() => {
		return block?.comments || [];
	});

	// Sanitize comment HTML (viewer comments - MUST sanitize)
	function sanitizeComment(html: string): string {
		return DOMPurify.sanitize(html, {
			ALLOWED_TAGS: ['p', 'b', 'i', 'em', 'strong', 'a', 'ul', 'ol', 'li', 'br', 'code', 'pre', 'blockquote', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6'],
			ALLOWED_ATTR: ['href', 'target', 'rel'],
			ALLOW_DATA_ATTR: false
		});
	}

	// Add new comment
	function addComment() {
		const trimmed = newCommentText.trim();
		if (!trimmed) return;

		// Convert markdown to HTML
		let commentHtml = '';
		try {
			commentHtml = marked.parse(trimmed);
		} catch (error) {
			commentHtml = `<p>${trimmed}</p>`;
		}

		// Sanitize the HTML
		const sanitizedHtml = sanitizeComment(commentHtml);

		// Create new comment object
		const newComment: Comment = {
			id: `comment-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
			author: 'Anonymous', // TODO: Get from user info
			content: sanitizedHtml,
			timestamp: Date.now()
		};

		// Update block with new comment
		const updatedComments = [...comments(), newComment];
		onUpdate({ comments: updatedComments });

		// Clear input
		newCommentText = '';
	}

	// Format timestamp
	function formatTimestamp(timestamp: number): string {
		const date = new Date(timestamp);
		const now = Date.now();
		const diff = now - timestamp;

		// Less than 1 minute
		if (diff < 60000) return 'Just now';
		// Less than 1 hour
		if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
		// Less than 1 day
		if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
		// Less than 1 week
		if (diff < 604800000) return `${Math.floor(diff / 86400000)}d ago`;

		// Format as date
		return date.toLocaleDateString();
	}
</script>

<div class="thread-viewer">
	<!-- Main Post -->
	<div class="main-post">
		{#if block?.css}
			<style>
				{block.css}
			</style>
		{/if}
		<div class="post-content">
			{@html mainPostHtml()}
		</div>
	</div>

	<!-- Comments Section -->
	<div class="comments-section">
		<div class="comments-header">
			<h3>Comments ({comments().length})</h3>
		</div>

		<!-- Existing Comments -->
		<div class="comments-list">
			{#each comments() as comment (comment.id)}
				<div class="comment">
					<div class="comment-header">
						<span class="comment-author">{comment.author}</span>
						<span class="comment-time">{formatTimestamp(comment.timestamp)}</span>
					</div>
					<div class="comment-content">
						{@html comment.content}
					</div>
				</div>
			{/each}

			{#if comments().length === 0}
				<div class="no-comments">
					<p>No comments yet. Be the first to comment!</p>
				</div>
			{/if}
		</div>

		<!-- Add Comment Form -->
		<div class="add-comment">
			<textarea
				bind:value={newCommentText}
				placeholder="Write a comment (Markdown supported)...&#10;&#10;**bold**, *italic*, [link](url), etc."
				rows="3"
			></textarea>
			<button onclick={addComment} disabled={!newCommentText.trim()}>
				Post Comment
			</button>
		</div>
	</div>
</div>

<style>
	.thread-viewer {
		display: flex;
		flex-direction: column;
		gap: 2rem;
		width: 100%;
	}

	.main-post {
		background: #ffffff;
		border-radius: 8px;
		border: 1px solid #e1e4e8;
		padding: 1.5rem;
	}

	.post-content {
		color: #24292e;
		line-height: 1.6;
	}

	.post-content :global(h1) {
		font-size: 2rem;
		margin-bottom: 1rem;
		font-weight: 600;
		color: #24292e;
	}

	.post-content :global(h2) {
		font-size: 1.5rem;
		margin-bottom: 0.75rem;
		font-weight: 600;
		color: #24292e;
	}

	.post-content :global(p) {
		margin-bottom: 1rem;
	}

	.post-content :global(a) {
		color: #0366d6;
		text-decoration: none;
	}

	.post-content :global(a:hover) {
		text-decoration: underline;
	}

	.comments-section {
		background: #f6f8fa;
		border-radius: 8px;
		padding: 1.5rem;
	}

	.comments-header {
		margin-bottom: 1.5rem;
		padding-bottom: 0.75rem;
		border-bottom: 2px solid #e1e4e8;
	}

	.comments-header h3 {
		margin: 0;
		font-size: 1.25rem;
		color: #24292e;
		font-weight: 600;
	}

	.comments-list {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		margin-bottom: 1.5rem;
	}

	.comment {
		background: #ffffff;
		border: 1px solid #e1e4e8;
		border-radius: 6px;
		padding: 1rem;
	}

	.comment-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 0.75rem;
	}

	.comment-author {
		font-weight: 600;
		color: #24292e;
		font-size: 0.875rem;
	}

	.comment-time {
		font-size: 0.75rem;
		color: #586069;
	}

	.comment-content {
		color: #24292e;
		line-height: 1.5;
		font-size: 0.875rem;
	}

	.comment-content :global(p) {
		margin: 0.5rem 0;
	}

	.comment-content :global(p:first-child) {
		margin-top: 0;
	}

	.comment-content :global(p:last-child) {
		margin-bottom: 0;
	}

	.comment-content :global(a) {
		color: #0366d6;
		text-decoration: none;
	}

	.comment-content :global(a:hover) {
		text-decoration: underline;
	}

	.comment-content :global(code) {
		background: #f6f8fa;
		padding: 0.2em 0.4em;
		border-radius: 3px;
		font-family: monospace;
		font-size: 85%;
	}

	.no-comments {
		text-align: center;
		padding: 2rem;
		color: #586069;
	}

	.no-comments p {
		margin: 0;
		font-style: italic;
	}

	.add-comment {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.add-comment textarea {
		width: 100%;
		padding: 0.75rem;
		border: 1px solid #e1e4e8;
		border-radius: 6px;
		font-family: inherit;
		font-size: 0.875rem;
		line-height: 1.5;
		resize: vertical;
		min-height: 80px;
	}

	.add-comment textarea:focus {
		outline: none;
		border-color: #0366d6;
		box-shadow: 0 0 0 3px rgba(3, 102, 214, 0.1);
	}

	.add-comment button {
		align-self: flex-end;
		padding: 0.5rem 1rem;
		background: #0366d6;
		color: white;
		border: none;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: background 0.2s;
	}

	.add-comment button:hover:not(:disabled) {
		background: #0256c7;
	}

	.add-comment button:disabled {
		background: #94a3b8;
		cursor: not-allowed;
		opacity: 0.6;
	}
</style>
