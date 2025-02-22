<script lang="ts">
	import EditorContainer from "./EditorContainer.svelte";
	import { initEditor, type AppState } from "./utils/editor";
	import { writable } from "svelte/store";
	import { setContext } from "svelte";

	const appState = writable<AppState>(initEditor());
	setContext("appState", appState);
</script>

<div class="flex h-full text-osvauld-textActive font-sans bg-blue-400">
	<EditorContainer />
</div>
