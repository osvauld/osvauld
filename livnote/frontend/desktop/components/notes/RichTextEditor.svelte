<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { EditorView } from "prosemirror-view";
	import type { EditorState } from "prosemirror-state";
	import type { UnlistenFn } from "@tauri-apps/api/event";
	import { notesInstance } from "./notes";
	import { dataState, uiState } from "../../state";
	import { DOMSerializer } from "prosemirror-model";
	import CommentModal from "./CommentModal.svelte";
	import "./rich-text-editor.css";

	// Event dispatcher for collaboration updates

	// Local state using $state
	let element = $state<HTMLElement | null>(null);
	let view = $state<EditorView | null>(null);
	let autoSaveInterval: number | null = null;
	let unsubscribeUpdate = $state<UnlistenFn | null>(null);
	let isLoading = $state(true);
	let currentlyLoadedNoteId = $state<string | null>(null);
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

	const copyContentListener = (event: Event): void => {
		if (!view) return;

		try {
			// Get the schema from the document
			const { schema } = notesInstance.getDoc();

			// Create a serializer with this schema
			const serializer = DOMSerializer.fromSchema(schema);

			// Create a document fragment
			const fragment = view.state.doc.content;

			// Create a container for the HTML
			const domFragment = document.createElement("div");

			// Serialize the fragment to HTML
			serializer.serializeFragment(fragment, { document }, domFragment);

			// Get both HTML and plain text versions
			const html = domFragment.innerHTML;
			const text = domFragment.textContent || "";

			// Use the Clipboard API to copy with formatting
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
	// Replace the existing loadNote function with this phased version:
	async function loadNote(id: string): Promise<void> {
		if (!element || loadingInProgress || currentlyLoadedNoteId === id) {
			return;
		}

		console.log(`[EDITOR] Starting phased loading for note ${id}`);
		loadingInProgress = true;
		currentlyLoadedNoteId = id;

		try {
			// Phase 1: Immediate - Show layout and prepare
			loadingPhase = "preparing";
			error = null;

			// Small delay to let the skeleton render
			await new Promise((resolve) => setTimeout(resolve, 50));

			// Phase 2: Initialize structure
			loadingPhase = "structure-ready";
			const docInfo = await notesInstance.loadNote();

			if (!docInfo.editorState) {
				throw new Error("Failed to initialize editor state");
			}

			// Phase 3: Create editor view
			if (view) {
				view.destroy();
				view = null;
			}

			view = createEditorView(element, docInfo.editorState);

			// Small delay to let editor render
			await new Promise((resolve) => setTimeout(resolve, 100));

			// Phase 4: Apply actual content
			loadingPhase = "content-loaded";
			notesInstance.applyPendingYjsState(view);

			// Update title in state
			if (dataState.currentNote && dataState.currentNote.data) {
				dataState.currentNote.data.title = notesInstance.getCurrentTitle();
			}

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

			// Setup auto-save (existing code)
			if (autoSaveInterval) {
				clearInterval(autoSaveInterval);
			}

			autoSaveInterval = window.setInterval(() => {
				if (dataState.currentNote) {
					notesInstance.saveNote().catch(console.error);
					uiState.noteSaved = true;
					setTimeout(() => {
						uiState.noteSaved = false;
					}, 1000);
				}
			}, 30000);
		} catch (err) {
			console.error("Error loading note:", err);
			error = `Failed to load note: ${err instanceof Error ? err.message : String(err)}`;
			loadingPhase = "error";
		} finally {
			loadingInProgress = false;
		}
	}

	function createEditorView(
		element: HTMLElement,
		state: EditorState,
	): EditorView {
		const { ydoc } = notesInstance.getDoc();

		const dispatchTransaction = async (tr: any) => {
			if (!view) return;

			try {
				const newState = view.state.apply(tr);
				view.updateState(newState);

				// Skip the update cycle for cursor placement transactions
				if (tr.getMeta("cursorPlacement")) {
					return;
				}
				notesInstance.updateEditorState(newState);
				notesInstance.setEditorView(view);
			} catch (err) {
				console.error("Error in dispatch transaction:", err);
			}
		};

		return new EditorView(element, {
			state,
			dispatchTransaction,
		});
	}

	function cleanupEditor(): void {
		currentlyLoadedNoteId = null;
		loadingPhase = "idle";
		if (unsubscribeUpdate) {
			unsubscribeUpdate();
		}
		if (view) {
			notesInstance.setEditorView(null);
			view.destroy();
			view = null;
		}
		if (autoSaveInterval) {
			clearInterval(autoSaveInterval);
		}

		// Save before cleanup if we have a note loaded
		if (currentlyLoadedNoteId && dataState.currentNote) {
			notesInstance.saveNote().catch(console.error);
		}

		currentlyLoadedNoteId = null;
	}

	// Check if window is too narrow for both panels, with debouncing
	function checkWindowSize() {
		if (resizeTimeoutId) {
			clearTimeout(resizeTimeoutId);
		}
		resizeTimeoutId = window.setTimeout(() => {
			// Still capture elementWidth, might be useful for other things or logging
			elementWidth = element?.getBoundingClientRect().width;

			const currentWindowWidth = window.innerWidth;
			const NAV_PANEL_APPROX_WIDTH = 360; // Based on prior comments in file

			// Only auto-collapse if not manually toggled
			if (!uiState.isNavigationPanelManuallyToggled) {
				// The threshold is the space needed for the nav panel plus the min space for the editor
				const thresholdToShowNav =
					NAV_PANEL_APPROX_WIDTH + uiState.MIN_EDITOR_WIDTH;

				uiState.showNavigationPanel = currentWindowWidth >= thresholdToShowNav;
			}
			resizeTimeoutId = null; // Clear the ID after execution
		}, 50); // User updated delay to 50ms
	}

	$effect(() => {
		const currentNoteId = dataState.currentNote?.id;
		const hasNoteData = dataState.currentNote?.data;

		// Only load if we have both ID and data
		if (
			currentNoteId &&
			hasNoteData &&
			currentNoteId !== currentlyLoadedNoteId
		) {
			loadNote(currentNoteId);
		} else if (!currentNoteId) {
			// Handle note clearing
			loadingPhase = "idle";
			currentlyLoadedNoteId = null;
		}
	});

	// Initialize when component mounts
	onMount(async () => {
		// Clear any state to ensure clean start
		currentlyLoadedNoteId = null;

		document.addEventListener(
			"request-editor-content",
			copyContentListener as EventListener,
		);

		// Add comment modal event listeners
		document.addEventListener(
			"open-comment-modal",
			handleOpenCommentModal as EventListener,
		);

		// Add comment click detection
		document.addEventListener("click", handleCommentClick);

		// Add comment text highlighting
		document.addEventListener(
			"highlight-comment-text",
			handleHighlightCommentText as EventListener,
		);

		window.addEventListener("resize", checkWindowSize);
		checkWindowSize(); // Initial check
	});

	// Clean up when component is destroyed
	onDestroy(() => {
		cleanupEditor();
		document.removeEventListener(
			"request-editor-content",
			copyContentListener as EventListener,
		);

		// Remove comment modal event listeners
		document.removeEventListener(
			"open-comment-modal",
			handleOpenCommentModal as EventListener,
		);

		// Remove comment click detection
		document.removeEventListener("click", handleCommentClick);

		// Remove comment text highlighting
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
		if (!pendingCommentPosition || !view) {
			console.error("No pending comment position or view");
			return;
		}
		try {
			// Use the new combined method from notesInstance
			notesInstance.createCommentAndApplyMark(pendingCommentPosition, content);
		} catch (error) {
			console.error("Error creating comment:", error);
		}
		// Reset modal state
		showCommentModal = false;
		modalSelectedText = "";
		pendingCommentPosition = null;

		// Refocus editor
		if (view) {
			view.focus();
		}
	}

	function handleCancelComment() {
		showCommentModal = false;
		modalSelectedText = "";
		pendingCommentPosition = null;
		// Refocus editor
		if (view) {
			view.focus();
		}
	}

	function handleCommentClick(event: MouseEvent) {
		const target = event.target as HTMLElement;

		// Check if the clicked element has a comment mark
		const commentElement = target.closest("[data-livnote-comment]");
		if (commentElement) {
			const threadId = commentElement.getAttribute("data-livnote-comment");
			if (threadId) {
				// Dispatch event to highlight the comment in sidebar
				const highlightEvent = new CustomEvent("highlight-comment-thread", {
					detail: { threadId },
				});
				document.dispatchEvent(highlightEvent);
			}
		}
	}

	function handleHighlightCommentText(event: CustomEvent) {
		const { threadId, position } = event.detail;
		// Find the comment span in the editor
		const commentSpan = document.querySelector(
			`[data-livnote-comment="${threadId}"]`,
		);
		if (commentSpan) {
			// Scroll to the comment if not visible
			commentSpan.scrollIntoView({
				behavior: "smooth",
				block: "center",
				inline: "nearest",
			});

			// Add highlight animation class
			commentSpan.classList.add("comment-text-highlight");

			// Remove the class after animation completes
			setTimeout(() => {
				commentSpan.classList.remove("comment-text-highlight");
			}, 3000);
		}
	}
</script>

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
