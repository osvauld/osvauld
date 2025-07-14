<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import {
		CommentIcon 
	} from "@osvauld/password-manager-common";

	import type { CommentThread } from "../../types/notes.types";
	import CommentThreadComponent from "./CommentThread.svelte";
	import { notesInstance } from "./notes";

	// Props using Svelte 5 runes
	interface Props {
		isVisible?: boolean;
		onClose?: () => void;
	}

	const { isVisible = true, onClose }: Props = $props();

	// State
	let threads = $state<CommentThread[]>([]);
	let isLoading = $state(false);
	let selectedThreadId = $state<string | null>(null);
	let showResolved = $state(false);
	let highlightedThreadId = $state<string | null>(null);
	let highlightTimeoutId: ReturnType<typeof setTimeout> | null = null;
	let animatingThreadId = $state<string | null>(null);
	let animationTimeoutId: ReturnType<typeof setTimeout> | null = null;

	// Derived values
	const sortedThreads = $derived.by(() => {
		let sorted = [...threads].sort((a, b) => a.position.from - b.position.from);

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
			showResolved ? thread.resolved : !thread.resolved,
		);
	});

	function loadThreads() {
		try {
			threads = notesInstance.getAllCommentThreads();
		} catch (error) {
			console.error("Error loading comment threads:", error);
		}
	}

	function handleThreadSelect(threadId: string) {
		selectedThreadId = selectedThreadId === threadId ? null : threadId;

		// Scroll to the commented text in the editor
		scrollToCommentInEditor(threadId);
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
			notesInstance.resolveCommentThread(threadId, resolved);
		} catch (error) {
			console.error("Error resolving thread:", error);
		}
	}

	function handleDeleteThread(threadId: string) {
		if (confirm("Are you sure you want to delete this comment thread?")) {
			try {
				notesInstance.removeCommentMark(threadId);
			} catch (error) {
				console.error("Error deleting thread:", error);
			}
		}
	}

	function getStatsText() {
		const activeCount = threads.filter((t) => !t.resolved).length;
		const resolvedCount = threads.filter((t) => t.resolved).length;
		return `${activeCount} active, ${resolvedCount} resolved`;
	}

	function highlightThread(threadId: string) {
		// Clear any existing timeouts
		if (highlightTimeoutId) {
			clearTimeout(highlightTimeoutId);
		}
		if (animationTimeoutId) {
			clearTimeout(animationTimeoutId);
		}
		// Find the thread to check if it's resolved
		const thread = threads.find((t) => t.id === threadId);
		if (thread) {
			// If the thread is resolved, switch to resolved tab
			if (thread.resolved && !showResolved) {
				showResolved = true;
			}
			// If the thread is active, switch to active tab
			else if (!thread.resolved && showResolved) {
				showResolved = false;
			}
		}

		// Set highlighted thread and move it to top (stays there permanently)
		highlightedThreadId = threadId;

		// Set temporary animation state (times out)
		animatingThreadId = threadId;

		// Remove animation after 4 seconds (matches CSS animation duration)
		animationTimeoutId = window.setTimeout(() => {
			animatingThreadId = null;
		}, 4000);
	}


	// Subscribe to comment updates
	onMount(() => {
		// Listen for comment highlight events from editor
		const handleCommentHighlight = (event: CustomEvent) => {
			const { threadId } = event.detail;
			highlightThread(threadId);
		};

		document.addEventListener(
			"highlight-comment-thread",
			handleCommentHighlight as EventListener,
		);

		// Load threads with a small delay to ensure notes instance is ready
		const initializeSidebar = () => {
			try {
				loadThreads();

				// Subscribe to real-time updates
				const commentsService = notesInstance.getCommentsService();
				commentsService.onUpdate("thread_added", loadThreads);
				commentsService.onUpdate("thread_updated", loadThreads);
				commentsService.onUpdate("thread_deleted", loadThreads);
			} catch (error) {
				console.error("Error initializing sidebar:", error);
				// Retry after a short delay
				setTimeout(initializeSidebar, 100);
			}
		};

		// Try immediately, and also after a small delay
		initializeSidebar();
		setTimeout(initializeSidebar, 50);

		return () => {
			document.removeEventListener(
				"highlight-comment-thread",
				handleCommentHighlight as EventListener,
			);
		};
	});

	onDestroy(() => {
		// Unsubscribe from updates
		try {
			const commentsService = notesInstance.getCommentsService();
			commentsService.offUpdate("thread_added", loadThreads);
			commentsService.offUpdate("thread_updated", loadThreads);
			commentsService.offUpdate("thread_deleted", loadThreads);
		} catch (error) {
			// Service might not be available during cleanup
		}

		// Clean up timeouts
		if (highlightTimeoutId) {
			clearTimeout(highlightTimeoutId);
		}
		if (animationTimeoutId) {
			clearTimeout(animationTimeoutId);
		}
	});

	// Expose loadThreads for parent component
	export { loadThreads };
