<script lang="ts">
	import type { CommentThread, Comment } from '../../types/notes.types';
	import { notesInstance } from './notes';
	import { ReplyIcon } from "@osvauld/password-manager-common";
	// Props
	interface Props {
		thread: CommentThread;
		isSelected?: boolean;
		isHighlighted?: boolean;
		onSelect: () => void;
		onResolve: (resolved: boolean) => void;
		onDelete: () => void;
	}
	
	const { thread, isSelected = false, isHighlighted = false, onSelect, onResolve, onDelete }: Props = $props();

	// State
	let isExpanded = $state(false);
	let replyText = $state('');
	let isAddingReply = $state(false);

	// Derived values
	const commentCount = $derived(thread.comments.length);
	const mainComment = $derived(thread.comments[0]);
	const replies = $derived(thread.comments.slice(1));
	const previewText = $derived(getPreviewText());

	function getPreviewText(): string {
		// Get the text that was commented on from the document position
		try {
			const editorDoc = notesInstance.getDoc().editorState?.doc;
			if (editorDoc && thread.position) {
				const slice = editorDoc.slice(thread.position.from, thread.position.to);
				const text = editorDoc.textBetween(thread.position.from, thread.position.to, ' ');
				return text.substring(0, 50) + (text.length > 50 ? '...' : '');
			}
		} catch (error) {
			console.warn('Could not get preview text:', error);
		}
		return 'Text excerpt';
	}

	function sanitize(text: string): string {
		const div = document.createElement('div');
		div.textContent = text;
		return div.innerHTML;
	}

	function handleToggleExpand() {
		isExpanded = !isExpanded;
		if (isExpanded && !isSelected) {
			onSelect();
		}
	}

	function handleThreadClick() {
		// Highlight corresponding text in editor
		const highlightTextEvent = new CustomEvent('highlight-comment-text', {
			detail: { threadId: thread.id, position: thread.position }
		});
		document.dispatchEvent(highlightTextEvent);
		
		// Also trigger the expand/select logic
		handleToggleExpand();
	}

	function handleAddReply() {
		const trimmedReply = replyText.trim();
		if (!trimmedReply) return;

		try {
			notesInstance.addCommentReply(thread.id, sanitize(trimmedReply));
			replyText = '';
			isAddingReply = false;
		} catch (error) {
			console.error('Error adding reply:', error);
		}
	}

	function formatTimestamp(timestamp: number): string {
		const date = new Date(timestamp);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffMins = Math.floor(diffMs / (1000 * 60));
		const diffHours = Math.floor(diffMins / 60);
		const diffDays = Math.floor(diffHours / 24);

		if (diffMins < 1) return 'Just now';
		if (diffMins < 60) return `${diffMins}m ago`;
		if (diffHours < 24) return `${diffHours}h ago`;
		if (diffDays < 7) return `${diffDays}d ago`;
		
		return date.toLocaleDateString();
	}
</script>

