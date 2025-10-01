<script lang="ts">
	import type { CommentThread, Comment } from "../../types/notes.types";
	import { ReplyIcon } from "../../icons";
	import { dataState } from "../../state";
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

	// Derived values
	const commentCount = $derived(thread.comments.length - 1);
	const mainComment = $derived(thread.comments[0]);
	const replies = $derived(thread.comments.slice(1));
	const previewText = $derived(getPreviewText());

	function getPreviewText(): string {
		// Get the text that was commented on from the document position
		try {
			const coordinator = dataState.getNotesCoordinator();
			const editorView = coordinator?.getEditorView();

			if (editorView && thread.position) {
				const doc = editorView.state.doc;
				const text = doc.textBetween(
					thread.position.from,
					thread.position.to,
					" ",
				);
				return text.substring(0, 50) + (text.length > 50 ? "..." : "");
			}
		} catch (error) {
			console.warn("Could not get preview text:", error);
		}
		return "Text excerpt";
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
			detail: { threadId: thread.id, position: thread.position },
		});
		document.dispatchEvent(highlightTextEvent);

		// Also trigger the expand/select logic
		handleToggleExpand();
	}

	function handleAddReply() {
		const trimmedReply = replyText.trim();
		if (!trimmedReply) return;

		try {
			let coordinator = dataState.getNotesCoordinator();
			let commentService = coordinator?.getCommentsStore();
			commentService?.addComment(thread.id, sanitize(trimmedReply));
			replyText = "";
			isAddingReply = false;
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
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="group rounded-md overflow-hidden transition-all duration-75 hover:bg-osvauld-frameblack hover:border-osvauld-defaultBorder {isSelected
		? 'bg-osvauld-frameblack border-osvauld-defaultBorder'
		: ''} {thread.resolved ? 'opacity-70' : ''} {isHighlighted
		? 'animate-pulse'
		: ''}"
>
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="p-3 cursor-pointer flex items-start gap-2 relative"
		onclick={handleThreadClick}
	>
		<div class="flex-1 min-w-0">
			<div class="flex justify-start items-center gap-2">
				<span
					class="w-11 h-11 flex justify-center items-center rounded-full text-commentThreadNameInitial border border-collaboratorBorder group-hover:border-osvauld-sideListTextActive group-hover:text-osvauld-sideListTextActive transition-all duration-75"
					>{mainComment.author.name.charAt(0).toUpperCase()}</span
				>
				<div class="flex flex-col items-start">
					<span class="capitalize text-white text-sm font-medium tracking-wider"
						>{mainComment.author.name}</span
					>
					<span class="text-xs text-statusColor"
						>{formatTimestamp(mainComment.timestamp)}</span
					>
				</div>
			</div>
			<div
				class="text-[11px] my-2.5 italic border-l-2 border-livnotePink pl-1.5 flex items-center justify-start text-statusColor"
			>
				<span class="max-w-full overflow-hidden text-ellipsis whitespace-nowrap"
					>"{previewText}"</span
				>
			</div>
			<div
				class="text-[13px] text-white leading-relaxed max-w-full overflow-hidden text-ellipsis whitespace-nowrap {isExpanded
					? 'whitespace-normal overflow-visible text-ellipsis-clip break-words'
					: ''}"
			>
				<span>{mainComment.content}</span>
			</div>
			{#if commentCount > 1}
				<div class="text-xs text-statusColor">
					{commentCount} replies
				</div>
			{/if}
		</div>
		<div
			class="absolute top-3 right-2 flex items-start gap-1 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
		>
			{#if thread.resolved}
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
					onDelete();
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
		<div class="p-2 border-t border-osvauld-defaultBorder text-xs">
			{#if replies.length > 0}
				<div class="mt-3 ml-2 pl-1 border-l border-osvauld-defaultBorder">
					{#each replies as reply (reply.id)}
						<div class="mb-3 pl-2 border-b border-osvauld-defaultBorder">
							<div class="flex justify-start items-center gap-2">
								<span
									class="w-9 h-9 flex justify-center items-center rounded-full text-commentThreadNameInitial border border-collaboratorBorder"
									>{mainComment.author.name.charAt(0).toUpperCase()}</span
								>
								<div class="flex flex-col items-start">
									<span
										class="capitalize text-white text-sm font-medium tracking-wider"
										>{reply.author.name}</span
									>
									<span class="text-xs text-statusColor"
										>{formatTimestamp(reply.timestamp)}</span
									>
								</div>
							</div>
							<div
								class="text-[13px] text-white leading-relaxed my-2 break-words"
							>
								{reply.content}
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
						maxlength="150"
						onkeydown={(e) => {
							if (e.key === "Enter" && !e.shiftKey) {
								e.preventDefault();
								handleAddReply();
							}
						}}
					></textarea>
					<div class="flex gap-2 justify-between items-center">
						<span class="text-xs text-statusColor">{replyText.length}/150</span>
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
				<div class="flex justify-end">
					<button
						class="flex items-center gap-2 bg-livnotePink text-primarydark px-2 py-1 rounded-sm text-xs transition cursor-pointer"
						onclick={() => (isAddingReply = true)}
					>
						<ReplyIcon />
						Reply
					</button>
				</div>
			{/if}
		</div>
	{/if}
</div>

<div class="w-full h-px bg-[#2a2b2f] my-1.5"></div>
