<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { EditorView } from "prosemirror-view";
	import { emit, type UnlistenFn } from "@tauri-apps/api/event";
	import { NotesCoordinator } from "./notesCoordinator";
	import { dataState, uiState } from "../../state";
	import { DOMSerializer } from "prosemirror-model";
	import CommentModal from "./CommentModal.svelte";
	import "./rich-text-editor.css";
	import "./schema/editorCustomStyles.css"; // Import the new CSS file

	// Initialize the coordinator

	// Local state using $state
	let element = $state<HTMLElement | null>(null);
	let view = $state<EditorView | null>(null);
	let autoSaveInterval: number | null = null;
	let unsubscribeUpdate = $state<UnlistenFn | null>(null);
	let isLoading = $state(true);
	let loadingInProgress = $state(false);
	let elementWidth = $state<number | undefined>(undefined);
	let resizeTimeoutId: number | null = null;
	let showCommentModal = $state(false);
	let modalSelectedText = $state("");
	let pendingCommentPosition = $state<{ from: number; to: number } | null>(
		null,
	);

	type LoadingPhase =
		| "idle"
		| "preparing"
		| "structure-ready"
		| "content-loaded"
		| "ready"
		| "error";
	let loadingPhase = $state<LoadingPhase>("idle");
	let error = $state<string | null>(null);

	let showSkeleton = $derived(
		loadingPhase === "preparing" || loadingPhase === "structure-ready",
	);
	let showError = $derived(loadingPhase === "error");
	let showContent = $derived(
		loadingPhase === "content-loaded" || loadingPhase === "ready",
	);

	$effect(() => {
		if (dataState.currentNoteId) {
			loadNote().then(() => {
				console.log("load complete");
			});
		}
	});

	const copyContentListener = (event: Event): void => {
		if (!view) return;

		try {
			const serializer = DOMSerializer.fromSchema(view.state.schema);
			const fragment = view.state.doc.content;
			const domFragment = document.createElement("div");
			serializer.serializeFragment(fragment, { document }, domFragment);

			const html = domFragment.innerHTML;
			const text = domFragment.textContent || "";

			if (navigator.clipboard && window.ClipboardItem) {
				navigator.clipboard
					.write([
						new ClipboardItem({
							"text/html": new Blob([html], { type: "text/html" }),
							"text/plain": new Blob([text], { type: "text/plain" }),
						}),
					])
					.catch((err) => {
						console.error("Clipboard API error:", err);
					});
			}
		} catch (error) {
			console.error("Error during copy:", error);
		}
	};

	async function loadNote(): Promise<void> {
		const coordinator = dataState.getNotesCoordinator();
		if (!coordinator) {
			throw new Error("Coordinator not initialized in dataState");
		}

		loadingInProgress = true;

		try {
			// Phase 1: Show skeleton
			loadingPhase = "preparing";
			error = null;
			await new Promise((resolve) => setTimeout(resolve, 50));

			// Phase 2: Initialize structure
			loadingPhase = "structure-ready";

			// Load note content
			const noteContent = dataState.getCurrentNoteData()?.data;
			if (!noteContent) {
				throw new Error("No note content available");
			}

			await coordinator.loadNote(noteContent);

			// Phase 3: Create editor view
			if (view) {
				view.destroy();
				view = null;
			}

			view = coordinator.createEditorView(element);
			await new Promise((resolve) => setTimeout(resolve, 100));

			// Phase 4: Content loaded
			loadingPhase = "content-loaded";

			// Phase 5: Finalize
			setTimeout(() => {
				if (view) {
					try {
						view.focus();
						const tr = view.state.tr;
						const endPosition = tr.doc.content.size;
						const selection = view.state.selection.constructor as any;
						tr.setSelection(
							selection.near(tr.doc.resolve(Math.max(0, endPosition))),
						);
						view.dispatch(tr.setMeta("cursorPlacement", true));
						loadingPhase = "ready";
					} catch (err) {
						console.error("Error positioning cursor:", err);
					}
				}
			}, 200);
		} catch (err) {
			console.error("Error loading note:", err);
			error = `Failed to load note: ${err instanceof Error ? err.message : String(err)}`;
			loadingPhase = "error";
		} finally {
			loadingInProgress = false;
		}
	}

	function cleanupEditor(): void {
		loadingPhase = "idle";
		if (unsubscribeUpdate) {
			unsubscribeUpdate();
		}

		setTimeout(() => {
			if (view) {
				view.destroy();
				view = null;
			}
		}, 0);

		if (autoSaveInterval) {
			clearInterval(autoSaveInterval);
		}
	}

	function checkWindowSize() {
		if (resizeTimeoutId) {
			clearTimeout(resizeTimeoutId);
		}
		resizeTimeoutId = window.setTimeout(() => {
			elementWidth = element?.getBoundingClientRect().width;
			const currentWindowWidth = window.innerWidth;
			const NAV_PANEL_APPROX_WIDTH = 360;

			if (!uiState.isNavigationPanelManuallyToggled) {
				const thresholdToShowNav =
					NAV_PANEL_APPROX_WIDTH + uiState.MIN_EDITOR_WIDTH;
				uiState.showNavigationPanel = currentWindowWidth >= thresholdToShowNav;
			}
			resizeTimeoutId = null;
		}, 50);
	}

	onMount(async () => {
		document.addEventListener(
			"request-editor-content",
			copyContentListener as EventListener,
		);
		document.addEventListener(
			"open-comment-modal",
			handleOpenCommentModal as EventListener,
		);
		document.addEventListener("click", handleCommentClick);
		document.addEventListener(
			"highlight-comment-text",
			handleHighlightCommentText as EventListener,
		);

		window.addEventListener("resize", checkWindowSize);
		checkWindowSize();
	});

	onDestroy(() => {
		cleanupEditor();

		document.removeEventListener(
			"request-editor-content",
			copyContentListener as EventListener,
		);
		document.removeEventListener(
			"open-comment-modal",
			handleOpenCommentModal as EventListener,
		);
		document.removeEventListener("click", handleCommentClick);
		document.removeEventListener(
			"highlight-comment-text",
			handleHighlightCommentText as EventListener,
		);

		window.removeEventListener("resize", checkWindowSize);
		if (resizeTimeoutId) {
			clearTimeout(resizeTimeoutId);
		}
	});

	function handleOpenCommentModal(event: CustomEvent) {
		const { selectedText, position } = event.detail;
		modalSelectedText = selectedText;
		pendingCommentPosition = position;
		showCommentModal = true;
	}

	function handleSaveComment(content: string) {
		if (!pendingCommentPosition) {
			console.error("No pending comment position or coordinator");
			return;
		}

		const coordinator = dataState.getNotesCoordinator();
		if (!coordinator) {
			console.error("No coordinator available");
			return;
		}
		try {
			coordinator.createComment(pendingCommentPosition, content);
		} catch (error) {
			console.error("Error creating comment:", error);
		}

		showCommentModal = false;
		modalSelectedText = "";
		pendingCommentPosition = null;

		if (view) {
			view.focus();
		}
	}

	function handleCancelComment() {
		showCommentModal = false;
		modalSelectedText = "";
		pendingCommentPosition = null;

		if (view) {
			view.focus();
		}
	}

	function handleCommentClick(event: MouseEvent) {
		const target = event.target as HTMLElement;
		const commentElement = target.closest("[data-livnote-comment]");

		if (commentElement) {
			const threadId = commentElement.getAttribute("data-livnote-comment");
			if (threadId) {
				const highlightEvent = new CustomEvent("highlight-comment-thread", {
					detail: { threadId },
				});
				document.dispatchEvent(highlightEvent);
			}
		}
	}

	function handleHighlightCommentText(event: CustomEvent) {
		const { threadId } = event.detail;
		const commentSpan = document.querySelector(
			`[data-livnote-comment="${threadId}"]`,
		);

		if (commentSpan) {
			commentSpan.scrollIntoView({
				behavior: "smooth",
				block: "center",
				inline: "nearest",
			});

			commentSpan.classList.add("comment-text-highlight");
			setTimeout(() => {
				commentSpan.classList.remove("comment-text-highlight");
			}, 3000);
		}
	}
