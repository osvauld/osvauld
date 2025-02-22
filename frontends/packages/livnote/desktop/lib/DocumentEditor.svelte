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
	export let syncRole;

	onMount(async () => {
		if (syncRole === "acceptor") {
			// Acceptor creates its own document and sends it
			let currentAppState;
			appState.subscribe((state) => {
				currentAppState = state;
				if (editorContainer && state) {
					editorContainer.appendChild(state.editor);
					document.documentElement.classList.add("dark");
				}
			});

			const state = initEditor();
			appState.set(state);

			const deviceId = crypto.randomUUID();
			tauriSync = new TauriSync(currentAppState.collection, deviceId);
			await tauriSync.sendInitialSnapshot();
		} else {
			// Initiator waits for snapshot before creating document
			await listen("sync-snapshot", async (event) => {
				console.log("Received sync snapshot event");
				const binaryData = new Uint8Array(event.payload as number[]);

				// Create a new state with empty doc
				const state = initEditor();
				const deviceId = crypto.randomUUID();

				// Initialize TauriSync
				tauriSync = new TauriSync(state.collection, deviceId);

				// Apply the snapshot to the doc
				if (state.collection.getDoc("page1")) {
					Y.applyUpdate(
						state.collection.getDoc("page1").spaceDoc,
						binaryData,
						"remote",
					);
				}

				// Update the UI state
				appState.set(state);

				if (editorContainer) {
					editorContainer.appendChild(state.editor);
					document.documentElement.classList.add("dark");
				}
			});
		}
	});

	// Clean up subscription when component is destroyed
	onDestroy(() => {
		if (editorContainer && editorContainer.firstChild) {
			editorContainer.removeChild(editorContainer.firstChild);
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
