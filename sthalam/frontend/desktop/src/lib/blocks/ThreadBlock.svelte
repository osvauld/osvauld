<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { marked } from 'marked';
	import DOMPurify from 'dompurify';
	import type { ThreadCommentsStore } from '../threadCommentsStore';
	import type { BlocksuiteStore } from '../blocksuiteStore';
	import { dataState } from '../../state';
	import { sendMessage } from '../../utils/helper';

	interface Props {
		blockId: string;
	}

	let { blockId }: Props = $props();

	let blocksuiteStore: BlocksuiteStore | null = null;
	let threadCommentsStore: ThreadCommentsStore | null = null;
	let blocksuiteUnsubscribe: (() => void) | null = null;
	let commentsUnsubscribe: (() => void) | null = null;

	// State for main post
	let mainPost = $state({
		content: '',
		mode: 'markdown',
		css: '',
		name: '',
		description: ''
	});

	// State for comments
	let comments = $state<Array<{
		id: string;
		author: string;
		userId?: string;
		content: string;
		timestamp: number;
	}>>([]);

	// Comment input
	let commentInput = $state('');

	// Collapse/expand state
	let isExpanded = $state(false);

	// Error state
	let commentError = $state<string | null>(null);

	// Handle blocksuite store ready
	function handleBlocksuiteStoreReady(event: CustomEvent) {
		const storeInstance = event.detail.blocksuiteStore;
		setupBlocksuiteSubscription(storeInstance);
	}

	function setupBlocksuiteSubscription(storeInstance: BlocksuiteStore) {
		// Clean up previous subscription
		if (blocksuiteUnsubscribe) {
			blocksuiteUnsubscribe();
			blocksuiteUnsubscribe = null;
		}

		blocksuiteStore = storeInstance;

		// Subscribe to updates
		blocksuiteUnsubscribe = blocksuiteStore.subscribe(() => {
			loadBlockData();
		});

		// Initial load
		loadBlockData();
	}

	function loadBlockData() {
		if (!blocksuiteStore) return;

		const data = blocksuiteStore.getBlock(blockId);
		if (data) {
			mainPost = {
				content: data.content || '',
				mode: data.mode || 'markdown',
				css: data.css || '',
				name: data.name || '',
				description: data.description || ''
			};
		}
	}

	// Handle comments store ready
	function handleCommentsStoreReady(event: CustomEvent) {
		const storeInstance = event.detail.threadCommentsStore;
		setupCommentsSubscription(storeInstance);
	}

	function setupCommentsSubscription(storeInstance: ThreadCommentsStore) {
		// Clean up previous subscription
		if (commentsUnsubscribe) {
			commentsUnsubscribe();
			commentsUnsubscribe = null;
		}

		threadCommentsStore = storeInstance;

		// Subscribe to updates
		commentsUnsubscribe = threadCommentsStore.subscribe(() => {
			loadComments();
		});

		// Initial load
		loadComments();
	}

	function loadComments() {
		if (!threadCommentsStore) return;

		comments = threadCommentsStore.getThreadComments(blockId);
	}

	async function submitComment() {
		const content = commentInput.trim();

		// Clear previous errors
		commentError = null;

		if (!content) {
			console.warn('No content provided');
			return;
		}

		if (!threadCommentsStore) {
			console.error('💬 ThreadCommentsStore is not available - comments feature may not be loaded yet');
			commentError = 'Comments are not available. Please try again.';
			return;
		}

		// Parse and sanitize markdown
		const sanitized = DOMPurify.sanitize(marked.parse(content));

		// Get user details from dataState
		const username = dataState.userDetails?.username || 'Anonymous';
		const userId = dataState.userDetails?.userId || '';

		try {
			// Add comment using store
			threadCommentsStore.addComment(blockId, {
				author: username,
				userId: userId,
				content: sanitized
			});

			console.log('✅ Comment added via ThreadCommentsStore');

			// Clear input and error
			commentInput = '';
			commentError = null;

			// Expand comments section to show the new comment
			isExpanded = true;

			// Trigger immediate save
			const currentResourceId = dataState.currentResourceId;
			if (currentResourceId) {
				dataState.saveCurrentResource(currentResourceId);

				// Sync comment to P2P network after saving
				try {
					await sendMessage('syncResource', { resourceId: currentResourceId });
					console.log('✅ Comment synced to P2P network');
				} catch (syncError) {
					console.error('Failed to sync comment:', syncError);
					// Don't fail the comment submission if sync fails
				}
			}
		} catch (error) {
			console.error('❌ Error adding comment:', error);
			commentError = 'Failed to add comment: ' + (error as Error).message;
		}
	}

	function handleKeyDown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			submitComment();
		}
	}

	// Render main post content
	const postHtml = $derived(() => {
		if (!mainPost.content) return '';
		const raw = mainPost.mode === 'html'
			? mainPost.content
			: marked.parse(mainPost.content);
		return DOMPurify.sanitize(raw);
	});

	// Listen for store ready events
	onMount(() => {
		// Check if coordinator and stores already exist (event may have already fired)
		const coordinator = dataState.getBlocksuiteCoordinator();
		if (coordinator) {
			const existingBlocksuiteStore = coordinator.getBlocksuiteStore();
			const existingCommentsStore = coordinator.getThreadCommentsStore();

			if (existingBlocksuiteStore) {
				console.log('💬 [ThreadBlock] Found existing BlocksuiteStore on mount');
				setupBlocksuiteSubscription(existingBlocksuiteStore);
			}

			if (existingCommentsStore) {
				console.log('💬 [ThreadBlock] Found existing ThreadCommentsStore on mount');
				setupCommentsSubscription(existingCommentsStore);
			}
		}

		// Also listen for future events
		document.addEventListener(
			"blocksuite-store-ready",
			handleBlocksuiteStoreReady as EventListener
		);
		document.addEventListener(
			"thread-comments-store-ready",
			handleCommentsStoreReady as EventListener
		);

		return () => {
			document.removeEventListener(
				"blocksuite-store-ready",
				handleBlocksuiteStoreReady as EventListener
			);
			document.removeEventListener(
				"thread-comments-store-ready",
				handleCommentsStoreReady as EventListener
			);
		};
	});

	onDestroy(() => {
		if (blocksuiteUnsubscribe) {
			blocksuiteUnsubscribe();
		}
		if (commentsUnsubscribe) {
			commentsUnsubscribe();
		}
	});
