<script lang="ts">
	import EditorContainer from "./EditorContainer.svelte";
	import { initEditor, type AppState } from "./utils/editor";
	import { writable } from "svelte/store";
	import { onMount, setContext } from "svelte";

	export let syncRole: string;

	const appState = writable<AppState>(initEditor());
	setContext("appState", appState);

	onMount(() => {
		console.log("document editor");
		if (syncRole === "acceptor") {
			const state = initEditor();
			appState.set(state);
		}
	});
</script>

<div class="flex h-full text-osvauld-textActive font-sans bg-blue-400">
	<EditorContainer {syncRole} />
</div>
