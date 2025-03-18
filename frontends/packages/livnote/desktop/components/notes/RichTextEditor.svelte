<script>
	import {
		onMount,
		onDestroy,
		createEventDispatcher,
		getContext,
	} from "svelte";
	import { EditorView } from "prosemirror-view";
	import { listen } from "@tauri-apps/api/event";
	import { notesInstance } from "./notes";
	import {
		currentNote,
		noteId,
		noteViewLayout,
		refreshCredentialList,
	} from "../../store/desktop.ui.store";
	import SavedTick from "@osvauld/password-manager-common/icons/savedTick.svelte";
	import { DOMSerializer } from "prosemirror-model";

	const dispatch = createEventDispatcher();
	let element;
	let view = null;
	let autoSaveInterval;
	let unsubscribeUpdate;
	let isLoading = false;
	let error = null;
	let currentlyLoadedNoteId = null;
	let loadingInProgress = false;
	let saved = false;
	const saveNoteAndSwitch = getContext("saveNoteAndSwitchFunction");

	// Listen for noteId changes and load the corresponding note
	$: if (
		$noteId &&
		element &&
		$noteId !== currentlyLoadedNoteId &&
		!loadingInProgress
	) {
		loadNote($noteId);
	}

	saveNoteAndSwitch(() => {
		if (view) {
			notesInstance.saveNote().catch(console.error);
		}

		// Return to list view
		noteViewLayout.set(false);

		// Clear current note ID
		currentlyLoadedNoteId = null;
	});

	async function saveNoteManual() {
		saved = true;
		notesInstance
			.saveNote()
			.catch(console.error)
			.then(() => refreshCredentialList.set(true));

		setTimeout(() => {
			saved = false;
		}, 1000);
	}

	const fallbackCopy = (html) => {
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

	const copyContentListener = (event) => {
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

	async function loadNote(id) {
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

			// If another load operation started while we were waiting, abort
			if (currentlyLoadedNoteId !== null && currentlyLoadedNoteId !== id) {
				console.log("Aborting load - another note was loaded");
				return;
			}

			// Create editor view with the loaded content
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
						tr.setSelection(
							view.state.selection.constructor.near(
								tr.doc.resolve(Math.max(0, endPosition)),
							),
						);

						// Dispatch the transaction with a custom "cursorPlacement" metadata
						view.dispatch(tr.setMeta("cursorPlacement", true));
					} catch (err) {
						console.error("Error positioning cursor:", err);
					}
				}
			}, 100);
			// Setup auto-save
			if (autoSaveInterval) {
				clearInterval(autoSaveInterval);
			}

			autoSaveInterval = setInterval(() => {
				// Savign animation go

				notesInstance.saveNote().catch(console.error);
				saved = true;
				setTimeout(() => {
					saved = false;
				}, 1000);
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
			error = `Failed to load note: ${err.message}`;
		} finally {
			isLoading = false;
			loadingInProgress = false;
		}
	}

	function createEditorView(element, state) {
		const { ydoc } = notesInstance.getDoc();

		const dispatchTransaction = async (tr) => {
			if (!view) return;

			try {
				const newState = view.state.apply(tr);
				view.updateState(newState);

				// Skip the update cycle for cursor placement transactions
				if (tr.getMeta("cursorPlacement")) {
					return;
				}

				notesInstance.updateEditorState(newState);

				// Only trigger Yjs update if document actually changed
				if (tr.docChanged && ydoc) {
					// This will trigger the 'update' event on ydoc with default (local) origin
					// The Notes class will handle sending the update to peers

					// Dispatch event for collaboration - ONLY send minimal data
					// to avoid cyclic structure serialization issues
					dispatch("collaboration-update", {
						noteId: currentlyLoadedNoteId,
					});
				}
			} catch (err) {
				console.error("Error in dispatch transaction:", err);
			}
		};

		return new EditorView(element, {
			state,
			dispatchTransaction,
		});
	}

	const prosemirrorInstanceDestructionHandle = () => {
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
		// Save one final time on destroy
		notesInstance
			.saveNote()
			.catch(console.error)
			.then(() => refreshCredentialList.set(true));

		// Clear current note ID
		currentlyLoadedNoteId = null;
	};

	// Initialize when component mounts
	onMount(async () => {
		console.log("RichTextEditor mounted");
		// Clear any state to ensure clean start
		currentlyLoadedNoteId = null;

		if ($noteId) {
			await loadNote($noteId);
		}

		document.addEventListener("request-editor-content", copyContentListener);
	});

	// We need to do cleanup when noteId Changes

	noteId.subscribe((id) => {
		if (id) prosemirrorInstanceDestructionHandle();
	});

	// Clean up when component is destroyed
	onDestroy(() => {
		console.log("RichTextEditor destroyed");
		prosemirrorInstanceDestructionHandle();
		document.removeEventListener("request-editor-content", copyContentListener);
	});
