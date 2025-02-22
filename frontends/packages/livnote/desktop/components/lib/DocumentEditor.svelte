<script lang="ts">
	import EditorContainer from "./EditorContainer.svelte";
	import { initEditor, type AppState } from "./utils/editor";
	import { writable } from "svelte/store";
	import { setContext } from "svelte";

	const appState = writable<AppState>(initEditor());
	setContext("appState", appState);
</script>

<div
	class="flex h-screen w-screen bg-osvauld-frameblack text-osvauld-textActive font-sans">
	<EditorContainer />
</div>
