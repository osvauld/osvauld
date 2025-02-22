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

	function refreshEditor(state: AppState, doc: Doc) {
		if (editorContainer && state) {
			// Remove old editor
			editorContainer.innerHTML = "";

			// Update editor with the confirmed non-null doc
			state.editor.doc = doc;

			// Append the updated editor
			editorContainer.appendChild(state.editor);
			document.documentElement.classList.add("dark");
		}
	}

	async function setupSyncListeners() {
		const unlisten1 = await listen("sync-update-be", async (event) => {
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

				// Update the editor with the confirmed non-null doc
				refreshEditor(currentState, doc);

				// Force a store update to trigger reactivity
				appState.update((state) => state);
			} catch (error) {
				console.error("Error handling sync update:", error);
			}
		});

		unlistenHandlers.push(unlisten1);
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
				const doc = state.collection.getDoc("page1") as Doc;
				if (doc) {
					refreshEditor(state, doc);
				}
			}
		});

		// Set up common sync listeners for both roles
		await setupSyncListeners();

		if (syncRole === "acceptor") {
			const deviceId = crypto.randomUUID();
			const currentState = get(appState);
			if (currentState) {
				tauriSync = new TauriSync(currentState.collection, deviceId);
				await tauriSync.sendInitialSnapshot();
				console.log("Acceptor: Sent initial snapshot");
			}
		} else if (syncRole === "initiator") {
			console.log("Setting up sync-snapshot-be listener");

			const unlistenSnapshot = await listen(
				"sync-snapshot-be",
				async (event) => {
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

						refreshEditor(currentState, doc);

						// Force a store update
						appState.update((state) => state);

						// Initialize TauriSync after receiving snapshot
						if (!tauriSync) {
							const deviceId = crypto.randomUUID();
							tauriSync = new TauriSync(currentState.collection, deviceId);
						}
					} catch (error) {
						console.error("Error in sync-snapshot-be handler:", error);
					}
				},
			);

			unlistenHandlers.push(unlistenSnapshot);
		}
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

