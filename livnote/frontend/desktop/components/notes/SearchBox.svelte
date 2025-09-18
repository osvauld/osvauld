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

<style>
	.search-container {
		position: fixed;
		top: 60px;
		right: 20px;
		z-index: 1000;
		background: #2a2b35;
		border: 1px solid #3a3b44;
		border-radius: 8px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
		min-width: 320px;
		max-width: 400px;
	}

	.search-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px 8px 16px;
		border-bottom: 1px solid #3a3b44;
	}

	.search-title {
		font-size: 14px;
		font-weight: 500;
		color: #f0f0f0;
	}

	.close-button {
		background: none;
		border: none;
		color: #85889c;
		cursor: pointer;
		padding: 4px;
		border-radius: 4px;
		transition: all 0.2s;
	}

	.close-button:hover {
		background: #3a3b44;
		color: #f0f0f0;
	}

	.search-content {
		padding: 16px;
	}

	.search-row {
		display: flex;
		gap: 8px;
		margin-bottom: 12px;
		align-items: center;
	}

	.search-input-container {
		position: relative;
		flex: 1;
	}

	.search-input {
		width: 100%;
		padding: 8px 12px;
		background: #16171f;
		border: 1px solid #3a3b44;
		border-radius: 4px;
		color: #f0f0f0;
		font-size: 14px;
		transition: border-color 0.2s;
	}

	.search-input:focus {
		outline: none;
		border-color: #4094ef;
	}

	.search-input::placeholder {
		color: #85889c;
	}

	.match-count {
		position: absolute;
		right: 8px;
		top: 50%;
		transform: translateY(-50%);
		font-size: 12px;
		color: #85889c;
		pointer-events: none;
	}

	.button-group {
		display: flex;
		gap: 4px;
	}

	.icon-button {
		background: none;
		border: 1px solid #3a3b44;
		color: #85889c;
		cursor: pointer;
		padding: 8px;
		border-radius: 4px;
		transition: all 0.2s;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.icon-button:hover {
		background: #3a3b44;
		border-color: #4a4b54;
		color: #f0f0f0;
	}

	.icon-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.icon-button.active {
		background: #4094ef;
		border-color: #4094ef;
		color: white;
	}

	.replace-row {
		margin-top: 8px;
	}

	.options-row {
		display: flex;
		gap: 12px;
		margin-top: 12px;
		padding-top: 12px;
		border-top: 1px solid #3a3b44;
	}

	.option-button {
		background: none;
		border: 1px solid #3a3b44;
		color: #85889c;
		cursor: pointer;
		padding: 6px 12px;
		border-radius: 4px;
		font-size: 12px;
		transition: all 0.2s;
	}

	.option-button:hover {
		background: #3a3b44;
		color: #f0f0f0;
	}

	.option-button.active {
		background: #4094ef;
		border-color: #4094ef;
		color: white;
	}

	.replace-buttons {
		display: flex;
		gap: 8px;
		margin-top: 8px;
	}

	.replace-button {
		background: #4094ef;
		border: 1px solid #4094ef;
		color: white;
		cursor: pointer;
		padding: 6px 12px;
		border-radius: 4px;
		font-size: 12px;
		transition: all 0.2s;
	}

	.replace-button:hover {
		background: #3280d1;
	}

	.replace-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.replace-button.secondary {
		background: none;
		border-color: #4094ef;
		color: #4094ef;
	}

	.replace-button.secondary:hover {
		background: rgba(64, 148, 239, 0.1);
	}
</style>

<div class="search-container">
	<div class="search-header">
		<div class="search-title">Find {isReplaceMode ? "and Replace" : ""}</div>
		<button class="close-button" onclick={hide} title="Close (Esc)"> ✕ </button>
	</div>

	<div class="search-content">
		<!-- Search Input -->
		<div class="search-row">
			<div class="search-input-container">
				<input
					bind:this={searchInput}
					bind:value={searchValue}
					class="search-input"
					type="text"
					placeholder="Search..."
					onkeydown={handleSearchKeydown}
				/>
				{#if searchState.totalMatches > 0}
					<div class="match-count">
						{searchState.currentMatch}/{searchState.totalMatches}
					</div>
				{/if}
			</div>

			<div class="button-group">
				<button
					class="icon-button"
					onclick={findPrevious}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Previous match (Shift+Enter)"
				>
					↑
				</button>
				<button
					class="icon-button"
					onclick={findNext}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Next match (Enter)"
				>
					↓
				</button>
				<button
					class="icon-button"
					class:active={isReplaceMode}
					onclick={toggleReplaceMode}
					title="Toggle replace mode"
				>
					↔
				</button>
			</div>
		</div>

		<!-- Replace Input -->
		{#if isReplaceMode}
			<div class="search-row replace-row">
				<div class="search-input-container">
					<input
						bind:this={replaceInput}
						bind:value={replaceValue}
						class="search-input"
						type="text"
						placeholder="Replace with..."
						onkeydown={handleReplaceKeydown}
					/>
				</div>
			</div>

			<div class="replace-buttons">
				<button
					class="replace-button secondary"
					onclick={replaceCurrent}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Replace current match"
				>
					Replace
				</button>
				<button
					class="replace-button secondary"
					onclick={replaceNext}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Replace and find next"
				>
					Replace + Next
				</button>
				<button
					class="replace-button"
					onclick={replaceAll}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Replace all matches"
				>
					Replace All
				</button>
			</div>
		{/if}

		<!-- Search Options -->
		<div class="options-row">
			<button
				class="option-button"
				class:active={caseSensitive}
				onclick={() => (caseSensitive = !caseSensitive)}
				title="Case sensitive"
			>
				Aa
			</button>
			<button
				class="option-button"
				class:active={wholeWord}
				onclick={() => (wholeWord = !wholeWord)}
				title="Whole word"
			>
				Ab|
			</button>
			<button
				class="option-button"
				class:active={useRegex}
				onclick={() => (useRegex = !useRegex)}
				title="Regular expression"
			>
				.*
			</button>
		</div>
	</div>
</div>
