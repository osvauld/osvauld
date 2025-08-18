<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { EditorView } from "prosemirror-view";
	import { emit, type UnlistenFn } from "@tauri-apps/api/event";
	import { dataState, uiState } from "../../state";
	import { DOMSerializer } from "prosemirror-model";
	import CommentModal from "./CommentModal.svelte";
	import "./rich-text-editor.css";
	import "./schema/editorCustomStyles.css"; // Import the new CSS file
	import "./setup/tableStyles.css";

	// Local state using $state
	let element = $state<HTMLElement | null>(null);
	let view = $state<EditorView | null>(null);
	let autoSaveInterval: number | null = null;
	let unsubscribeUpdate = $state<UnlistenFn | null>(null);
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
	let showError = $derived(loadingPhase === "error");
	let showSkeleton = $derived(
		uiState.isNoteLoading ||
			loadingPhase === "preparing" ||
			loadingPhase === "structure-ready",
	);

	let showContent = $derived(
		!uiState.isNoteLoading &&
			(loadingPhase === "content-loaded" || loadingPhase === "ready"),
	);
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
		loadingPhase = "preparing";
		error = null;

		try {
			const noteContent = dataState.getCurrentNoteData()?.data;
			if (!noteContent) {
				throw new Error("No note content available");
			}
			// Start loading - events will handle the rest
			coordinator.loadNote(noteContent);
		} catch (err) {
			console.error("Error loading note:", err);
			error = `Failed to load note: ${err instanceof Error ? err.message : String(err)}`;
			loadingPhase = "error";
		} finally {
			loadingInProgress = false;
		}
	}
	function handleEditorViewReady(event: CustomEvent) {
		loadingPhase = "content-loaded";

		const getEditorManager = event.detail.getEditorManager;
		if (element && getEditorManager) {
			const editorManager = getEditorManager();
			view = editorManager.createView(element);
			loadingPhase = "ready";
			uiState.setEditorLoading(false);

			// Set up auto-save interval (10 seconds)
			if (autoSaveInterval) {
				clearInterval(autoSaveInterval);
			}
			autoSaveInterval = setInterval(() => {
				if (dataState.currentNoteId && view) {
					dataState.saveNote(dataState.currentNoteId);
				}
			}, 10000);

			// Focus editor
			setTimeout(() => {
				if (view) {
					view.focus();
					const tr = view.state.tr;
					const endPosition = tr.doc.content.size;
					const selection = view.state.selection.constructor as any;
					tr.setSelection(
						selection.near(tr.doc.resolve(Math.max(0, endPosition))),
					);
					view.dispatch(tr.setMeta("cursorPlacement", true));
				}
			}, 100);
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
	$effect(() => {
		if (dataState.currentNoteId) {
			// Set editor loading in UI state
			cleanupEditor();
			uiState.setEditorLoading(true);

			// Set local loading phases for skeleton
			loadingPhase = "preparing";
			loadingInProgress = true;
			error = null;

			setTimeout(async () => {
				loadingPhase = "structure-ready";

				try {
					await loadNote();
				} catch (err) {
					console.error("❌ loadNote failed:", err);
					error = `Failed to load note: ${err instanceof Error ? err.message : String(err)}`;
					loadingPhase = "error";
					uiState.setEditorLoading(false); // Clear on error
				} finally {
					loadingInProgress = false;
				}
			}, 0);
		}
	});
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
		document.addEventListener(
			"editor-view-ready",
			handleEditorViewReady as EventListener,
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
		document.removeEventListener(
			"editor-view-ready",
			handleEditorViewReady as EventListener,
		);
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

<style>
	.editor-container {
		width: min(90vw, 900px);
		max-width: 900px;
		margin: 0 auto;
		height: 100%;
		background: #16171f;
		color: white;
		position: relative;
		border-radius: 1rem;
		display: flex;
		flex-direction: column;
		overflow: hidden; /* Prevent container from growing */
	}

	@media print, (export-mode: true) {
		.editor-container {
			width: 180mm !important;
			transform: scale(0.95);
			transform-origin: top left;
		}

		.ProseMirror {
			font-size: 11pt !important;
			line-height: 1.3 !important;
		}
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
		position: relative;
	}

	:global(.ProseMirror p.is-empty) {
		position: relative;
	}

	:global(.ProseMirror p.is-empty::before) {
		content: attr(data-placeholder);
		color: #85889c;
		pointer-events: none;
		font-style: italic;
		position: absolute;
		top: 50%;
		transform: translateY(-50%);
		font-size: 16px;
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
				class:opacity-100={showContent}
			></div>

			<!-- Skeleton overlay -->
			{#if showSkeleton}
				<div class="absolute inset-0 p-6 space-y-4 bg-[#16171f]">
					<div class="animate-pulse space-y-6">
						<div class="text-white text-sm mb-4 p-2 rounded">
							{#if uiState.isNoteFetching}
								decrypting note ...
							{:else if uiState.isEditorLoading}
								Setting up editor...
							{:else}
								Loading...
							{/if}
						</div>

						<!-- Title skeleton -->
						<div class="h-8 bg-gray-600 rounded-lg w-3/4"></div>

						<div class="h-4 bg-gray-600 rounded w-5/6"></div>
						<div class="h-4 bg-gray-600 rounded w-4/5"></div>

						<div class="h-4 bg-gray-600 rounded w-5/6"></div>
						<div class="h-4 bg-gray-600 rounded w-4/5"></div>

						<div class="h-24 bg-gray-600 rounded w-5/6"></div>
						<div class="h-4 bg-gray-600 rounded w-4/5"></div>
						<div class="h-4 bg-gray-600 rounded w-full"></div>
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
	onCancel={handleCancelComment}
/>
