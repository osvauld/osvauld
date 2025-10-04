<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { CommentIcon } from "@osvauld/icons";

	import type { CommentThread } from "../../types/notes.types";
	import CommentThreadComponent from "./CommentThread.svelte";
	import { dataState } from "../../state";

	interface Props {
		onClose?: () => void;
	}

	const { onClose }: Props = $props();

	let selectedThreadId = $state<string | null>(null);
	let showResolved = $state(false);
	let highlightedThreadId = $state<string | null>(null);
	let animatingThreadId = $state<string | null>(null);
	let animationTimeoutId: ReturnType<typeof setTimeout> | null = null;
	let commentsUnsubscribe: (() => void) | null = null;
	let threads = $state<CommentThread[]>([]);
	let commentsStore: any = null;
	let currentUserId: string | null = null;

	function handleCommentsStoreReady(event: CustomEvent) {
		const commentsStoreInstance = event.detail.commentsStore;
		setupCommentsSubscriptionWithStore(commentsStoreInstance);
	}

	function setupCommentsSubscriptionWithStore(commentsStoreInstance: any) {
		// Clean up previous subscription
		if (commentsUnsubscribe) {
			commentsUnsubscribe();
			commentsUnsubscribe = null;
		}

		// Store reference to commentsStore
		commentsStore = commentsStoreInstance;

		// Get current user ID
		currentUserId = dataState.userDetails?.userId || null;

		// Subscribe to updates
		commentsUnsubscribe = commentsStore.subscribe(() => {
			threads = commentsStore.getAllThreads();
		});

		// Get initial threads
		threads = commentsStore.getAllThreads();
	}

	// Derived values
	const sortedThreads = $derived.by(() => {
		let sorted = [...threads].sort(
			(a, b) => b.threadInfo.createdAt - a.threadInfo.createdAt,
		);

		// If a thread is highlighted, move it to the top
		if (highlightedThreadId) {
			const highlightedIndex = sorted.findIndex(
				(t) => t.id === highlightedThreadId,
			);
			if (highlightedIndex > 0) {
				const [highlightedThread] = sorted.splice(highlightedIndex, 1);
				sorted.unshift(highlightedThread);
			}
		}

		return sorted;
	});

	const filteredThreads = $derived.by(() => {
		return sortedThreads.filter((thread: CommentThread) =>
			showResolved ? thread.threadInfo.resolved : !thread.threadInfo.resolved,
		);
	});

	// Check for unread comments
	const hasUnreadComments = $derived.by(() => {
		if (!commentsStore || !currentUserId) return false;

		return threads.some((thread: CommentThread) => {
			if (thread.threadInfo.resolved) return false;

			// Check if any reply is unread
			return thread.replies.some(
				(reply) => !reply.readBy.includes(currentUserId!),
			);
		});
	});

	function handleThreadSelect(threadId: string) {
		selectedThreadId = selectedThreadId === threadId ? null : threadId;

		// Mark thread as read when selected
		markThreadAsRead(threadId);

		// Scroll to the commented text in the editor
		scrollToCommentInEditor(threadId);
	}

	function markThreadAsRead(threadId: string) {
		if (commentsStore) {
			commentsStore.markThreadAsRead(threadId);
		}
	}

	function scrollToCommentInEditor(threadId: string) {
		// Find the comment span in the editor
		const commentSpan = document.querySelector(
			`[data-livnote-comment="${threadId}"]`,
		);
		if (commentSpan) {
			commentSpan.scrollIntoView({
				behavior: "smooth",
				block: "center",
				inline: "nearest",
			});

			// Temporarily highlight the comment
			commentSpan.classList.add("comment-flash");
			setTimeout(() => {
				commentSpan.classList.remove("comment-flash");
			}, 2000);
		}
	}

	function handleResolveThread(threadId: string, resolved: boolean) {
		try {
			const coordinator = dataState.getNotesCoordinator();
			const commentsService = coordinator?.getCommentsStore();
			if (commentsService) {
				if (resolved) {
					commentsService.resolveThread(threadId);
				} else {
					commentsService.unresolveThread(threadId);
				}
			}
		} catch (error) {
			console.error("Error resolving thread:", error);
		}
	}

	function handleDeleteThread(threadId: string) {
		try {
			const coordinator = dataState.getNotesCoordinator();
			const commentsService = coordinator?.getCommentsStore();
			if (commentsService) {
				commentsService.deleteThread(threadId);
			}
		} catch (error) {
			console.error("Error deleting thread:", error);
		}
	}

	function highlightThread(threadId: string) {
		// Clear any existing timeouts
		if (animationTimeoutId) {
			clearTimeout(animationTimeoutId);
		}

		// Find the thread to check if it's resolved
		const thread = threads.find((t) => t.id === threadId);
		if (thread) {
			// If the thread is resolved, switch to resolved tab
			if (thread.threadInfo.resolved && !showResolved) {
				showResolved = true;
			}
			// If the thread is active, switch to active tab
			else if (!thread.threadInfo.resolved && showResolved) {
				showResolved = false;
			}
		}

		// Set highlighted thread and move it to top
		highlightedThreadId = threadId;

		// Set temporary animation state
		animatingThreadId = threadId;

		// Remove animation after 4 seconds
		animationTimeoutId = window.setTimeout(() => {
			animatingThreadId = null;
		}, 4000);
	}

	// Listen for events
	onMount(() => {
		document.addEventListener(
			"comments-store-ready",
			handleCommentsStoreReady as EventListener,
		);
		// Listen for comment highlight events from editor
		const handleCommentHighlight = (event: CustomEvent) => {
			const { threadId } = event.detail;
			highlightThread(threadId);
		};

		document.addEventListener(
			"highlight-comment-thread",
			handleCommentHighlight as EventListener,
		);

		return () => {
			document.removeEventListener(
				"highlight-comment-thread",
				handleCommentHighlight as EventListener,
			);
			document.removeEventListener(
				"comments-store-ready",
				handleCommentsStoreReady as EventListener,
			);
		};
	});

	onDestroy(() => {
		// Clean up timeouts
		if (animationTimeoutId) {
			clearTimeout(animationTimeoutId);
		}
		if (commentsUnsubscribe) {
			commentsUnsubscribe();
		}
	});
