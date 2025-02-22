<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { getContext } from "svelte";
	import type { Writable } from "svelte/store";
	import type { AppState } from "./utils/editor.ts";
	import { TauriSync } from "./utils/tauriSync.js";
	import { listen } from "@tauri-apps/api/event";
	import { initEditor } from "./utils/editor";
	import * as Y from "yjs";

	const appState = getContext<Writable<AppState>>("appState");
	let editorContainer: HTMLDivElement;
	let tauriSync: TauriSync;
	let unsubscribe: () => void;
	export let syncRole: string;

	onMount(async () => {
		if (syncRole === "acceptor") {
			// For acceptor, we already have the state initialized
			let currentState: AppState;
			unsubscribe = appState.subscribe((state) => {
				currentState = state;
				if (editorContainer && state) {
					editorContainer.innerHTML = "";
					editorContainer.appendChild(state.editor);
					document.documentElement.classList.add("dark");
				}
			});

			// Initialize TauriSync and send initial snapshot
			const deviceId = crypto.randomUUID();
			tauriSync = new TauriSync(currentState.collection, deviceId);
			await tauriSync.sendInitialSnapshot();
			console.log("Acceptor: Sent initial snapshot");
		} else if (syncRole === "initiator") {
			console.log("Setting up sync-snapshot-be listener");
			await listen("sync-snapshot-be", async (event) => {
				try {
					console.log("Received sync-snapshot-be event");
					const binaryData = new Uint8Array(event.payload as number[]);
					console.log("Created Uint8Array with length:", binaryData.length);

					const state = initEditor();
					console.log("Created initial editor state");

					const doc = state.collection.getDoc("page1");
					if (doc) {
						console.log("Found page1 doc, applying update");
						Y.applyUpdate(doc.spaceDoc, binaryData, "remote");
						console.log("Successfully applied update to doc");
					}

					appState.set(state);
					console.log("Set new app state");

					unsubscribe = appState.subscribe((state) => {
						if (editorContainer && state) {
							editorContainer.innerHTML = "";
							editorContainer.appendChild(state.editor);
							document.documentElement.classList.add("dark");
						}
					});

					const deviceId = crypto.randomUUID();
					tauriSync = new TauriSync(state.collection, deviceId);
				} catch (error) {
					console.error("Error in sync-snapshot-be handler:", error);
				}
			});
		}
	});

	onDestroy(() => {
		if (unsubscribe) {
			unsubscribe();
		}
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
