<script lang="ts">
    import { onMount, onDestroy } from "svelte";
	import type { CommentThread, Reply } from "../../types/notes.types";
	import { ReplyIcon } from "@osvauld/icons";
	import { dataState } from "../../state";
    import type { CommentsStore } from "./commentsStore";

	// Props
	interface Props {
		thread: CommentThread;
		isSelected?: boolean;
		isHighlighted?: boolean;
		onSelect: () => void;
		onResolve: (resolved: boolean) => void;
		onDelete: () => void;
	}

	const {
		thread,
		isSelected = false,
		isHighlighted = false,
		onSelect,
		onResolve,
		onDelete,
	}: Props = $props();

	// State
	let isExpanded = $state(false);
	let replyText = $state("");
	let isAddingReply = $state(false);
	let replyFormRef = $state<HTMLDivElement>();
	let commentsStore = $state<CommentsStore | null>(null);
	let contentVersion = $state(0);
	let unsubscribe: (() => void) | null = null;
	let userCollapsed = $state(false);
	let currentUserId: string | null = null;

	// Acquire commentsStore once ready
	function setCommentsStoreFromCoordinator() {
		try {
			const coordinator = dataState.getNotesCoordinator();
			const store = coordinator?.getCommentsStore();
			if (store) commentsStore = store;
			currentUserId = dataState.userDetails?.userId || null;
		} catch (_) {
			// ignore; will be provided by event later
		}
	}

	onMount(() => {
		setCommentsStoreFromCoordinator();
		const handleReady = (event: CustomEvent) => {
			commentsStore = event.detail.commentsStore;
		};
		document.addEventListener(
			"comments-store-ready",
			handleReady as EventListener,
		);
		// grab user id once mounted as well
		currentUserId = dataState.userDetails?.userId || null;
		return () => {
			document.removeEventListener(
				"comments-store-ready",
				handleReady as EventListener,
			);
			if (unsubscribe) {
				unsubscribe();
				unsubscribe = null;
			}
		};
	});

	// Subscribe to commentsStore changes to refresh content
	$effect(() => {
		if (commentsStore && typeof commentsStore.subscribe === "function") {
			if (unsubscribe) unsubscribe();
			unsubscribe = commentsStore.subscribe(() => {
				contentVersion++;
			});
		}
	});

	// Derived values
	const replyCount = $derived(
		thread.replies.length > 1 ? thread.replies.length - 1 : 0,
	);
	const mainReply = $derived(thread.replies[0] ?? {
		id: "",
		authorName: "Unknown",
		createdAt: thread.threadInfo.createdAt,
	});
	const additionalReplies = $derived(thread.replies.slice(1));
	const unreadRepliesCount = $derived.by(() => {
		if (!currentUserId) return 0;
		return additionalReplies.filter((r) => !r.readBy?.includes(currentUserId!)).length;
	});
	const previewText = $derived(getPreviewText());

	function getPreviewText(): string {
		// Get the text that was commented on from the document position
		try {
			const coordinator = dataState.getNotesCoordinator();
			const editorView = coordinator?.getEditorView();

			if (editorView && thread.threadInfo.position) {
				const doc = editorView.state.doc;
				const text = doc.textBetween(
					thread.threadInfo.position.from,
					thread.threadInfo.position.to,
					" ",
				);
				return text.substring(0, 50) + (text.length > 50 ? "..." : "");
			}
		} catch (error) {
			console.warn("Could not get preview text:", error);
		}
		return "Text excerpt";
	}

	function getReplyContent(replyId: string): string {
		if (!commentsStore) return "";
		return commentsStore.getReplyPlainText(replyId);
	}

	function getReplyContentReactive(replyId: string): string {
		// depend on version so UI updates when store pushes changes
		contentVersion;
		return getReplyContent(replyId);
	}

	function sanitize(text: string): string {
		const div = document.createElement("div");
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
		const highlightTextEvent = new CustomEvent("highlight-comment-text", {
			detail: { threadId: thread.id, position: thread.threadInfo.position },
		});
		document.dispatchEvent(highlightTextEvent);

		// Also trigger the expand/select logic
		handleToggleExpand();
	}

	function handleAddReply() {
		const trimmedReply = replyText.trim();
		if (!trimmedReply) return;

		try {
			if (commentsStore) {
				commentsStore.addReply(thread.id, sanitize(trimmedReply));
				replyText = "";
				isAddingReply = false;
			}
		} catch (error) {
			console.error("Error adding reply:", error);
		}
	}

	function formatTimestamp(timestamp: number): string {
		const date = new Date(timestamp);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffMins = Math.floor(diffMs / (1000 * 60));
		const diffHours = Math.floor(diffMins / 60);
		const diffDays = Math.floor(diffHours / 24);

		if (diffMins < 1) return "Just now";
		if (diffMins < 60) return `${diffMins}m ago`;
		if (diffHours < 24) return `${diffHours}h ago`;
		if (diffDays < 7) return `${diffDays}d ago`;

		return date.toLocaleDateString();
	}

	// Scroll reply form into view when it becomes visible
	$effect(() => {
		if (isAddingReply && replyFormRef) {
			setTimeout(() => {
				replyFormRef?.scrollIntoView({
					behavior: "smooth",
					block: "nearest",
					inline: "nearest",
				});
			}, 100);
		}
	});

	// Expand when becoming selected, unless user manually collapsed
	$effect(() => {
		if (!isSelected) {
			userCollapsed = false; // reset when deselected
			return;
		}
		if (isSelected && !isExpanded && !userCollapsed) {
			isExpanded = true;
		}
	});
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="group rounded-md overflow-hidden transition-all duration-75 hover:bg-osvauld-frameblack hover:border-osvauld-defaultBorder {isSelected
		? 'bg-osvauld-frameblack border-osvauld-defaultBorder'
		: ''} {thread.threadInfo.resolved ? 'opacity-70' : ''} {isHighlighted
		? 'animate-pulse'
		: ''}"
