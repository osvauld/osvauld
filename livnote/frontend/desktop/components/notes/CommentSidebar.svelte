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

	// Derived counts to avoid recomputing in template
	const unresolvedCount = $derived.by(() => {
		return sortedThreads.filter((t) => !t.threadInfo.resolved).length;
	});

	const resolvedCount = $derived.by(() => {
		return sortedThreads.filter((t) => t.threadInfo.resolved).length;
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

<!-- Flash animation for highlighting comments -->
<style>
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

<div class="w-full h-full flex flex-col overflow-hidden pb-[3px] min-h-0">
	<div class="py-4 relative">
		<h3
			class="text-base font-normal tracking-[0.02em] text-osvauld-plainwhite m-0 border-b border-osvauld-defaultBorder pb-4 mb-4 w-full"
		>
			Comments
		</h3>
		<div class="flex gap-4">
			<button
				class="text-[15px] font-normal tracking-[0.02em] p-0 cursor-pointer text-textActive text-left transition-all duration-100 ease-in-out border-b-2 border-transparent relative hover:text-osvauld-plainwhite hover:border-osvauld-plainwhite"
				class:!text-osvauld-plainwhite={!showResolved}
				class:!border-osvauld-plainwhite={!showResolved}
				onclick={() => (showResolved = false)}
			>
				Open<sup class="ml-0.5 text-[10px] text-textActive align-super"
					>{unresolvedCount}</sup
				>
				{#if hasUnreadComments}
					<span
						class="absolute -top-1 -right-1 w-2 h-2 bg-liveGreen rounded-full"
					></span>
				{/if}
			</button>
			<button
				class="text-[15px] font-normal tracking-[0.02em] p-0 cursor-pointer text-textActive text-left transition-all duration-100 ease-in-out border-b-2 border-transparent hover:text-osvauld-plainwhite hover:border-osvauld-plainwhite"
				class:!text-osvauld-plainwhite={showResolved}
				class:!border-osvauld-plainwhite={showResolved}
				onclick={() => (showResolved = true)}
			>
				Resolved<sup class="ml-0.5 text-[10px] text-textActive align-super"
					>{resolvedCount}</sup
				>
			</button>
		</div>
	</div>

	<div class="scrollbar-thin flex-1 overflow-y-auto min-h-0">
		{#if filteredThreads.length === 0}
			<div class="py-4 text-center text-textActive">
				{#if threads.length === 0}
					<span><CommentIcon size={24} color="var(--color-textActive)" /></span>
					<div
						class="text-sm text-textActive leading-[1.4] font-normal tracking-[0.02em] text-left mt-4"
					>
						Give feedback, ask a question, or just leave a note of appreciation. <br
						/>
						Select anywhere in the note to leave a comment.
					</div>
				{:else if showResolved}
					<div
						class="text-sm font-normal tracking-[0.02em] mb-2 text-textActive text-left"
					>
						No resolved comments
					</div>
					<div
						class="text-sm text-textActive leading-[1.4] font-normal tracking-[0.02em] text-left"
					>
						Resolved comments will appear here.
					</div>
				{:else}
					<div
						class="text-sm font-normal tracking-[0.02em] mb-2 text-textActive text-left"
					>
						No active comments
					</div>
					<div
						class="text-sm text-textActive leading-[1.4] font-normal tracking-[0.02em] text-left"
					>
						All comments have been resolved.
					</div>
				{/if}
			</div>
		{:else}
			<div class="flex flex-col min-h-0 overflow-y-auto pr-1">
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
