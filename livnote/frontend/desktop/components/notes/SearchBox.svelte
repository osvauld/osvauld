<script lang="ts">
	import { onMount, onDestroy, tick } from "svelte";
	import type { EditorView } from "prosemirror-view";
	import { Selection } from "prosemirror-state";
	import {
		SearchManager,
		type SearchState,
		type SearchOptions,
	} from "./SearchManager";
	import { moveToStart, moveToEnd } from "./utils/prosemirror-helpers";
	let {
		searchManager,
		editorView,
		onHide,
	}: {
		searchManager: SearchManager;
		editorView: EditorView | null;
		onHide?: () => void;
	} = $props();

	// Component state
	let searchInput = $state<HTMLInputElement | null>(null);
	let replaceInput = $state<HTMLInputElement | null>(null);
	let searchValue = $state("");
	let replaceValue = $state("");
	let caseSensitive = $state(false);
	let wholeWord = $state(false);
	let useRegex = $state(false);
	let isReplaceMode = $state(false);
	let searchState = $state<SearchState>({
		query: null,
		isActive: false,
		currentMatch: 0,
		totalMatches: 0,
		isReplaceMode: false,
	});

	let unsubscribe: (() => void) | null = null;

	// Reactive updates
	$effect(() => {
		if (searchValue && editorView) {
			performSearch();
		} else if (!searchValue && editorView) {
			clearSearch();
		}
	});

	$effect(() => {
		if (caseSensitive || wholeWord || useRegex) {
			if (searchValue && editorView) {
				performSearch();
			}
		}
	});

	onMount(() => {
		// Subscribe to search state changes
		unsubscribe = searchManager.subscribe((state) => {
			searchState = state;
		});

		// Handle escape key to close search
		document.addEventListener("keydown", handleGlobalKeydown);

		// Focus search input when component mounts
		tick().then(() => {
			searchInput?.focus();
			searchInput?.select();
		});
	});

	onDestroy(() => {
		unsubscribe?.();
		document.removeEventListener("keydown", handleGlobalKeydown);
	});

	function handleGlobalKeydown(event: KeyboardEvent) {
		if (event.key === "Escape") {
			hide();
		}
	}

	function hide() {
		if (editorView) {
			clearSearch();
			editorView.focus();
		}
		onHide?.(); // Call the parent's hide function
	}

	function performSearch() {
		if (!editorView || !searchValue.trim()) return;

		const options: SearchOptions = {
			search: searchValue,
			replace: replaceValue,
			caseSensitive,
			wholeWord,
			regexp: useRegex,
		};

		searchManager.startSearch(editorView, options);
	}

	function clearSearch() {
		if (!editorView) return;
		searchManager.clearSearch(editorView);
	}

	function findNext() {
		if (!editorView) return;
		const found = searchManager.findNext(editorView);
		if (!found && searchState.totalMatches > 0) {
			// Wrap to beginning
			moveToStart(editorView);
			searchManager.findNext(editorView);
		}
	}

	function findPrevious() {
		if (!editorView) return;
		const found = searchManager.findPrevious(editorView);
		if (!found && searchState.totalMatches > 0) {
			// Wrap to end
			moveToEnd(editorView);
			searchManager.findPrevious(editorView);
		}
	}

	function replaceNext() {
		if (!editorView) return;
		searchManager.replaceNext(editorView);
	}

	function replaceCurrent() {
		if (!editorView) return;
		searchManager.replaceCurrent(editorView);
	}

	function replaceAll() {
		if (!editorView) return;
		searchManager.replaceAll(editorView);
	}

	function toggleReplaceMode() {
		isReplaceMode = !isReplaceMode;
		searchManager.toggleReplaceMode();
	}

	function handleSearchKeydown(event: KeyboardEvent) {
		switch (event.key) {
			case "Enter":
				if (event.shiftKey) {
					findPrevious();
				} else {
					findNext();
				}
				event.preventDefault();
				break;
			case "Escape":
				hide();
				break;
		}
	}

	function handleReplaceKeydown(event: KeyboardEvent) {
		switch (event.key) {
			case "Enter":
				replaceNext();
				event.preventDefault();
				break;
			case "Escape":
				hide();
				break;
		}
	}

	// Export methods for external use - remove show method since we're controlling visibility externally
	// export { hide };
</script>

<!-- VS Code-style search widget -->
<div
	class="bg-[#16171f] border border-[#2a2b2f] text-sm min-w-[320px] max-w-[400px] shadow-lg"
