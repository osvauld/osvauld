<script lang="ts">
	import EditorContainer from "./EditorContainer.svelte";
	import { initEditor, type AppState } from "./utils/editor";
	import { writable } from "svelte/store";
	import { onMount, setContext } from "svelte";

	export let syncRole: string;

	// Initialize the store with null, it will be set when appropriate
	const appState = writable<AppState>(null);
	setContext("appState", appState);

	onMount(() => {
		console.log("document editor");
		if (syncRole === "acceptor") {
			const state = initEditor();
			appState.set(state);
		}
	});
</script>

<div
	class="flex h-screen w-screen bg-osvauld-frameblack text-osvauld-textActive font-sans">
	<EditorContainer {syncRole} />
</div>

