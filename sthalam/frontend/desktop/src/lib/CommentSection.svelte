<script lang="ts">
	import Comment from './Comment.svelte';
	import CommentInput from './CommentInput.svelte';

	type Props = {
		comments: any[];
		onAddComment: (comment: any) => void;
		onUpdateComment: (commentId: string, updates: any) => void;
		onDeleteComment: (commentId: string) => void;
	};

	let { comments, onAddComment, onUpdateComment, onDeleteComment }: Props = $props();

	function handleNewComment(content: string, mode: string, css?: string) {
		const newComment = {
			id: `comment-${Date.now()}`,
			content,
			mode, // 'markdown' | 'html'
			css: css || "",
			author: "Owner", // Will be replaced with actual user later
			timestamp: new Date().toISOString(),
			replies: []
		};
		onAddComment(newComment);
	}

	function handleReply(parentId: string, content: string, mode: string, css?: string) {
		const reply = {
			id: `comment-${Date.now()}`,
			content,
			mode,
			css: css || "",
			author: "Owner",
			timestamp: new Date().toISOString(),
			parentId,
			replies: []
		};

		// Find parent comment and add reply
		const updatedComments = addReplyToComment(comments, parentId, reply);
		// For now, we'll just add it as a top-level comment with parentId
		// In a real implementation, you'd update the parent's replies array
		onAddComment(reply);
	}

	function addReplyToComment(commentsList: any[], parentId: string, reply: any): any[] {
		return commentsList.map(comment => {
			if (comment.id === parentId) {
				return {
					...comment,
					replies: [...(comment.replies || []), reply]
				};
			}
			if (comment.replies && comment.replies.length > 0) {
				return {
					...comment,
					replies: addReplyToComment(comment.replies, parentId, reply)
				};
			}
			return comment;
		});
	}

	// Build a tree structure for nested comments
	const commentTree = $derived(() => {
		const tree: any[] = [];
		const commentMap = new Map();

		// First pass: create map of all comments
		comments.forEach(comment => {
			commentMap.set(comment.id, { ...comment, replies: [] });
		});

		// Second pass: build tree
		comments.forEach(comment => {
			const node = commentMap.get(comment.id);
			if (comment.parentId && commentMap.has(comment.parentId)) {
				const parent = commentMap.get(comment.parentId);
				parent.replies.push(node);
			} else {
				tree.push(node);
			}
		});

		return tree;
	});
</script>

<div class="comment-section">
	<div class="section-header">
		<h3>Comments ({comments.length})</h3>
	</div>

	<div class="new-comment">
		<CommentInput
			onSubmit={handleNewComment}
			placeholder="Add a comment to this thread..."
		/>
	</div>

	<div class="comments-list">
		{#each commentTree() as comment}
			<Comment
				{comment}
				onReply={handleReply}
				onUpdate={onUpdateComment}
				onDelete={onDeleteComment}
			/>
		{/each}

		{#if comments.length === 0}
			<div class="no-comments">
				<p>No comments yet. Be the first to comment!</p>
			</div>
		{/if}
	</div>
</div>

<style>
	.comment-section {
		background: #161b22;
		border-radius: 8px;
		border: 1px solid #30363d;
		overflow: hidden;
	}

	.section-header {
		padding: 1.5rem;
		border-bottom: 1px solid #30363d;
	}

	.section-header h3 {
		margin: 0;
		color: #c9d1d9;
		font-size: 1.125rem;
		font-weight: 600;
	}

	.new-comment {
		padding: 1.5rem;
		border-bottom: 1px solid #30363d;
		background: #0d1117;
	}

	.comments-list {
		padding: 1rem 0;
	}

	.no-comments {
		padding: 3rem 1.5rem;
		text-align: center;
		color: #8b949e;
	}

	.no-comments p {
		margin: 0;
		font-size: 0.875rem;
	}
</style>
