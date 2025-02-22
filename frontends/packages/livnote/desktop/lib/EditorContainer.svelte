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
			// Get the current state (which was created in DocumentEditor)
			let currentAppState;
			unsubscribe = appState.subscribe((state) => {
				currentAppState = state;
				if (editorContainer && state) {
					editorContainer.innerHTML = "";
					editorContainer.appendChild(state.editor);
					document.documentElement.classList.add("dark");
				}
			});

			const deviceId = crypto.randomUUID();
			tauriSync = new TauriSync(currentAppState.collection, deviceId);
			await tauriSync.sendInitialSnapshot();
		} else {
			console.log("test");
			// For initiator, wait for snapshot and then initialize
			await listen("sync-snapshot", async (event) => {
				const binaryData = new Uint8Array(event.payload as number[]);

				// Create initial state
				const state = initEditor();

				// Apply the snapshot to the doc
				const doc = state.collection.getDoc("page1");
				if (doc) {
					Y.applyUpdate(doc.spaceDoc, binaryData, "remote");
				}

				appState.set(state);

				// Set up subscription
				unsubscribe = appState.subscribe((state) => {
					if (editorContainer && state) {
						editorContainer.innerHTML = "";
						editorContainer.appendChild(state.editor);
						document.documentElement.classList.add("dark");
					}
				});

				const deviceId = crypto.randomUUID();
				tauriSync = new TauriSync(state.collection, deviceId);
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