</script>

<!--Styles remain the same -->
<style>
	.comment-sidebar {
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		padding-bottom: 3px;
		min-height: 0;
	}

	.sidebar-header {
		padding: 16px 0px;
		position: relative;
	}

	.sidebar-title {
		font-size: 16px;
		font-weight: 300;
		letter-spacing: 0.02em;
		color: #fff;
		margin: 0 0 8px 0;
		border-bottom: 1px solid #2a2b2f;
		padding-bottom: 16px;
		margin-bottom: 16px;
		width: 100%;
	}

	.filter-tabs {
		display: flex;
		gap: 16px;
	}

	.filter-tab {
		font-size: 15px;
		font-weight: 300;
		letter-spacing: 0.02em;
		padding: 0;
		cursor: pointer;
		color: var(--color-statusColor);
		text-align: left;
		transition: all 0.1s ease;
		border-bottom: 2px solid transparent;
	}

	.filter-tab:hover {
		color: #fff;
		border-color: #fff;
	}

	.filter-tab.active {
		color: #fff;
		border-color: #fff;
	}

	.sidebar-content {
		flex: 1;
		overflow-y: auto;
		min-height: 0;
	}

	.empty-state {
		padding: 16px 0px;
		text-align: center;
		color: #85889c;
	}

	.empty-state-title {
		font-size: 14px;
		font-weight: 300;
		letter-spacing: 0.02em;
		margin-bottom: 8px;
		color: #fff;
	}

	.empty-state-text {
		font-size: 12px;
		color: var(--color-statusColor);
		line-height: 1.4;
		font-weight: 200;
		letter-spacing: 0.02em;
		text-align: left;
	}

	.thread-list {
		display: flex;
		flex-direction: column;
		min-height: 0;
		overflow-y: auto;
		padding-right: 4px;
	}

	/* Scrollbar styling */
	.sidebar-content::-webkit-scrollbar {
		width: 4px;
	}

	.sidebar-content::-webkit-scrollbar-track {
		background: transparent;
	}

	.sidebar-content::-webkit-scrollbar-thumb {
		background-color: #2f303e;
		border-radius: 4px;
	}

	/* Flash animation for highlighting comments */
	:global(.comment-flash) {
		animation: commentFlash 2s ease-in-out;
	}

	@keyframes commentFlash {
		0%,
		100% {
			background: rgb(124 145 249 / 0.1);
		}
		50% {
			background: rgb(124 145 249 / 0.3);
		}
	}
</style>

<div class="comment-sidebar">
	<div class="sidebar-header">
		<h3 class="sidebar-title">Comments</h3>
		<div class="filter-tabs">
			<button
				class="filter-tab relative"
				class:active={!showResolved}
				onclick={() => (showResolved = false)}
			>
				Open
				{#if hasUnreadComments}
					<span
						class="absolute -top-1 -right-1 w-2 h-2 bg-green-500 rounded-full"
					></span>
				{/if}
			</button>
			<button
				class="filter-tab"
				class:active={showResolved}
				onclick={() => (showResolved = true)}
			>
				Resolved
			</button>
		</div>
	</div>

	<div class="sidebar-content">
		{#if filteredThreads.length === 0}
			<div class="empty-state">
				{#if threads.length === 0}
					<span><CommentIcon size={24} /></span>
					<div class="empty-state-text mt-4 text-xs">
						Give feedback, ask a question, or just leave a note of appreciation. <br
						/>
						Select anywhere in the note to leave a comment.
					</div>
				{:else if showResolved}
					<div class="empty-state-title text-left">No resolved comments</div>
					<div class="empty-state-text">
						Resolved comments will appear here.
					</div>
				{:else}
					<div class="empty-state-title text-left">No active comments</div>
					<div class="empty-state-text text-left">
						All comments have been resolved.
					</div>
				{/if}
			</div>
		{:else}
			<div class="thread-list">
				{#each filteredThreads as thread, index (thread.id)}
					<CommentThreadComponent
						{thread}
						isSelected={selectedThreadId === thread.id}
						isHighlighted={animatingThreadId === thread.id}
						onSelect={() => handleThreadSelect(thread.id)}
						onResolve={(resolved: boolean) =>
							handleResolveThread(thread.id, resolved)}
						onDelete={() => handleDeleteThread(thread.id)}
					/>
				{/each}
			</div>
		{/if}
	</div>
</div>
