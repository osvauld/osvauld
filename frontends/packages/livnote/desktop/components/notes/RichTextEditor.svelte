<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { EditorView } from "prosemirror-view";
	import type { EditorState } from "prosemirror-state";
	import { listen } from "@tauri-apps/api/event";
	import type { UnlistenFn } from "@tauri-apps/api/event";
	import { notesInstance } from "./notes";
	import { dataState, uiState } from "../../state";
	import { DOMSerializer } from "prosemirror-model";
	import "./rich-text-editor.css";

	// Event dispatcher for collaboration updates

	// Local state using $state
	let element = $state<HTMLElement | null>(null);
	let view = $state<EditorView | null>(null);
	let autoSaveInterval = $state<number | null>(null);
	let unsubscribeUpdate = $state<UnlistenFn | null>(null);
	let isLoading = $state(true);
	let error = $state<string | null>(null);
	let currentlyLoadedNoteId = $state<string | null>(null);
	let loadingInProgress = $state(false);
	let saved = $state(false);

	// Copy content utilities
	const fallbackCopy = (html: string): void => {
		const tempElement = document.createElement("div");
		tempElement.innerHTML = html;
		tempElement.style.position = "absolute";
		tempElement.style.left = "-9999px";
		document.body.appendChild(tempElement);

		// Select the temp element
		const selection = window.getSelection();
		const range = document.createRange();
		range.selectNodeContents(tempElement);
		selection?.removeAllRanges();
		selection?.addRange(range);

		// Execute copy
		document.execCommand("copy");

		// Clean up
		selection?.removeAllRanges();
		document.body.removeChild(tempElement);

		console.log("Note copied using fallback method");
	};

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
					.then(() => {
						console.log("Note copied with formatting");
					})
					.catch((err) => {
						console.error("Clipboard API error:", err);
						// Fallback to the execCommand method
						fallbackCopy(html);
					});
			} else {
				// Use fallback method
				fallbackCopy(html);
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
			console.log("Destroying existing editor view");
			view.destroy();
			view = null;
		}

		isLoading = true;
		error = null;

		try {
			console.log(`Loading note: ${id}`);

			// Load the note with the given ID
			const docInfo = await notesInstance.loadNote(id);

			// Force a small delay to ensure DOM is ready
			await new Promise((resolve) => setTimeout(resolve, 50));

			if (currentlyLoadedNoteId === id) {
				console.log("Note already loaded, skipping");
				return;
			}
			currentlyLoadedNoteId = id;

			// Create editor view with the loaded content
			if (docInfo.editorState) {
				view = createEditorView(element, docInfo.editorState);

				// Mark this note as loaded
				currentlyLoadedNoteId = id;

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
					notesInstance
						.saveNote(dataState.currentNote.data?.title || "Untitled")
						.catch(console.error);

					saved = true;
					setTimeout(() => {
						saved = false;
					}, 1000);
				}
			}, 30000); // Auto-save every 30 seconds

			// Set up listener for sync updates from other peers
			if (unsubscribeUpdate) {
				unsubscribeUpdate();
			}

			unsubscribeUpdate = await listen("sync-update-be", (event) => {
				try {
					const parsed =
						typeof event.payload === "string"
							? JSON.parse(event.payload)
							: event.payload;

					const { update, clientID: remoteClientID } = parsed;

					// Remote update will be applied with 'sync' origin
					notesInstance.applyUpdate(update, remoteClientID);

					// If view exists, force a refresh to show the changes
					if (view) {
						view.updateState(view.state);
					}
				} catch (err) {
					console.error("Error handling update:", err);
				}
			});

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
			} catch (err) {
				console.error("Error in dispatch transaction:", err);
			}
		};

		return new EditorView(element, {
			state,
			dispatchTransaction,
		});
	}

	// Public method to save the note
	export function saveNote(): Promise<void> {
		if (!dataState.currentNote) return Promise.resolve();

		return notesInstance
			.saveNote(dataState.currentNote.data?.title || "Untitled")
			.then(() => {
				dispatch("save-complete", true);
				return Promise.resolve();
			})
			.catch((error) => {
				console.error("Error saving note:", error);
				return Promise.reject(error);
			});
	}

	function cleanupEditor(): void {
		if (unsubscribeUpdate) {
			unsubscribeUpdate();
		}
		if (view) {
			view.destroy();
			view = null;
		}
		if (autoSaveInterval) {
			clearInterval(autoSaveInterval);
		}

		// Save before cleanup if we have a note loaded
		if (currentlyLoadedNoteId && dataState.currentNote) {
			notesInstance
				.saveNote(dataState.currentNote.data?.title || "Untitled")
				.catch(console.error);
		}

		currentlyLoadedNoteId = null;
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
	});

	// Clean up when component is destroyed
	onDestroy(() => {
		console.log("RichTextEditor destroyed");
		cleanupEditor();
		document.removeEventListener(
			"request-editor-content",
			copyContentListener as EventListener,
		);
	});
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

	.editor-main {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow-y: auto;
		margin: 5px 15px 5px 15px;
		max-height: 100%; /* Ensure it doesn't grow beyond container */
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
	<div class="editor-main scrollbar-thin">
		{#if isLoading}
			<div
				class="loading-overlay flex justify-center items-center h-full w-full">
				<div class="text-osvauld-fieldText">Loading note...</div>
			</div>
		{:else if error}
			<div class="error-message">{error}</div>
		{/if}

		<div bind:this={element} class="h-full"></div>
	</div>
</div>