>
	<!-- Search row -->
	<div class="flex items-center bg-[#1e1f2a] border-b border-[#2a2b2f]">
		<!-- Replace toggle (first) -->
		<div class="flex items-center">
			<button
				class="w-4 h-8 flex items-center justify-center text-[#cccccc] hover:bg-[#2a2b2f]"
				class:text-[#007acc]={isReplaceMode}
				onclick={toggleReplaceMode}
				title="Toggle Replace"
				aria-label="Toggle Replace"
			>
				<svg
					width="16"
					height="16"
					viewBox="0 0 16 16"
					fill="currentColor"
					class:rotate-90={isReplaceMode}
				>
					<path d="M6 4l4 4-4 4V4z"></path>
				</svg>
			</button>
		</div>

		<!-- Search input with integrated controls -->
		<div class="flex items-center flex-1">
			<div class="relative flex-1">
				<input
					bind:this={searchInput}
					bind:value={searchValue}
					class="w-full h-8 px-2 pr-16 bg-transparent text-[#cccccc] text-sm border-none outline-none placeholder:text-[#6a737d]"
					type="text"
					autocorrect="off"
					autocomplete="off"
					placeholder="Find"
					onkeydown={handleSearchKeydown}
				/>
				<!-- Match count -->
				{#if searchState.totalMatches > 0}
					<div
						class="absolute right-0 top-1/2 -translate-y-1/2 text-xs text-[#858585] pointer-events-none"
					>
						{searchState.currentMatch} of {searchState.totalMatches}
					</div>
				{/if}
			</div>

			<!-- Navigation buttons -->
			<div class="flex">
				<button
					class="w-8 h-8 flex items-center justify-center text-[#cccccc] hover:bg-[#2a2d2e] disabled:text-[#6a737d] disabled:cursor-not-allowed"
					onclick={findPrevious}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Previous match (Shift+Enter)"
					aria-label="Previous match"
				>
					<svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
						<path d="M8 12L3 7l5-5 1.41 1.41L5.83 7l3.58 3.59L8 12z"></path>
					</svg>
				</button>
				<button
					class="w-8 h-8 flex items-center justify-center text-[#cccccc] hover:bg-[#2a2d2e] disabled:text-[#6a737d] disabled:cursor-not-allowed"
					onclick={findNext}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Next match (Enter)"
					aria-label="Next match"
				>
					<svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
						<path d="M8 4l5 5-5 5-1.41-1.41L10.17 9 6.59 5.41 8 4z"></path>
					</svg>
				</button>
			</div>
		</div>

		<!-- Options and controls -->
		<div class="flex items-center px-1">
			<!-- Toggle options -->
			<button
				class="w-8 h-8 flex items-center justify-center text-xs font-mono hover:bg-[#2a2d2e]"
				class:text-[#007acc]={caseSensitive}
				class:text-[#cccccc]={!caseSensitive}
				onclick={() => (caseSensitive = !caseSensitive)}
				title="Match Case"
			>
				Aa
			</button>
			<button
				class="w-8 h-8 flex items-center justify-center text-xs font-mono hover:bg-[#2a2d2e]"
				class:text-[#007acc]={wholeWord}
				class:text-[#cccccc]={!wholeWord}
				onclick={() => (wholeWord = !wholeWord)}
				title="Match Whole Word"
			>
				Ab
			</button>
			<button
				class="w-8 h-8 flex items-center justify-center text-xs font-mono hover:bg-[#2a2d2e]"
				class:text-[#007acc]={useRegex}
				class:text-[#cccccc]={!useRegex}
				onclick={() => (useRegex = !useRegex)}
				title="Use Regular Expression"
			>
				.*
			</button>

			<!-- Close button -->
			<button
				class="w-8 h-8 flex items-center justify-center text-[#cccccc] hover:bg-[#2a2d2e]"
				onclick={hide}
				title="Close (Escape)"
				aria-label="Close"
			>
				<svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
					<path
						d="M8 6.293l5.293-5.293.707.707L8.707 7 14 12.293l-.707.707L8 7.707l-5.293 5.293-.707-.707L7.293 7 2 1.707l.707-.707L8 6.293z"
					></path>
				</svg>
			</button>
		</div>
	</div>

	<!-- Replace row (when active) -->
	{#if isReplaceMode}
		<div class="flex items-center bg-[#1e1f2a]">
			<!-- Replace input -->
			<div class="flex items-center flex-1">
				<div class="relative flex-1">
					<input
						bind:this={replaceInput}
						bind:value={replaceValue}
						class="w-full h-8 px-2 pr-16 bg-transparent text-[#cccccc] text-sm border-none outline-none placeholder:text-[#6a737d]"
						type="text"
						placeholder="Replace"
						onkeydown={handleReplaceKeydown}
					/>
				</div>
			</div>

			<!-- Replace controls -->
			<div class="flex items-center px-1">
				<button
					class="w-8 h-8 flex items-center justify-center text-[#cccccc] hover:bg-[#2a2d2e] disabled:text-[#6a737d] disabled:cursor-not-allowed"
					onclick={replaceCurrent}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Replace"
					aria-label="Replace"
				>
					<svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
						<path
							d="M13.78 4.22a.75.75 0 010 1.06l-7.25 7.25a.75.75 0 01-1.06 0L2.22 9.28a.75.75 0 011.06-1.06L6 10.94l6.72-6.72a.75.75 0 011.06 0z"
						></path>
					</svg>
				</button>
				<button
					class="w-8 h-8 flex items-center justify-center text-[#cccccc] hover:bg-[#2a2d2e] disabled:text-[#6a737d] disabled:cursor-not-allowed"
					onclick={replaceAll}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Replace All"
					aria-label="Replace All"
				>
					<svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
						<path
							d="M1.5 3.5A1.5 1.5 0 013 2h10a1.5 1.5 0 011.5 1.5v9A1.5 1.5 0 0113 14H3a1.5 1.5 0 01-1.5-1.5v-9zM3 3.5v9a.5.5 0 00.5.5h9a.5.5 0 00.5-.5v-9a.5.5 0 00-.5-.5h-9a.5.5 0 00-.5.5z"
						></path>
						<path
							d="M5.5 7a.5.5 0 01.5-.5h4a.5.5 0 010 1H6a.5.5 0 01-.5-.5zM5.5 9a.5.5 0 01.5-.5h4a.5.5 0 010 1H6a.5.5 0 01-.5-.5z"
						></path>
					</svg>
				</button>
			</div>
		</div>
	{/if}
</div>
