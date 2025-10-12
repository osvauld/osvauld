<script lang="ts">
	import { marked } from 'marked';
	import CommentInput from './CommentInput.svelte';
	import Comment from './Comment.svelte';

	type Props = {
		comment: any;
		onReply: (parentId: string, content: string, mode: string, css?: string) => void;
		onUpdate: (commentId: string, updates: any) => void;
		onDelete: (commentId: string) => void;
		depth?: number;
	};

	let { comment, onReply, onUpdate, onDelete, depth = 0 }: Props = $props();

	let showReplyInput = $state(false);
	let isCollapsed = $state(false);

	// Parse comment content based on mode
	const renderedContent = $derived(() => {
		if (comment.mode === 'html') {
			return comment.content;
		} else {
			try {
				return marked.parse(comment.content || '');
			} catch (error) {
				return '<p>Error parsing content</p>';
			}
		}
	});

	function handleReply(content: string, mode: string, css?: string) {
		onReply(comment.id, content, mode, css);
		showReplyInput = false;
	}

	function toggleCollapse() {
		isCollapsed = !isCollapsed;
	}

	// Format timestamp
	function formatTime(timestamp: string): string {
		const date = new Date(timestamp);
		const now = new Date();
		const diff = now.getTime() - date.getTime();

		const minutes = Math.floor(diff / 60000);
		const hours = Math.floor(diff / 3600000);
		const days = Math.floor(diff / 86400000);

		if (minutes < 1) return 'just now';
		if (minutes < 60) return `${minutes}m ago`;
		if (hours < 24) return `${hours}h ago`;
		return `${days}d ago`;
	}

	const hasReplies = $derived(comment.replies && comment.replies.length > 0);
	const replyCount = $derived(comment.replies ? comment.replies.length : 0);
</script>

<div class="comment" class:collapsed={isCollapsed} style="--depth: {depth}">
	<div class="comment-line"></div>

	<div class="comment-header">
		<div class="author-info">
			<button class="collapse-btn" onclick={toggleCollapse}>
				{isCollapsed ? '+' : '−'}
			</button>
			<span class="author">{comment.author}</span>
			<span class="timestamp">{formatTime(comment.timestamp)}</span>
			{#if comment.mode === 'html'}
				<span class="badge">HTML</span>
			{/if}
		</div>
	</div>

	{#if !isCollapsed}
		<div class="comment-body">
			<style>
				{comment.css || ''}
			</style>
			<div class="comment-content">
				{@html renderedContent()}
			</div>
		</div>

		<div class="comment-actions">
			<button class="action-btn" onclick={() => showReplyInput = !showReplyInput}>
				💬 Reply
			</button>
			{#if hasReplies}
				<span class="reply-count">{replyCount} {replyCount === 1 ? 'reply' : 'replies'}</span>
			{/if}
		</div>

		{#if showReplyInput}
			<div class="reply-input">
				<CommentInput
					onSubmit={handleReply}
					placeholder="Write a reply..."
					onCancel={() => showReplyInput = false}
					showCancel={true}
				/>
			</div>
		{/if}

		<!-- Nested replies -->
		{#if hasReplies}
			<div class="replies">
				{#each comment.replies as reply}
					<Comment
						comment={reply}
						{onReply}
						{onUpdate}
						{onDelete}
						depth={depth + 1}
					/>
				{/each}
			</div>
		{/if}
	{:else}
		<div class="collapsed-info">
			<span>{replyCount} {replyCount === 1 ? 'reply' : 'replies'} hidden</span>
		</div>
	{/if}
</div>

<style>
	.comment {
		position: relative;
		margin-left: calc(var(--depth) * 2rem);
		margin-bottom: 0.5rem;
	}

	.comment-line {
		position: absolute;
		left: 0.625rem;
		top: 2rem;
		bottom: 0;
		width: 2px;
		background: #30363d;
		display: none;
	}

	.comment:has(.replies) .comment-line {
		display: block;
	}

	.comment-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.75rem 1rem;
		background: #161b22;
		border-left: 2px solid #30363d;
	}

	.author-info {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		font-size: 0.875rem;
	}

	.collapse-btn {
		width: 1.25rem;
		height: 1.25rem;
		border: 1px solid #30363d;
		background: #21262d;
		color: #8b949e;
		border-radius: 3px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 0.875rem;
		transition: all 0.2s;
	}

	.collapse-btn:hover {
		background: #30363d;
		color: #c9d1d9;
	}

	.author {
		font-weight: 600;
		color: #c9d1d9;
	}

	.timestamp {
		color: #8b949e;
		font-size: 0.75rem;
	}

	.badge {
		padding: 0.125rem 0.5rem;
		background: #667eea;
		color: white;
		border-radius: 3px;
		font-size: 0.625rem;
		font-weight: 600;
		text-transform: uppercase;
	}

	.comment-body {
		padding: 0.75rem 1rem;
		padding-left: 3rem;
		background: #0d1117;
		border-left: 2px solid #30363d;
	}

	.comment-content {
		color: #c9d1d9;
		line-height: 1.6;
		font-size: 0.875rem;
	}

	.comment-content :global(h1),
	.comment-content :global(h2),
	.comment-content :global(h3) {
		color: #c9d1d9;
		margin-bottom: 0.5rem;
	}

	.comment-content :global(h1) {
		font-size: 1.5rem;
	}

	.comment-content :global(h2) {
		font-size: 1.25rem;
	}

	.comment-content :global(h3) {
		font-size: 1.125rem;
	}

	.comment-content :global(p) {
		margin-bottom: 0.75rem;
	}

	.comment-content :global(ul),
	.comment-content :global(ol) {
		margin-left: 1.5rem;
		margin-bottom: 0.75rem;
	}

	.comment-content :global(code) {
		background: #161b22;
		padding: 0.2rem 0.4rem;
		border-radius: 3px;
		font-family: 'SF Mono', monospace;
		font-size: 0.875rem;
	}

	.comment-content :global(pre) {
		background: #161b22;
		padding: 0.75rem;
		border-radius: 6px;
		overflow-x: auto;
		margin-bottom: 0.75rem;
	}

	.comment-content :global(a) {
		color: #667eea;
		text-decoration: none;
	}

	.comment-content :global(a:hover) {
		text-decoration: underline;
	}

	.comment-content :global(blockquote) {
		border-left: 3px solid #30363d;
		padding-left: 1rem;
		color: #8b949e;
		margin: 0.75rem 0;
	}

	.comment-actions {
		padding: 0.5rem 1rem;
		padding-left: 3rem;
		background: #0d1117;
		border-left: 2px solid #30363d;
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	.action-btn {
		padding: 0.25rem 0.75rem;
		background: transparent;
		border: 1px solid #30363d;
		color: #8b949e;
		border-radius: 4px;
		cursor: pointer;
		font-size: 0.75rem;
		transition: all 0.2s;
	}

	.action-btn:hover {
		background: #21262d;
		color: #c9d1d9;
	}

	.reply-count {
		color: #8b949e;
		font-size: 0.75rem;
	}

	.reply-input {
		padding: 0.75rem 1rem;
		padding-left: 3rem;
		background: #0d1117;
		border-left: 2px solid #30363d;
	}

	.replies {
		margin-top: 0.5rem;
	}

	.collapsed-info {
		padding: 0.5rem 1rem;
		padding-left: 3rem;
		background: #0d1117;
		border-left: 2px solid #30363d;
		color: #8b949e;
		font-size: 0.75rem;
	}

	.collapsed .comment-body,
	.collapsed .comment-actions,
	.collapsed .reply-input,
	.collapsed .replies {
		display: none;
	}
</style>