</script>

<!-- Rest of the component remains the same -->
<style>
	/* Basic editor container structure */
	.editor-container {
		margin: 0 auto;
		width: 100%;
		height: 100%;
		background: #16171f;
		color: white;
		position: relative;
		border-radius: 1rem;
		display: flex;
		flex-direction: column;
		overflow: hidden; /* Prevent container from growing */
	}

	.editor-content-wrapper {
		flex: 1;
		display: flex;
		overflow: hidden;
	}

	.editor-main {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow-y: auto;
		margin: 5px 15px 5px 15px;
		max-height: 100%; /* Ensure it doesn't grow beyond container */
		min-width: 0; /* Allow flexbox to shrink */
		position: relative;
	}

	/* Add styles for the editor content */
	:global(.ProseMirror) {
		min-height: 100%;
		height: fit-content;
		overflow-wrap: break-word;
		word-wrap: break-word;
		word-break: break-word;
	}
	@keyframes shimmer {
		0% {
			background-position: -200px 0;
		}
		100% {
			background-position: calc(200px + 100%) 0;
		}
	}

	.animate-pulse {
		animation: shimmer 2s ease-in-out infinite;
	}

	.animate-pulse > div {
		background: linear-gradient(90deg, #2a2b35 25%, #3a3b44 37%, #2a2b35 63%);
		background-size: 400px 100%;
		animation: shimmer 1.5s ease-in-out infinite;
	}
</style>

<div class="editor-container">
	<div class="editor-content-wrapper">
		<div class="editor-main scrollbar-thin">
			<!-- Always show the editor element, but overlay different states -->
			<div
				bind:this={element}
				class="h-full max-h-full overflow-y-scroll scrollbar-thin"
				class:opacity-0={showSkeleton}
				class:opacity-100={showContent}>
			</div>

			<!-- Skeleton overlay -->
			{#if showSkeleton}
				<div class="absolute inset-0 p-6 space-y-4">
					<div class="animate-pulse space-y-6">
						<!-- Title skeleton -->
						<div class="h-8 bg-osvauld-fieldActive rounded-lg w-3/4"></div>

						<!-- Content skeletons -->
						<div class="space-y-3">
							<div class="h-4 bg-osvauld-fieldActive rounded w-full"></div>
							<div class="h-4 bg-osvauld-fieldActive rounded w-5/6"></div>
							<div class="h-4 bg-osvauld-fieldActive rounded w-4/5"></div>
						</div>

						<div class="space-y-3">
							<div class="h-4 bg-osvauld-fieldActive rounded w-full"></div>
							<div class="h-4 bg-osvauld-fieldActive rounded w-3/4"></div>
						</div>

						<div class="space-y-3">
							<div class="h-4 bg-osvauld-fieldActive rounded w-5/6"></div>
							<div class="h-4 bg-osvauld-fieldActive rounded w-full"></div>
							<div class="h-4 bg-osvauld-fieldActive rounded w-2/3"></div>
						</div>
					</div>

					<!-- Loading phase indicator -->
					<div class="absolute bottom-4 left-6 text-osvauld-fieldText text-sm">
						{#if loadingPhase === "preparing"}
							Preparing document...
						{:else if loadingPhase === "structure-ready"}
							Loading content...
						{/if}
					</div>
				</div>
			{/if}

			<!-- Error overlay -->
			{#if showError}
				<div class="absolute inset-0 flex justify-center items-center">
					<div class="text-red-400 text-center">
						<div class="text-lg font-medium mb-2">Failed to load note</div>
						<div class="text-sm text-osvauld-fieldText">{error}</div>
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<CommentModal
	isVisible={showCommentModal}
	selectedText={modalSelectedText}
	onSave={handleSaveComment}
	onCancel={handleCancelComment} />
