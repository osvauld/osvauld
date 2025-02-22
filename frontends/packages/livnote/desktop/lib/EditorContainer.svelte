<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { getContext } from "svelte";
	import type { Writable } from "svelte/store";
	import type { AppState } from "./utils/editor.ts";
	import { TauriSync } from "./utils/tauriSync.js";
	import { listen } from "@tauri-apps/api/event";
	import { initEditor } from "./utils/editor";
	import * as Y from "yjs";
	import { get } from "svelte/store";
	import type { Doc } from "@blocksuite/store";

	const appState = getContext<Writable<AppState>>("appState");
	let editorContainer: HTMLDivElement;
	let tauriSync: TauriSync;
	let unsubscribe: () => void;
	let unlistenHandlers: Array<() => void> = [];
	export let syncRole: string;

	function refreshEditor(state: AppState) {
		if (editorContainer && state) {
			editorContainer.innerHTML = "";
			editorContainer.appendChild(state.editor);
			document.documentElement.classList.add("dark");
		}
	}

	async function initializeTauriSync(state: AppState) {
		const deviceId = crypto.randomUUID();
		tauriSync = new TauriSync(state.collection, deviceId);

		// If we're the acceptor, send the initial snapshot
		if (syncRole === "acceptor") {
			await tauriSync.sendInitialSnapshot();
			console.log("Acceptor: Sent initial snapshot");
		}
	}

	async function setupSyncListeners() {
		// Listen for sync updates
		const unlistenUpdate = await listen("sync-update-be", async (event) => {
			try {
				console.log("Received sync-update-be event");
				const binaryData = new Uint8Array(event.payload as number[]);
				console.log("Update size:", binaryData.length);

				const currentState = get(appState);
				if (!currentState) return;

				const doc = currentState.collection.getDoc("page1") as Doc;
				if (!doc) {
					console.error("No page1 doc found");
					return;
				}

				console.log("Applying update to doc");
				Y.applyUpdate(doc.spaceDoc, binaryData, "remote");

				refreshEditor(currentState);
				appState.update((state) => state);
			} catch (error) {
				console.error("Error handling sync update:", error);
			}
		});

		// Listen for snapshot updates
		const unlistenSnapshot = await listen("sync-snapshot-be", async (event) => {
			try {
				console.log("Received sync-snapshot-be event");
				const binaryData = new Uint8Array(event.payload as number[]);
				console.log("Snapshot size:", binaryData.length);

				const currentState = get(appState);
				if (!currentState) return;

				const doc = currentState.collection.getDoc("page1") as Doc;
				if (!doc) {
					console.error("No page1 doc found");
					return;
				}

				Y.applyUpdate(doc.spaceDoc, binaryData, "remote");
				console.log("Applied snapshot to doc");

				refreshEditor(currentState);
				appState.update((state) => state);
			} catch (error) {
				console.error("Error in sync-snapshot-be handler:", error);
			}
		});

		unlistenHandlers.push(unlistenUpdate, unlistenSnapshot);
	}

	onMount(async () => {
		console.log(`Mounting EditorContainer with role: ${syncRole}`);

		// Initialize state for both roles
		const state = initEditor();
		const initialDoc = state.collection.getDoc("page1") as Doc;
		if (!initialDoc) {
			console.error("Failed to initialize document");
			return;
		}

		state.editor.doc = initialDoc;
		appState.set(state);

		// Set up subscription for UI updates
		unsubscribe = appState.subscribe((state) => {
			if (state) {
				refreshEditor(state);
			}
		});

		// Set up sync listeners for both roles
		await setupSyncListeners();

		// Initialize TauriSync for both roles
		await initializeTauriSync(state);
	});

	onDestroy(() => {
		if (unsubscribe) {
			unsubscribe();
		}
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
