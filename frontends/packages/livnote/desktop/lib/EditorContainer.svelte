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

	const appState = getContext<Writable<AppState>>("appState");
	let editorContainer: HTMLDivElement;
	let unsubscribe: () => void;
	let unlistenHandlers: Array<() => void> = [];
	export let syncRole: string;
	let currentDoc: Doc | null = null;

	function refreshEditor(state: AppState) {
		if (editorContainer && state) {
			editorContainer.innerHTML = "";
			editorContainer.appendChild(state.editor);
			document.documentElement.classList.add("dark");
		}
	}

	function setupDocumentHandlers(doc: Doc) {
		console.log("Setting up document handlers");

		doc.spaceDoc.on("update", (update: Uint8Array, origin: unknown) => {
			if (origin !== "remote") {
				console.log("Local update detected, sending to peer");
				handleDocUpdate(update);
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
			const updateArray = Array.from(update);
			console.log("Sending update, size:", updateArray.length);
			await emit("sync-update", updateArray);
		} catch (error) {
			console.error("Error sending update:", error);
		}
	}

	async function sendInitialSnapshot() {
		console.log("Preparing to send initial Y.js state");
		if (!currentDoc) {
			console.error("No document available for snapshot");
			return;
		}

		try {
			const encodedState = Y.encodeStateAsUpdate(currentDoc.spaceDoc);
			const stateArray = Array.from(encodedState);
			console.log("Sending snapshot, size:", stateArray.length);
			await emit("sync-snapshot", stateArray);
			console.log("Snapshot sent successfully");
		} catch (error) {
			console.error("Error sending snapshot:", error);
		}
	}

	async function setupSyncListeners() {
		console.log("Setting up sync listeners for role:", syncRole);

		// Handle incoming updates
		const unlistenUpdate = await listen("sync-update-be", async (event) => {
			try {
				console.log("Received sync update");
				const binaryData = new Uint8Array(event.payload as number[]);

				const doc = currentDoc;
				if (!doc) return;

				Y.applyUpdate(doc.spaceDoc, binaryData, "remote");
				refreshEditor(get(appState));
				appState.update((state) => state);
			} catch (error) {
				console.error("Error handling sync update:", error);
			}
		});

		// Handle incoming snapshots
		const unlistenSnapshot = await listen("sync-snapshot-be", async (event) => {
			try {
				console.log("Received sync snapshot");
				const binaryData = new Uint8Array(event.payload as number[]);

				const doc = currentDoc;
				if (!doc) return;

				Y.applyUpdate(doc.spaceDoc, binaryData, "remote");
				refreshEditor(get(appState));
				appState.update((state) => state);
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

		// Set up UI updates
		unsubscribe = appState.subscribe((state) => {
			if (state) refreshEditor(state);
		});

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
		if (editorContainer) {
			editorContainer.innerHTML = "";
		}
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