>
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="p-2 cursor-pointer flex items-start gap-2 relative"
		onclick={handleThreadClick}
		onkeydown={(e) => {
			if (e.key === 'Enter' || e.key === ' ') {
				e.preventDefault();
				handleThreadClick();
			}
			if (e.key.toLowerCase() === 'r') {
				const target = e.target as HTMLElement | null;
				if (target) {
					const tag = target.tagName;
					const isTextEntry = tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable || (typeof target.matches === 'function' && target.matches('[role="textbox"], [contenteditable="true"]'));
					if (isTextEntry) {
						return;
					}
				}
				e.preventDefault();
				isExpanded = true;
				isAddingReply = true;
			}
		}}
		tabindex="0"
		aria-label="Comment thread. Press Enter to expand, R to reply"
	>
		<div class="flex-1 min-w-0">
			<div class="flex justify-start items-center gap-2">
			
				<div class="flex items-center gap-2 min-w-0">
					<span class="capitalize text-white text-sm font-medium tracking-wider truncate max-w-[10rem]"
						>{mainReply.authorName}</span
					>
					<span class="text-[11px] text-textActive">• {formatTimestamp(mainReply.createdAt)}</span>
				</div>
			</div>
			<div
				class="text-[11px] my-2.5 italic border-l border-livnotePink pl-1.5 flex items-center justify-start text-textActive"
			>
				<span class="max-w-full overflow-hidden text-ellipsis whitespace-nowrap tracking-wider"
					>"{previewText}"</span
				>
			</div>
			<div
				class="text-[13px] text-white leading-relaxed max-w-full overflow-hidden text-ellipsis whitespace-nowrap {isExpanded
					? 'whitespace-normal overflow-visible text-ellipsis-clip break-words'
					: ''}"
			>
				<span>{getReplyContentReactive(mainReply.id) || "Message unavailable"}</span>
			</div>
			<div class="flex items-center justify-between">
				
					<div class="text-xs text-textActive flex items-center">
						{replyCount}
						{replyCount === 1 ? "reply" : "replies"}
						{#if !isExpanded && unreadRepliesCount > 0}
							<span class="ml-1 inline-flex items-center justify-center rounded bg-red-500  text-[10px] leading-none px-[4px] min-w-[14px] h-[14px]">
								{unreadRepliesCount}
							</span>
						{/if}
					</div>
				
				{#if isExpanded}
					<button
						class="flex items-center gap-1 text-[11px] text-white/80 hover:text-white transition-colors px-1 py-0.5 rounded cursor-pointer"
						aria-label="Hide replies"
						onclick={(e) => {
							e.stopPropagation();
							isExpanded = false;
							userCollapsed = true;
						}}
					>
						Hide
					</button>
				{:else}
					<button
						class="flex items-center gap-1 text-[11px] text-white/80 hover:text-white transition-colors px-1 py-0.5 rounded cursor-pointer"
						aria-label="Reply to thread"
						onclick={(e) => {
							e.stopPropagation();
							isAddingReply = true;
							if (!isExpanded) isExpanded = true;
						}}
					>
						<ReplyIcon /> Reply
					</button>
				{/if}
			</div>
		</div>
		<div
			class="absolute top-3 right-2 flex items-start gap-1 transition-opacity duration-200 {isSelected ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}"
		>
			{#if thread.threadInfo.resolved}
				<button
					class="p-1 border-none bg-transparent text-commentUnresolve cursor-pointer rounded-sm flex items-center justify-center transition-all duration-200 hover:bg-osvauld-defaultBorder hover:text-osvauld-fieldTextActive"
					title="Mark as unresolved"
					onclick={(e) => {
						e.stopPropagation();
						onResolve(false);
					}}
				>
					<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
						<path
							d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm4.59-12.42L10 14.17l-2.59-2.58L6 13l4 4 8-8z"
						></path>
					</svg>
				</button>
			{:else}
				<button
					class="p-1 border-none bg-transparent text-commentResolve cursor-pointer rounded-sm flex items-center justify-center transition-all duration-200 hover:bg-osvauld-defaultBorder hover:text-osvauld-fieldTextActive"
					title="Mark as resolved"
					onclick={(e) => {
						e.stopPropagation();
						onResolve(true);
					}}
				>
					<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
						<path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"></path>
					</svg>
				</button>
			{/if}

			<button
				class="p-1 border-none bg-transparent text-commentDelete cursor-pointer rounded-sm flex items-center justify-center transition-all duration-200 hover:bg-osvauld-defaultBorder hover:text-osvauld-fieldTextActive"
				title="Delete thread"
				onclick={(e) => {
					e.stopPropagation();
					if (confirm('Delete this comment thread?')) onDelete();
				}}
			>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
					<path
						d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
					></path>
				</svg>
			</button>
		</div>
	</div>

	{#if isExpanded}
		<div class="p-2 pt-0 border-t border-osvauld-defaultBorder text-xs">
			{#if additionalReplies.length > 0}
				<div class="mt-2 ml-2 pl-1 border-l border-osvauld-defaultBorder">
					{#each additionalReplies as reply, i (reply.id)}
						<div class="mb-1 pl-1 {i !== additionalReplies.length - 1 ? 'border-b border-osvauld-defaultBorder' : ''}">
							<div class="flex justify-start items-center gap-2">
							
								<div class="flex items-center gap-2 min-w-0">
									<span
										class="capitalize text-white text-sm font-medium tracking-wider truncate max-w-[10rem]"
										>{reply.authorName}</span
									>
									<span class="text-[11px] text-statusColor">• {formatTimestamp(reply.createdAt)}</span>
								</div>
							</div>
							<div
								class="text-[13px] text-textActive leading-relaxed mt-0.5 mb-1 break-words"
							>
								{getReplyContentReactive(reply.id)}
							</div>
						</div>
					{/each}
				</div>
			{/if}

			{#if isAddingReply}
				<div bind:this={replyFormRef} class="mt-3 flex flex-col gap-2">
					<textarea
						class="w-full p-2 border border-osvauld-defaultBorder rounded bg-osvauld-fieldActive text-osvauld-fieldTextActive text-[13px] focus:outline-none focus:border-livnotePink placeholder:text-statusColor"
						bind:value={replyText}
						placeholder="Add a reply..."
						autofocus
						autocapitalize="off"
						spellcheck="false"
						maxlength="1000"
						onkeydown={(e) => {
							if (e.key === "Enter" && !e.shiftKey) {
								e.preventDefault();
								handleAddReply();
							}
						}}
					></textarea>
					<div class="flex gap-2 justify-between items-center">
						<span class="text-[11px] text-statusColor">
							{#if replyText.length > 900}
								{replyText.length}/1000
							{:else}
								Enter to submit • Shift+Enter for newline
							{/if}
						</span>
						<div class="flex items-center gap-2">
							<button
								class="px-3 py-1.5 border border-osvauld-defaultBorder rounded bg-transparent text-osvauld-fieldText text-xs cursor-pointer transition-all duration-200 hover:bg-osvauld-defaultBorder hover:text-osvauld-fieldTextActive"
								onclick={() => {
									isAddingReply = false;
									replyText = "";
								}}
							>
								Cancel
							</button>
							<button
								class="px-3 py-1.5 border-none rounded bg-livnotePink text-primarydark text-xs cursor-pointer transition-all duration-200 hover:bg-livnotePink"
								onclick={handleAddReply}
								disabled={!replyText.trim()}
							>
								Submit
							</button>
						</div>
					</div>
				</div>
			{:else}
				<div class="flex justify-end mt-1">
				<button
					class="flex items-center gap-1 text-[11px] text-white/80 hover:text-white transition-colors px-1 py-0.5 rounded cursor-pointer"
					aria-label="Reply to thread"
					onclick={() => (isAddingReply = true)}
				>
					<ReplyIcon /> Reply
				</button>
				</div>
			{/if}
		</div>
	{/if}
</div>

<div class="w-full h-px bg-[#2a2b2f] my-1.5"></div>
