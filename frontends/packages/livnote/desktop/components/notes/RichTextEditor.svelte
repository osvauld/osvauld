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
	let error = $state<string | null>(null);
	let currentlyLoadedNoteId = $state<string | null>(null);
	let loadingInProgress = $state(false);
	let elementWidth = $state<number | undefined>(undefined);
	let resizeTimeoutId: number | null = null;
	let showCommentModal = $state(false);
	let modalSelectedText = $state("");
	let pendingCommentPosition = $state<{ from: number; to: number } | null>(
		null,
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

	async function loadNote(id: string): Promise<void> {
		if (!element || loadingInProgress) return;

		loadingInProgress = true;

		// Clear any existing content and show loading state
		if (view) {
			view.destroy();
			view = null;
		}

		isLoading = true;
		error = null;

		try {
			// Load the note with the given ID
			const docInfo = await notesInstance.loadNote();

			// Force a small delay to ensure DOM is ready
			await new Promise((resolve) => setTimeout(resolve, 50));

			if (currentlyLoadedNoteId === id) {
				return;
			}
			currentlyLoadedNoteId = id;

			// Create editor view with the loaded content
			if (docInfo.editorState) {
				view = createEditorView(element, docInfo.editorState);

				// Mark this note as loaded
				currentlyLoadedNoteId = id;
				setTimeout(() => {
					notesInstance.applyPendingYjsState(view);
					if (dataState.currentNote && dataState.currentNote.data) {
						dataState.currentNote.data.title = notesInstance.getCurrentTitle();
					}
				}, 50);

				setTimeout(() => {
					if (view) {
						try {
							// Force focus on the editor
							view.focus();

							// Create a transaction to position the cursor at the end
							const tr = view.state.tr;

							// Get the end position of the document
							const endPosition = tr.doc.content.size;

							// Set the selection at the end position
							const selection = view.state.selection.constructor as any;
							tr.setSelection(
								selection.near(tr.doc.resolve(Math.max(0, endPosition))),
							);

							// Dispatch the transaction with a custom "cursorPlacement" metadata
							view.dispatch(tr.setMeta("cursorPlacement", true));
						} catch (err) {
							console.error("Error positioning cursor:", err);
						}
					}
				}, 100);
			}

			// Setup auto-save
			if (autoSaveInterval) {
				clearInterval(autoSaveInterval);
			}

			autoSaveInterval = window.setInterval(() => {
				if (dataState.currentNote) {
					notesInstance.saveNote().catch(console.error);

					// Below state is set for showing saved update
					uiState.noteSaved = true;

					setTimeout(() => {
						uiState.noteSaved = false;
					}, 1000);
				}
			}, 30000); // Auto-save every 30 seconds

			// Force an update to ensure content is rendered
			if (view) {
				const tr = view.state.tr;
				view.dispatch(tr);
			}
		} catch (err) {
			console.error("Error loading note:", err);
			error = `Failed to load note: ${err instanceof Error ? err.message : String(err)}`;
		} finally {
			isLoading = false;
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

		if (currentNoteId && currentNoteId !== currentlyLoadedNoteId) {
			loadNote(currentNoteId);
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
</style>

<div class="editor-container">
	<div class="editor-content-wrapper">
		<div class="editor-main scrollbar-thin">
			{#if isLoading}
				<div
					class="loading-overlay flex justify-center items-center h-full w-full">
					<div class="text-osvauld-fieldText">Loading note...</div>
				</div>
			{:else if error}
				<div class="error-message">{error}</div>
			{/if}

			<div
				bind:this={element}
				class="h-full max-h-full overflow-y-scroll scrollbar-thin">
			</div>
		</div>
	</div>
</div>

<CommentModal
	isVisible={showCommentModal}
	selectedText={modalSelectedText}
	onSave={handleSaveComment}
	onCancel={handleCancelComment} />
