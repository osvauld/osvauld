<script lang="ts">
	import { onMount } from "svelte";
	import { getContext } from "svelte";
	import type { Writable } from "svelte/store";
	import type { AppState } from "./utils/editor.js";

	const appState = getContext<Writable<AppState>>("appState");
	let editorContainer: HTMLDivElement;

	onMount(() => {
		appState.subscribe(({ editor }) => {
			if (editorContainer) {
				editorContainer.appendChild(editor);
				document.documentElement.classList.add("dark");
			}
		});
	});
</script>

<style>
	:global(.editor-container) {
		background-color: #fff !important;
	}

	:global(.affine-default-page-block-container),
	:global(.affine-database-kanban-view),
	:global(.affine-database-table),
	:global(.affine-page-root-block-container) {
		background-color: #fff !important;
		color: #a3a4b5 !important;
	}

	:global(.kanban-column),
	:global(.kanban-card),
	:global(.affine-database-table-cell) {
		background-color: #fff !important;
		border-color: #292a36 !important;
	}

	:global(doc-meta-tags) {
		display: none;
	}

	:global(affine-page-root-block-container) {
		background-color: #fff !important;
	}
</style>

<div
	bind:this="{editorContainer}"
	class="editor-container h-full w-full bg-osvauld-frameblack">
</div>