</script>

<div class="thread-container" data-block-id={blockId} style={mainPost.css}>
	{#if mainPost.name || mainPost.description || mainPost.content}
		<div class="thread-main">
			{#if mainPost.name}
				<h2 class="thread-title">{mainPost.name}</h2>
			{/if}
			{#if mainPost.description}
				<p class="thread-description">{mainPost.description}</p>
			{/if}
			<div class="thread-content">
				{@html postHtml()}
			</div>
		</div>
	{/if}

	<div class="thread-comments">
		{#if comments.length > 0}
			<button class="comments-toggle" onclick={() => isExpanded = !isExpanded}>
				<span class="toggle-icon">{isExpanded ? '▼' : '▶'}</span>
				<span class="comments-count">
					{#if comments.length === 1}
						1 comment
					{:else}
						{comments.length} comments
					{/if}
				</span>
			</button>

			{#if isExpanded}
				<div class="comments-expanded">
					<div class="comments-list">
						{#each comments as comment (comment.id)}
							<div class="comment">
								<div class="comment-header">
									<strong class="comment-author">{comment.author}</strong>
									<span class="comment-time">
										{new Date(comment.timestamp).toLocaleString()}
									</span>
								</div>
								<div class="comment-content">
									{@html comment.content}
								</div>
							</div>
						{/each}
					</div>
				</div>
			{/if}
		{/if}

		<!-- Comment form always visible -->
		<div class="comment-form" class:first-comment={comments.length === 0}>
			{#if comments.length === 0}
				<p class="no-comments-label">Be the first to comment!</p>
			{/if}
			{#if commentError}
				<div class="comment-error">
					{commentError}
				</div>
			{/if}
			<textarea
				bind:value={commentInput}
				placeholder="Write a comment (Markdown supported)... Press Enter to post, Shift+Enter for new line"
				onkeydown={handleKeyDown}
				rows="3"
			></textarea>
			<button onclick={submitComment} disabled={!commentInput.trim()}>
				Post Comment
			</button>
		</div>
	</div>
</div>

<style>
	.thread-container {
		width: 100%;
		padding: 1.5rem;
		background: var(--thread-bg, #f6f8fa);
		border-radius: var(--thread-border-radius, 8px);
		border: 1px solid var(--thread-border-color, #e1e4e8);
		margin: 1rem 0;
		color: var(--thread-text-color, #24292e);
	}

	.thread-main {
		margin-bottom: 2rem;
		padding-bottom: 1.5rem;
		border-bottom: 2px solid var(--thread-border-color, #e1e4e8);
	}

	.thread-title {
		font-size: 1.75rem;
		font-weight: 700;
		color: var(--thread-title-color, #24292e);
		margin: 0 0 0.5rem 0;
	}

	.thread-description {
		font-size: 1rem;
		color: var(--thread-description-color, #6e7681);
		margin: 0 0 1rem 0;
	}

	.thread-content {
		color: var(--thread-content-color, #24292e);
		line-height: 1.6;
		max-height: 500px;
		overflow-y: auto;
		padding-right: 0.5rem;
	}

	.thread-content::-webkit-scrollbar {
		width: 6px;
	}

	.thread-content::-webkit-scrollbar-track {
		background: var(--thread-scrollbar-track, #f1f3f5);
		border-radius: 3px;
	}

	.thread-content::-webkit-scrollbar-thumb {
		background: var(--thread-scrollbar-thumb, #adb5bd);
		border-radius: 3px;
	}

	.thread-content::-webkit-scrollbar-thumb:hover {
		background: var(--thread-scrollbar-thumb-hover, #868e96);
	}

	.thread-content :global(h1),
	.thread-content :global(h2),
	.thread-content :global(h3) {
		color: var(--thread-content-heading-color, #24292e);
		margin-bottom: 0.75rem;
		margin-top: 1.5rem;
	}

	.thread-content :global(p) {
		margin-bottom: 1rem;
	}

	.thread-content :global(a) {
		color: var(--thread-link-color, #0366d6);
		text-decoration: none;
	}

	.thread-content :global(a:hover) {
		text-decoration: underline;
	}

	.thread-content :global(code) {
		background: var(--thread-code-bg, #f6f8fa);
		color: var(--thread-code-color, inherit);
		padding: 0.2em 0.4em;
		border-radius: 3px;
		font-family: monospace;
		font-size: 0.9em;
	}

	.thread-content :global(pre) {
		background: var(--thread-code-bg, #f6f8fa);
		padding: 1rem;
		border-radius: 6px;
		overflow-x: auto;
		margin-bottom: 1rem;
	}

	.thread-comments {
		margin-top: 1.5rem;
	}

	.comments-toggle {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.75rem 1rem;
		background: var(--thread-toggle-bg, white);
		border: 1px solid var(--thread-toggle-border, #d1d5da);
		border-radius: 6px;
		width: 100%;
		text-align: left;
		cursor: pointer;
		transition: all 0.2s;
		font-size: 0.9375rem;
		font-weight: 600;
		color: var(--thread-toggle-color, #586069);
	}

	.comments-toggle:hover {
		background: var(--thread-toggle-hover-bg, #f6f8fa);
		border-color: var(--thread-toggle-hover-border, #0366d6);
		color: var(--thread-toggle-hover-color, #0366d6);
	}

	.toggle-icon {
		font-size: 0.75rem;
		color: var(--thread-toggle-icon-color, #6a737d);
		transition: transform 0.2s;
	}

	.comments-count {
		flex: 1;
	}

	.comments-expanded {
		margin-top: 1rem;
		animation: slideDown 0.2s ease-out;
	}

	@keyframes slideDown {
		from {
			opacity: 0;
			transform: translateY(-10px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.comments-list {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		margin-bottom: 1.5rem;
		max-height: 400px;
		overflow-y: auto;
		padding-right: 0.5rem;
	}

	.comments-list::-webkit-scrollbar {
		width: 6px;
	}

	.comments-list::-webkit-scrollbar-track {
		background: var(--thread-scrollbar-track, #f1f3f5);
		border-radius: 3px;
	}

	.comments-list::-webkit-scrollbar-thumb {
		background: var(--thread-scrollbar-thumb, #adb5bd);
		border-radius: 3px;
	}

	.comments-list::-webkit-scrollbar-thumb:hover {
		background: var(--thread-scrollbar-thumb-hover, #868e96);
	}

	.comment {
		background: var(--thread-comment-bg, white);
		padding: 1rem;
		border-radius: 6px;
		border: 1px solid var(--thread-comment-border, #e1e4e8);
		color: var(--thread-comment-text, #24292e);
	}

	.comment-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 0.5rem;
	}

	.comment-author {
		font-weight: 600;
		color: var(--thread-comment-author-color, #24292e);
	}

	.comment-time {
		font-size: 0.875rem;
		color: var(--thread-comment-time-color, #586069);
	}

	.comment-content {
		color: var(--thread-comment-content-color, #24292e);
		line-height: 1.5;
	}

	.comment-content :global(p) {
		margin: 0;
	}

	.comment-content :global(p + p) {
		margin-top: 0.5rem;
	}

	.no-comments {
		color: var(--thread-no-comments-color, #586069);
		font-style: italic;
		margin: 1rem 0;
	}

	.comment-form {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		margin-top: 1rem;
		background: var(--thread-form-bg, transparent);
	}

	.comment-form.first-comment {
		margin-top: 0;
	}

	.no-comments-label {
		color: var(--thread-no-comments-color, #6e7681);
		font-size: 0.875rem;
		margin: 0 0 0.75rem 0;
		font-style: italic;
	}

	textarea {
		width: 100%;
		padding: 0.75rem;
		border: 1px solid var(--thread-input-border, #d1d5da);
		border-radius: 6px;
		background: var(--thread-input-bg, white);
		color: var(--thread-input-text, #24292e);
		font-family: inherit;
		font-size: 0.875rem;
		line-height: 1.5;
		resize: vertical;
		transition: border-color 0.2s;
	}

	textarea:focus {
		outline: none;
		border-color: var(--thread-input-focus-border, #0366d6);
		box-shadow: 0 0 0 3px var(--thread-input-focus-shadow, rgba(3, 102, 214, 0.1));
	}

	textarea::placeholder {
		color: var(--thread-input-placeholder, #6e7681);
	}

	button {
		align-self: flex-end;
		padding: 0.75rem 1.5rem;
		background: var(--thread-button-bg, #0366d6);
		color: var(--thread-button-text, white);
		border: none;
		border-radius: 6px;
		font-weight: 600;
		cursor: pointer;
		transition: background 0.2s;
	}

	button:hover:not(:disabled) {
		background: var(--thread-button-hover-bg, #0256c7);
	}

	button:disabled {
		background: var(--thread-button-disabled-bg, #94a3b8);
		color: var(--thread-button-disabled-text, #cbd5e0);
		cursor: not-allowed;
	}

	.comment-error {
		padding: 0.75rem;
		background: var(--thread-error-bg, #f8d7da);
		color: var(--thread-error-text, #721c24);
		border: 1px solid var(--thread-error-border, #f5c6cb);
		border-radius: 6px;
		font-size: 0.875rem;
		margin-bottom: 0.75rem;
	}
</style>
