<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { getContext } from "svelte";
	import type { Writable } from "svelte/store";
	import type { AppState } from "./utils/editor.ts";
	import { listen, emit } from "@tauri-apps/api/event";
	import { initEditor } from "./utils/editor";
	import * as Y from "yjs";
	import { get } from "svelte/store";
	import type { Doc } from "@blocksuite/store";
	import { sendMessage } from "@osvauld/password-manager-common";

	const appState = getContext<Writable<AppState>>("appState");
	let editorContainer: HTMLDivElement;
	let unsubscribe: () => void;
	let unlistenHandlers: Array<() => void> = [];
	export let syncRole: string;
	let currentDoc: Doc | null = null;
	let editor: any = null;

	function setupDocumentHandlers(doc: Doc) {
		console.log("Setting up document handlers");

		doc.spaceDoc.on("update", (update: Uint8Array, origin: unknown) => {
			if (origin !== "remote") {
				console.log("Local update detected, sending to peer");
				handleDocUpdate(update);
			} else {
				console.log(origin, "....");
			}
		});

		doc.spaceDoc.on("afterTransaction", (transaction: Y.Transaction) => {
			console.log("Document transaction:", {
				origin: transaction.origin,
				changed: transaction.changed.size > 0,
			});
		});
	}
	async function handleDocUpdate(update: Uint8Array) {
		try {
			// Check buffer size before converting
			if (update.length > 0) {
				console.log("Sending update, size:", update.length);
				// Send the update directly as Uint8Array
				await emit("sync-update", Array.from(update));
			} else {
				console.warn("Empty update received, skipping");
			}
		} catch (error) {
			console.error("Error sending update:", error, {
				updateSize: update?.length,
				updateType: typeof update,
			});
		}
	}

	async function sendInitialSnapshot() {
		console.log("Preparing to send initial Y.js state");
		if (!currentDoc) {
			console.error("No document available for snapshot");
			return;
		}

		try {
			const encodedState = Y.encodeStateAsUpdateV2(currentDoc.spaceDoc);
			const stateArray = Array.from(encodedState);
			console.log("Sending snapshot, size:", stateArray.length);
			await sendMessage("sendSnapshot", stateArray);
			console.log("Snapshot sent successfully");
		} catch (error) {
			console.error("Error sending snapshot:", error);
		}
	}

	function refreshDocumentUI() {
		if (currentDoc && editor) {
			console.log("Refreshing document UI");
			// Trigger a reload of the document content
			currentDoc.load(() => {
				// Update the editor's doc reference
				editor.doc = currentDoc;
				console.log("Document reloaded in editor");
			});
		}
	}

	async function setupSyncListeners() {
		console.log("Setting up sync listeners for role:", syncRole);

		const unlistenUpdate = await listen("sync-update-be", async (event) => {
			try {
				console.log("Received sync update");
				console.log("Raw payload:", event.payload);

				const binaryData = new Uint8Array(event.payload as number[]);
				console.log("Converted to Uint8Array, length:", binaryData.length);

				const doc = currentDoc;
				if (!doc) return;
				doc.spaceDoc.transact(() => {
					Y.applyUpdateV2(doc.spaceDoc, binaryData);
				}, "remote");
				console.log("Doc state after update:", {
					hasContent: !doc.isEmpty,
					meta: doc.meta,
				});

				refreshDocumentUI();
			} catch (error) {
				console.error("Error handling sync update:", error);
			}
		});

		const unlistenSnapshot = await listen("sync-snapshot-be", async (event) => {
			try {
				console.log("Received sync snapshot");
				const binaryData = new Uint8Array(event.payload as number[]);

				const doc = currentDoc;
				if (!doc) return;

				Y.applyUpdateV2(doc.spaceDoc, binaryData, "remote");
				refreshDocumentUI();
			} catch (error) {
				console.error("Error handling snapshot:", error);
			}
		});

		unlistenHandlers.push(unlistenUpdate, unlistenSnapshot);
	}

	onMount(async () => {
		console.log(`Mounting EditorContainer with role: ${syncRole}`);

		// Initialize the editor state
		const state = initEditor();
		console.log("Editor state initialized");
		appState.set(state);

		// Set up document
		currentDoc = state.collection.getDoc("page1") as Doc;
		if (currentDoc) {
			console.log("Setting up document handlers for page1");
			setupDocumentHandlers(currentDoc);
		} else {
			console.error("Failed to get page1 document");
		}

		// Store editor reference and mount it once
		editor = state.editor;
		if (editorContainer && editor) {
			document.documentElement.classList.add("dark");
			editorContainer.appendChild(editor);
		}

		// Set up event listeners
		await setupSyncListeners();

		// If we're the acceptor, send initial snapshot
		if (syncRole === "acceptor") {
			console.log("Acceptor: Sending initial snapshot");
			await sendInitialSnapshot();
		}

		console.log("Mount complete for role:", syncRole);
	});

	onDestroy(() => {
		console.log("Destroying editor container");
		unsubscribe?.();
		unlistenHandlers.forEach((unlisten) => unlisten());
	});
</script>

<style>
	:global(.editor-container) {
		background-color: #0d0e13 !important;
	}
	:global(.affine-default-page-block-container),
	:global(.affine-database-kanban-view),
	:global(.affine-database-table),
	:global(.affine-page-root-block-container) {
		background-color: #0d0e13 !important;
		color: #a3a4b5 !important;
	}
	:global(.kanban-column),
	:global(.kanban-card),
	:global(.affine-database-table-cell) {
		background-color: #111218 !important;
		border-color: #292a36 !important;
	}
</style>

<div
	bind:this={editorContainer}
	class="editor-container h-full w-full bg-osvauld-frameblack">
</div>