</script>

<style>
	.comment-sidebar {
		width: 100%;
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		overflow: hidden;
		transform: translateX(100%);
		transition: transform 0.3s ease-in-out;
		z-index: 100;
	}

	.comment-sidebar.visible {
		transform: translateX(0);
	}

	.sidebar-header {
		padding: 16px 0px;
		position: relative;
	}

	.collapse-button {
		position: absolute;
		top: 16px;
		right: 16px;
		background: #2a2b2f;
		border: 1px solid #3a3b44;
		color: #85889c;
		cursor: pointer;
		padding: 6px;
		border-radius: 4px;
		transition: all 0.2s ease;
		display: flex;
		align-items: center;
		justify-content: center;
		opacity: 1;
		transform: scale(1);
	}

	.collapse-button:hover {
		background: #3a3b44;
		color: #bfc0cc;
		border-color: #4a4b53;
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

	.sidebar-stats {
		font-size: 12px;
		color: #85889c;
		margin-bottom: 12px;
	}

	.filter-tabs {
		display: flex;
		gap: 16px;
	}

	.filter-tab {
		font-size: 14px;
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
		border-color:  #fff;
	}

	.filter-tab.active {
		color: #fff;
		border-color: #fff;
	}

	.sidebar-content {
		flex: 1;
		overflow-y: auto;
	}

	.empty-state {
		padding: 16px 0px;
		text-align: center;
		color: #85889c;
	}

	.empty-state-title {
		font-size: 14px;
		margin-bottom: 8px;
		color: #bfc0cc;
	}

	.empty-state-text {
		font-size: 14px;
		color: var(--color-statusColor);
		line-height: 1.4;
		font-weight: 200;
		letter-spacing: 0.02em;
		text-align: left;
	}

	.thread-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.comment-thread.thread-highlighted {
		animation: highlightPulse 4s ease-in-out;
		background: rgba(255, 215, 0, 0.1);
		border-color: #ffd700 !important;
	}

	@keyframes highlightPulse {
		0%,
		100% {
			background: rgba(255, 215, 0, 0.1);
			transform: scale(1);
		}
		15% {
			background: rgba(255, 215, 0, 0.25);
			transform: scale(1.02);
		}
		30% {
			background: rgba(255, 215, 0, 0.2);
			transform: scale(1.01);
		}
		45% {
			background: rgba(255, 215, 0, 0.15);
			transform: scale(1);
		}
		60% {
			background: rgba(255, 215, 0, 0.1);
			transform: scale(1);
		}
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
			background: rgba(255, 215, 0, 0.1);
		}
		50% {
			background: rgba(255, 215, 0, 0.3);
		}
	}
</style>

<div class="comment-sidebar" class:visible={isVisible}>
	<div class="sidebar-header">
		<h3 class="sidebar-title">Comments</h3>
			<!-- <div class="sidebar-stats">{getStatsText()}</div> -->
			<div class="filter-tabs">
				<button
					class="filter-tab"
					class:active={!showResolved}
					onclick={() => (showResolved = false)}>
					Open
				</button>
				<button
					class="filter-tab"
					class:active={showResolved}
					onclick={() => (showResolved = true)}>
					Resolved
				</button>
			</div>
	</div>

	<div class="sidebar-content">
		{#if isLoading}
			<div class="empty-state">
				<div class="empty-state-title">Loading comments...</div>
			</div>
		{:else if filteredThreads.length === 0}
			<div class="empty-state">
				{#if threads.length === 0}
				  <span><CommentIcon size={24}/></span>
					<div class="empty-state-text mt-4">
						Give feedback, ask a question, or just leave a note of appreciation. <br/>
Select anywhere in the note to leave a comment.
					</div>
				{:else if showResolved}
					<div class="empty-state-title">No resolved comments</div>
					<div class="empty-state-text">
						Resolved comments will appear here.
					</div>
				{:else}
					<div class="empty-state-title text-left">
						No active comments</div>
					<div class="empty-state-text text-left">All comments have been resolved.</div>
				{/if}
			</div>
		{:else}
			<div class="thread-list">
				{#each filteredThreads as thread (thread.id)}
					<CommentThreadComponent
						{thread}
						isSelected={selectedThreadId === thread.id}
						isHighlighted={animatingThreadId === thread.id}
						onSelect={() => handleThreadSelect(thread.id)}
						onResolve={(resolved: boolean) =>
							handleResolveThread(thread.id, resolved)}
						onDelete={() => handleDeleteThread(thread.id)} />
				{/each}
			</div>
		{/if}
	</div>
</div>