<style>
	.comment-thread {
		border-radius: 6px;
		overflow: hidden;
		transition: all 0.2s ease;
	}

	.comment-thread:hover {
		border-color: #3a3b44;
		background: #1a1b23;
	}

	.comment-thread.selected {
		/* box-shadow: 0 0 0 3px #8A86E5; */
	}

	.comment-thread.resolved {
		opacity: 0.7;
	}

	.comment-thread.thread-highlighted {
		animation: highlightPulse 0.5s ease-in-out;
		/* background: #23fd56; */
		/* box-shadow: 0 2px 12px 0 rgba(255, 215, 0, 0.18); */
	}

	.thread-header {
		padding: 12px;
		cursor: pointer;
		display: flex;
		align-items: flex-start;
		gap: 8px;
	
	}


	.thread-content {
		flex: 1;
		min-width: 0;
	}

	.thread-preview {
		font-size: 11px;
	  margin: 10px 0;
		font-style: italic;
		border-left: 2px solid var(--color-commentYellow);
		padding-left: 6px;
		display: flex;
		align-items: center;
		justify-content: start;
	
	}

	.thread-comment {
		font-size: 13px;
		color: #fff;
		line-height: 1.4;
		margin-bottom: 6px;
		letter-spacing: 0.02em;
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.thread-comment.expanded {
		white-space: normal;
		overflow: visible;
		text-overflow: clip;
		overflow-wrap: break-word;
	}

	.thread-meta {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 11px;
		color: #85889c;
	}

	.thread-author {
		font-weight: 500;
	}

	.thread-count {
		padding: 2px 6px;
		background: #2a2b2f;
		border-radius: 10px;
		font-size: 10px;
	}

	.thread-actions {
		display: flex;
		align-items: flex-start;
		gap: 4px;
		opacity: 0;
		transition: opacity 0.2s ease;
	}

	.comment-thread:hover .thread-actions {
		opacity: 1;
	}

	.action-btn {
		padding: 4px;
		border: none;
		background: transparent;
		color: #85889c;
		cursor: pointer;
		border-radius: 3px;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all 0.2s ease;
	}

	.action-btn:hover {
		background: #2a2b2f;
		color: #bfc0cc;
	}

	.action-btn.resolve {
		color: #4CAF50;
	}

	.action-btn.unresolve {
		color: #ff9800;
	}

	.action-btn.delete {
		color: #f44336;
	}

	.thread-details {
		padding: 4px 8px 8px 8px;
		border-top: 1px solid #2a2b2f;
	}



	.reply-list {
		margin-top: 12px;
	}

	.reply-item {
		margin-bottom: 12px;
		padding-left: 12px;
		border-left: 2px solid #2a2b2f;
	}

	.reply-content {
		font-size: 13px;
		color: #bfc0cc;
		line-height: 1.4;
		margin-bottom: 4px;
	}

	.reply-meta {
		font-size: 11px;
		color: #85889c;
	}

	.add-reply-form {
		margin-top: 12px;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.reply-input {
		width: 100%;
		padding: 8px;
		border: 1px solid #2a2b2f;
		border-radius: 4px;
		background: #16171f;
		color: #bfc0cc;
		font-size: 13px;
	}

	.reply-input:focus {
		outline: none;
		border-color: var(--color-livnotePink);
	}

	.reply-actions {
		display: flex;
		gap: 8px;
		justify-content: space-between;
		align-items: center;
	}

	.reply-btn {
		padding: 6px 12px;
		border: none;
		border-radius: 4px;
		font-size: 12px;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.reply-btn.primary {
		background: var(--color-livnotePink);
		color: #16171f;
	}

	.reply-btn.primary:hover {
		background: var(--color-livnotePink);
	}

	.reply-btn.secondary {
		background: transparent;
		color: #85889c;
		border: 1px solid #2a2b2f;
	}

	.reply-btn.secondary:hover {
		background: #2a2b2f;
		color: #bfc0cc;
	}

	.add-reply-trigger {
		width: 100%;
		padding: 8px;
		border: 1px dashed #2a2b2f;
		border-radius: 4px;
		background: transparent;
		color: #85889c;
		font-size: 12px;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.add-reply-trigger:hover {
		box-shadow: 0 2px 12px 0 rgba(255, 215, 0, 0.18);
	}

	@keyframes highlightPulse {
   50% { 
			background: transparent;
		}
	}
</style>

<div class="comment-thread" class:selected={isSelected} class:resolved={thread.resolved} class:thread-highlighted={isHighlighted}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="thread-header"  onclick={handleThreadClick}>
		<div class="thread-content">
			<div class="flex justify-start items-center gap-2">
				<span class="w-11 h-11 flex justify-center items-center  rounded-full text-commentThreadNameInitial border-2 border-collaboratorBorder">{mainComment.author.name.charAt(0).toUpperCase()}</span>
				<div class="flex flex-col items-start">
					<span class="capitalize text-white text-sm font-medium tracking-wider">{mainComment.author.name}</span>
					<span class="text-xs text-statusColor">{formatTimestamp(mainComment.timestamp)}</span>
				</div>
			</div>
			<div class="thread-preview text-statusColor">
				<span class="max-w-full overflow-hidden text-ellipsis whitespace-nowrap">"{previewText}"</span>
			</div>
			<div class="thread-comment" class:expanded={isExpanded}>
				<span >{mainComment.content}
				</span>
			</div>
			<div class="thread-meta">
				{#if commentCount > 1}
					<span class="thread-count">{commentCount} comments</span>
				{/if}
			</div>
		</div>
		
		<!-- <div class="thread-actions">
			{#if thread.resolved}
				<button 
					class="action-btn unresolve" 
					title="Mark as unresolved"
					onclick={(e) => { e.stopPropagation(); onResolve(false); }}
				>
					<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
						<path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm4.59-12.42L10 14.17l-2.59-2.58L6 13l4 4 8-8z"/>
					</svg>
				</button>
			{:else}
				<button 
					class="action-btn resolve" 
					title="Mark as resolved"
					onclick={(e) => { e.stopPropagation(); onResolve(true); }}
				>
					<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
						<path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
					</svg>
				</button>
			{/if}
			
			<button 
				class="action-btn delete" 
				title="Delete thread"
				onclick={(e) => { e.stopPropagation(); onDelete(); }}
			>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
					<path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/>
				</svg>
			</button>
		</div> -->
	</div>

	{#if isExpanded}
		<div class="thread-details">
			{#if replies.length > 0}
				<div class="reply-list">
					{#each replies as reply (reply.id)}
						<div class="reply-item">
							<div class="reply-content">{reply.content}</div>
							<div class="reply-meta">
								{reply.author.name} • {formatTimestamp(reply.timestamp)}
							</div>
						</div>
					{/each}
				</div>
			{/if}

			{#if isAddingReply}
				<div class="add-reply-form">
					<input
						class="reply-input"
						bind:value={replyText}
						placeholder="Add a reply..."
						autofocus
						maxlength="150"
						onkeydown={(e) => {
							if (e.key === 'Enter') {
								e.preventDefault();
								handleAddReply();
							}
						}}
					/>
					<div class="reply-actions">
						<span class="text-xs text-statusColor">{replyText.length}/150</span>
						<div class="flex items-center gap-2">
							<button
								class="reply-btn secondary"
								onclick={() => {
									isAddingReply = false;
									replyText = '';
								}}
							>
								Cancel
							</button>
							<button
								class="reply-btn primary"
								onclick={handleAddReply}
								disabled={!replyText.trim()}
							>
								Submit
							</button>
						</div>
					</div>
				</div>
			{:else}
				<div class="flex justify-end py-1">
					<button
						class="flex items-center gap-2 bg-livnotePink text-primarydark px-4 py-2 rounded-md text-sm  transition cursor-pointer"
						onclick={() => isAddingReply = true}
					>
						<ReplyIcon />
						Reply
					</button>
				</div>
			{/if}
		</div>
	{/if}
</div> 

<div class="w-full h-px bg-[#2a2b2f] my-0.5"></div>