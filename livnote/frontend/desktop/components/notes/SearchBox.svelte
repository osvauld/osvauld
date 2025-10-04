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
	import {
		GoBack,
		BackArrow,
		ClosePanel,
		Replace,
		ReplaceAll,
		Regex,
		WholeWord,
		CaseSensitive,
	} from "@osvauld/icons";
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
		// Mark search UI as visible
		searchManager.setUIVisible(true);

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
		// Mark search UI as hidden
		searchManager.setUIVisible(false);

		unsubscribe?.();
		document.removeEventListener("keydown", handleGlobalKeydown);
		// Clear search when component is destroyed
		if (editorView) {
			clearSearch();
		}
	});

	function handleGlobalKeydown(event: KeyboardEvent) {
		if (event.key === "Escape") {
			hide();
		}
	}

	function hide() {
		// Mark search UI as hidden
		searchManager.setUIVisible(false);

		// Clear local state first - this triggers the reactive effect to clear search
		searchValue = "";
		replaceValue = "";

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
		<div class="flex items-center mx-0.5">
			<button
				class="px-1 h-8 flex items-center justify-center text-[#cccccc] hover:bg-[#2a2b2f]"
				class:text-[#007acc]={isReplaceMode}
				onclick={toggleReplaceMode}
				title="Toggle Replace"
				aria-label="Toggle Replace"
			>
				<GoBack
					width={12}
					height={12}
					color="#cccccc"
					className={isReplaceMode ? "-rotate-90" : "rotate-180"}
					bgColor="transparent"
				/>
			</button>
		</div>

		<!-- Search input with integrated controls -->
		<div class="flex items-center flex-1 py-0.5">
			<div
				class="flex items-center relative flex-1 border border-[#2a2b2f] mr-0.5"
			>
				<input
					bind:this={searchInput}
					bind:value={searchValue}
					class="w-full h-7 px-2 pr-16 bg-transparent text-[#cccccc] text-sm border-none outline-none placeholder:text-[#6a737d]"
					type="text"
					autocorrect="off"
					autocomplete="off"
					placeholder="Find"
					onkeydown={handleSearchKeydown}
				/>

				<div class="flex items-center px-0.5">
					<!-- Toggle options -->
					<button
						class="p-1 flex items-center justify-center text-xs font-mono hover:bg-[#2a2d2e]"
						class:text-livnotePink={caseSensitive}
						class:text-[#6a737d]={!caseSensitive}
						onclick={() => (caseSensitive = !caseSensitive)}
						title="Match Case"
					>
						<CaseSensitive size={16} />
					</button>
					<button
						class="p-1 flex items-center justify-center text-xs font-mono hover:bg-[#2a2d2e]"
						class:text-livnotePink={wholeWord}
						class:text-[#6a737d]={!wholeWord}
						onclick={() => (wholeWord = !wholeWord)}
						title="Match Whole Word"
					>
						<WholeWord size={16} />
					</button>
					<button
						class="p-1 flex items-center justify-center text-xs font-mono hover:bg-[#2a2d2e]"
						class:text-livnotePink={useRegex}
						class:text-[#6a737d]={!useRegex}
						onclick={() => (useRegex = !useRegex)}
						title="Use Regular Expression"
					>
						<Regex size={16} />
					</button>
				</div>
			</div>

			<!-- Navigation buttons -->
			<div class="flex mx-0.5">
				<div
					class="w-14 text-xs text-[#6a737d] flex justify-start items-center pointer-events-none whitespace-nowrap"
				>
					{#if searchState.totalMatches > 0}
						<span class="pl-1"
							>{searchState.currentMatch} of {searchState.totalMatches}</span
						>
					{:else}
						No results
					{/if}
				</div>
				<button
					class="w-6 h-8 flex items-center justify-center text-[#cccccc] hover:bg-[#2a2d2e] disabled:text-[#6a737d]"
					onclick={findPrevious}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Previous match (Shift+Enter)"
					aria-label="Previous match"
				>
					<BackArrow className="rotate-90" size={16} />
				</button>
				<button
					class="w-6 h-8 flex items-center justify-center text-[#cccccc] hover:bg-[#2a2d2e] disabled:text-[#6a737d]"
					onclick={findNext}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Next match (Enter)"
					aria-label="Next match"
				>
					<BackArrow className="-rotate-90" size={16} />
				</button>
			</div>
		</div>

		<!-- Options and controls -->
		<div class="flex items-center px-0.5">
			<!-- Close button -->
			<button
				class="w-6 h-8 flex items-center justify-center text-[#6a737d] hover:bg-[#2a2d2e]"
				onclick={hide}
				title="Close (Escape)"
				aria-label="Close"
			>
				<ClosePanel size={16} />
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
						autocorrect="off"
						autocomplete="off"
						autocapitalize="off"
						placeholder="Replace"
						onkeydown={handleReplaceKeydown}
					/>
				</div>
			</div>

			<!-- Replace controls -->
			<div class="flex items-center px-1">
				<button
					class="w-8 h-7 my-0.5 flex items-center justify-center hover:bg-[#2a2d2e] text-[#6a737d]"
					onclick={replaceCurrent}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Replace"
					aria-label="Replace"
				>
					<Replace size={16} />
				</button>
				<button
					class="w-8 h-7 my-0.5 flex items-center justify-center hover:bg-[#2a2d2e] text-[#6a737d]"
					onclick={replaceAll}
					disabled={!searchState.isActive || searchState.totalMatches === 0}
					title="Replace All"
					aria-label="Replace All"
				>
					<ReplaceAll size={16} />
				</button>
			</div>
		</div>
	{/if}
</div>
