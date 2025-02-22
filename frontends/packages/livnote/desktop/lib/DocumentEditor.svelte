<script lang="ts">
	import EditorContainer from "./EditorContainer.svelte";
	import { initEditor, type AppState } from "./utils/editor";
	import { writable } from "svelte/store";
	import { onMount, setContext } from "svelte";
	import { listen } from "@tauri-apps/api/event";

	export let syncRole: string;
	let isReady = false;

	// Initialize the store with null, it will be set when appropriate
	const appState = writable<AppState>(null);
	setContext("appState", appState);

	onMount(async () => {
		if (syncRole === "acceptor") {
			// For acceptor, create the initial state right away
			const state = initEditor();
			appState.set(state);
			isReady = true;
		} else {
			// For initiator, wait for sync-snapshot event before creating state
			await listen("sync-snapshot", async (event) => {
				isReady = true;
				// The actual state will be handled in EditorContainer
			});
		}
	});
</script>

<div
	class="flex h-screen w-screen bg-osvauld-frameblack text-osvauld-textActive font-sans">
	{#if isReady}
		<EditorContainer {syncRole} />
	{:else}
		<div class="flex justify-center items-center h-full">
			<p>Waiting for connection...</p>
		</div>
	{/if}
</div>