</script>

<style>
	/* ProseMirror menubar styles for horizontal layout */
	:global(.ProseMirror-menubar-wrapper) {
		height: 100%;
		display: flex;
		flex-direction: column;
	}

	.editor-container {
		margin: 0 auto;
		width: 100%;
		height: 100%;
		background: #16171f;
		color: white;
		position: relative;
		border-radius: 20px;
	}

	:global(.ProseMirror-example-setup-style) {
		position: relative;
		padding: 15px;
		min-height: 100px;
		max-width: 96%;
		outline: none;
		line-height: 1.5;
		color: white;
		background: #16171f;
		border-radius: 20px;
		overflow-y: scroll;
		flex-grow: 1;
		margin: 5px auto 5px auto;
	}

	:global(.ProseMirror-menubar) {
		min-height: 92px;
		padding: 4px 24px;
		white-space: nowrap;
		overflow-y: hidden;
		background: #16171f;
		display: flex;
		align-items: center;
		gap: 1px;
		border-bottom: 1px solid #2a2b2f;
		border-top-left-radius: 20px;
		border-top-right-radius: 20px;
	}

	:global(.ProseMirror-menuitem) {
		display: inline-flex;
		align-items: center;
		height: 24px;
		margin-right: 4px;
		cursor: pointer;
	}

	:global(.ProseMirror-menu-dropdown) {
		vertical-align: middle;
		padding: 2px 4px;
		font-size: 14px;
		color: white;
	}

	:global(.ProseMirror-menu-dropdown-wrap) {
		position: relative;
		display: inline-block;
	}

	:global(.ProseMirror-menu-dropdown-menu) {
		position: fixed;
		background: #16171f;
		border: 1px solid #2a2b2f;
		border-radius: 2px;
		padding: 2px 0;
		min-width: 67px;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
	}

	:global(.ProseMirror-menu-dropdown-item) {
		padding: 2px 8px;
		cursor: pointer;
		font-size: 14px;
		color: white;
		position: relative;
	}

	:global(.ProseMirror-menu-dropdown-item:hover) {
		background: #2a2b2f;
	}

	:global(.ProseMirror-menu-submenu) {
		position: absolute;
		right: -70px;
		top: 0;
		background: #16171f;
		border: 1px solid #2a2b2f;
		border-radius: 2px;
		padding: 2px 0;
		min-width: 67px;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
	}
	:global(.ProseMirror-icon) {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 2px;
		cursor: pointer;
		border: 1px solid transparent;
		border-radius: 2px;
		font-size: 16px;
		color: white;
	}

	:global(.ProseMirror-icon svg) {
		fill: currentColor;
		color: white;
	}

	:global(.ProseMirror-icon:hover) {
		background: #2a2b2f;
	}

	:global(.ProseMirror-menu-disabled) {
		opacity: 0.3;
	}

	:global(.ProseMirror-icon span) {
		color: white;
		font-weight: bold;
	}

	:global(.ProseMirror-menu-dropdown-item:hover) {
		background: #2a2b2f;
	}

	:global(.ProseMirror-icon) {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 2px;
		cursor: pointer;
		border: 1px solid transparent;
		border-radius: 2px;
		font-size: 16px;
		color: white;
	}

	:global(.ProseMirror-icon:hover) {
		background: #2a2b2f;
	}

	:global(.ProseMirror) {
		position: relative;
		padding: 15px;
		min-height: 100px;
		outline: none;
		line-height: 1.5;
		color: white;
	}

	:global(.ProseMirror p) {
		margin: 0 0 1em 0;
	}

	:global(.ProseMirror h1) {
		font-size: 2em;
		margin: 0.67em 0;
		color: white;
	}

	:global(.ProseMirror h1) {
		font-size: 2em;
		margin: 0.67em 0;
		color: white;
		font-weight: bold;
	}

	:global(.ProseMirror h2) {
		font-size: 1.5em;
		margin: 0.83em 0;
		color: white;
		font-weight: bold;
	}

	:global(.ProseMirror h3) {
		font-size: 1.17em;
		margin: 1em 0;
		color: white;
		font-weight: bold;
	}

	:global(.ProseMirror h4) {
		font-size: 1em;
		margin: 1.33em 0;
		color: white;
		font-weight: bold;
	}

	:global(.ProseMirror h5) {
		font-size: 0.83em;
		margin: 1.67em 0;
		color: white;
		font-weight: bold;
	}

	:global(.ProseMirror h6) {
		font-size: 0.67em;
		margin: 2.33em 0;
		color: white;
		font-weight: bold;
	}

	/* Improve menu styling for better visibility of heading options */
	:global(.ProseMirror-menu-dropdown-item[title*="Heading"]) {
		font-weight: bold;
	}

	:global(.ProseMirror-menu-dropdown-item[title="Heading 1"]) {
		font-size: 1.2em;
	}

	:global(.ProseMirror-menu-dropdown-item[title="Heading 2"]) {
		font-size: 1.1em;
	}

	:global(.ProseMirror-menu-dropdown-item[title="Heading 3"]) {
		font-size: 1em;
	}

	:global(.ProseMirror-menu-dropdown-item[title="Heading 4"]) {
		font-size: 0.95em;
	}

	:global(.ProseMirror-menu-dropdown-item[title="Heading 5"]) {
		font-size: 0.9em;
	}

	:global(.ProseMirror-menu-dropdown-item[title="Heading 6"]) {
		font-size: 0.85em;
	}

	/* Cursor and selection styles */
	:global(.ProseMirror-yjs-cursor) {
		position: relative;
		margin-left: -1px;
		margin-right: -1px;
		border-left: 2px solid black; /* Slightly thicker */
		border-right: 2px solid black;
		pointer-events: none;
		z-index: 20;
	}

	/* Username tooltip */
	:global(.ProseMirror-yjs-cursor > div) {
		position: absolute;
		top: -1.8em;
		left: -1px;
		font-size: 12px;
		background-color: inherit; /* Will inherit from the cursor */
		font-family: "Inter", "Segoe UI", sans-serif;
		font-weight: 500;
		line-height: normal;
		user-select: none;
		color: white;
		padding: 3px 8px;
		border-radius: 4px;
		white-space: nowrap;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
	}

	:global(.ProseMirror-icon:hover) {
		border-color: #ddd;
		background: #e5e5e5;
	}

	:global(.ProseMirror) {
		position: relative;
		padding: 15px;
		min-height: 100px;
		outline: none;
		line-height: 1.5;
	}

	:global(.ProseMirror p) {
		margin: 0 0 1em 0;
	}

	:global(.ProseMirror h1) {
		font-size: 2em;
		margin: 0.67em 0;
	}

	/* Cursor and selection styles */
	:global(.ProseMirror-yjs-cursor) {
		position: relative;
		margin-left: -1px;
		margin-right: -1px;
		border-left: 1px solid black;
		border-right: 1px solid black;
		pointer-events: none;
	}

	:global(.ProseMirror-yjs-cursor > div) {
		position: absolute;
		top: -1.05em;
		left: -1px;
		font-size: 13px;
		background-color: rgb(250, 129, 0);
		font-family: serif;
		font-style: normal;
		font-weight: normal;
		line-height: normal;
		user-select: none;
		color: white;
		padding: 2px 6px;
		border-radius: 3px;
		white-space: nowrap;
	}
	:global(.ProseMirror-menu-dropdown-menu) {
		z-index: 999;
	}

	:global(.ProseMirror-example-setup-style::-webkit-scrollbar) {
		width: 4px;
		height: 4px;
	}

	:global(.ProseMirror-example-setup-style::-webkit-scrollbar-track) {
		background: transparent;
	}

	:global(.ProseMirror-example-setup-style::-webkit-scrollbar-thumb) {
		background-color: #2f303e;
		border-radius: 4px;
	}

	:global(.slash-command-menu) {
		max-height: 300px;
		overflow-y: auto;
		border-radius: 8px;
		animation: fadeIn 0.1s ease-in-out;
	}

	:global(.slash-command-menu::-webkit-scrollbar) {
		width: 4px;
		height: 4px;
	}

	:global(.slash-command-menu::-webkit-scrollbar-track) {
		background: transparent;
	}

	:global(.slash-command-menu::-webkit-scrollbar-thumb) {
		background-color: #2f303e;
		border-radius: 4px;
	}

	:global(.slash-command-item) {
		transition: background-color 0.15s ease;
		border-radius: 4px;
		margin: 4px;
	}

	:global(.slash-command-item:first-child) {
		margin-top: 4px;
	}

	:global(.slash-command-item:last-child) {
		margin-bottom: 4px;
	}

	:global(.slash-command-icon) {
		background: #2f303e;
		border-radius: 4px;
		width: 28px !important;
		height: 28px !important;
		color: #bfc0cc;
	}
	:global(.ProseMirror ul) {
		padding-left: 1.5em;
		margin: 0.5em 0;
		list-style-type: disc;
	}

	:global(.ProseMirror ul li) {
		margin: 0.2em 0;
		position: relative;
	}

	:global(.ProseMirror ul li p) {
		margin: 0;
	}

	/* Numbered List Styles */
	:global(.ProseMirror ol) {
		padding-left: 1.5em;
		margin: 0.5em 0;
		list-style-type: decimal;
	}

	:global(.ProseMirror ol li) {
		margin: 0.2em 0;
		position: relative;
	}

	:global(.ProseMirror ol li p) {
		margin: 0;
	}

	/* Nested List Styles */
	:global(.ProseMirror li > ul, .ProseMirror li > ol) {
		margin: 0.2em 0 0.2em 1em;
	}

	/* List item active state */
	:global(.ProseMirror li.ProseMirror-selectednode) {
		outline: 2px solid #2a2b2f;
	}

	/* Make sure list buttons in the menu are properly visible */
	:global(
		.ProseMirror-menu-dropdown-item[title="Wrap in bullet list"],
		.ProseMirror-menu-dropdown-item[title="Wrap in ordered list"]
	) {
		display: flex;
		align-items: center;
	}

	:global(
		.ProseMirror-menu-dropdown-item[title="Wrap in bullet list"]::before
	) {
		content: "•";
		margin-right: 5px;
		font-size: 1.2em;
	}

	:global(
		.ProseMirror-menu-dropdown-item[title="Wrap in ordered list"]::before
	) {
		content: "1.";
		margin-right: 5px;
		font-weight: bold;
	}

	:global(.ProseMirror blockquote) {
		border-left: 3px solid #4a4b53;
		margin-left: 0;
		margin-right: 0;
		padding-left: 1em;
		font-style: italic;
		color: #bfc0cc;
	}

	:global(.ProseMirror blockquote p) {
		margin: 0.5em 0;
	}

	/* Add a subtle background for better visibility in dark mode */
	:global(.ProseMirror blockquote) {
		background-color: rgba(255, 255, 255, 0.03);
		border-radius: 4px;
		padding: 8px 16px 8px 12px;
	}
</style>

<div class="editor-container">
	<div class="editor-main relative h-full">
		{#if isLoading}
			<div
				class="loading-overlay flex justify-center items-center h-full w-full">
				<div class="text-osvauld-fieldText">Loading note...</div>
			</div>
		{:else if error}
			<div class="error-message">{error}</div>
		{/if}

		<div bind:this="{element}" class="h-full scrollbar-thin"></div>
		<button
			on:click="{saveNoteManual}"
			class="absolute z-10 top-6 right-5 w-32 border border-osvauld-iconblack text-osvauld-fieldText text-[16px] font-medium px-2.5 py-1.5 rounded-lg cursor-pointer whitespace-nowrap">
			{#if saved}
				<span class="whitespace-nowrap flex items-center justify-center"
					><span class="text-[#9DD062] mr-2">Saved...</span>
					<span><SavedTick /></span></span>
			{:else}
				<span>Save Changes</span>
			{/if}
		</button>
	</div>
</div>
