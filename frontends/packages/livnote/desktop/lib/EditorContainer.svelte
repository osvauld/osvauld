<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { getContext } from "svelte";
	import type { Writable } from "svelte/store";
	import type { AppState } from "./utils/editor.ts";
	import { TauriSync } from "./utils/tauriSync.js";
	import { initEditor } from "./utils/editor";
	import { get } from "svelte/store";
	import type { Doc } from "@blocksuite/store";

	const appState = getContext<Writable<AppState>>("appState");
	let editorContainer: HTMLDivElement;
	let tauriSync: TauriSync;
	let unsubscribe: () => void;
	export let syncRole: string;

	function refreshEditor(state: AppState) {
		if (editorContainer && state) {
			editorContainer.innerHTML = "";
			editorContainer.appendChild(state.editor);
			document.documentElement.classList.add("dark");
		}
	}

	onMount(async () => {
		console.log(`Mounting EditorContainer with role: ${syncRole}`);

		// Initialize state
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

		// Initialize TauriSync based on role
		if (syncRole === "acceptor") {
			const deviceId = crypto.randomUUID();
			const currentState = get(appState);
			if (currentState) {
				tauriSync = new TauriSync(currentState.collection, deviceId);
				await tauriSync.sendInitialSnapshot();
				console.log("Acceptor: Sent initial snapshot");
			}
		} else if (syncRole === "initiator") {
			const deviceId = crypto.randomUUID();
			const currentState = get(appState);
			if (currentState) {
				tauriSync = new TauriSync(currentState.collection, deviceId);
			}
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

