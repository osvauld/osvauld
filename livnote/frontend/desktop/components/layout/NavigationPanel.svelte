<script lang="ts">
	import {
		RightArrow as Arrow,
		MobileHome as Home,
		Star,
		MobileNote,
		BlueClose,
	} from "../../icons";

	// Import the centralized state
	import { dataState, uiState } from "../../state";

	// Import VaultManager
	import VaultManager from "../ui/VaultManager.svelte";
	import { slide } from "svelte/transition";

	// Define an enum for section selection
	enum Section {
		HOME = "home",
		FAVOURITES = "favourites",
	}

	// Local UI state using $state
	let selectedSection = $state<Section>(Section.HOME);
	let hoveredCredential = $state<string | null>(null);

	// Handle section changes
	function handleSectionChange(section: Section) {
		selectedSection = section;

		// Use the centralized state for favorites
		dataState.toggleFavoriteView(section === Section.FAVOURITES);
	}

	// Function to handle note selection
	function selectNote(note: any) {
		//TODO: save old note
		dataState.switchNote(note.id);
	}

	// Toggle navigation panel (close)
	function closeNavigationPanel() {
		uiState.toggleNavigationPanel(false);
		uiState.resetNavigationPanelManualToggle();
		document.documentElement.style.setProperty("--min-editor-width", "0px");
	}
</script>

<!-- Navigation panel that can be hidden -->
{#if uiState.showNavigationPanel}
	<nav
		class="w-[22.5rem] shrink-0 h-full max-h-full py-10 px-4 whitespace-nowrap relative"
		in:slide={{ axis: "x", duration: 200 }}
		out:slide={{ axis: "x", duration: 200 }}
		aria-label="Main Navigation">
		<!-- Close button (only shown when manually toggled) -->
		{#if uiState.isNavigationPanelManuallyToggled}
			<button
				aria-label="Close navigation panel"
				class="absolute top-1.5 right-3 p-1.5 mb-2 rounded-md bg-osvauld-fieldActive hover:bg-osvauld-iconblack transition-colors cursor-pointer"
				onclick={closeNavigationPanel}>
				<BlueClose color="#85889C" />
			</button>
		{/if}

		<div class="relative">
			<button
				class="w-full text-[26px] text-osvauld-fieldText font-medium leading-6 bg-osvauld-frameblack rounded-lg border border-osvauld-defaultBorder px-4 py-2 flex justify-between items-center capitalize truncate"
				aria-label="Switch Vault"
				aria-controls="vaultSelector"
				aria-expanded={uiState.vaultManagerActive}
				onclick={() => uiState.toggleVaultManager()}>
				<span class="flex-1 truncate text-left py-1"
					>{dataState.currentVault.id === "all"
						? "All Vaults"
						: dataState.currentVault.name}</span
				><span
					class="shrink-0 transition-transform duration-300 {uiState.vaultManagerActive
						? '-rotate-90'
						: 'rotate-90'}"><Arrow color="#F2F2F0" size={24} /></span
				></button>
			{#if uiState.vaultManagerActive}
				<VaultManager position="navigationPanel" />
			{/if}
		</div>
		<div
			class="border-b border-osvauld-borderColor text-osvauld-fieldText flex flex-col my-6 py-1 gap-1">
			<!-- <ul class="space-y-1 font-light text-base text-" role="list">
				<li>
					<button
						class="w-full flex items-center gap-3 p-3 rounded-lg
												transition-colors
												{!dataState.currentNote && selectedSection === Section.HOME
							? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
							: ''}"
						on:click={() => handleSectionChange(Section.HOME)}
						aria-current={selectedSection === Section.HOME ? 'page' : undefined}>
						<Home
							color={!dataState.currentNote && selectedSection === Section.HOME
								? '#F2F2F0'
								: '#85889C'} />
						<span>Home</span>
					</button>
				</li>
				<li>
					<button
						class="w-full flex items-center gap-3 p-3 rounded-lg
												{!dataState.currentNote && selectedSection === Section.FAVOURITES
							? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
							: ''}"
						on:click={() => handleSectionChange(Section.FAVOURITES)}
						aria-current={selectedSection === Section.FAVOURITES
							? 'page'
							: undefined}>
						<Star
							color={!dataState.currentNote && selectedSection === Section.FAVOURITES
								? '#F2F2F0'
								: '#85889C'} />
						<span>Favourites</span>
					</button>
				</li>
			</ul> -->
		</div>

		{#if dataState.isDataLoading}
			<div class="text-osvauld-fieldText text-center p-4">Loading...</div>
		{:else}
			<ul
				class="font-light text-base space-y-1 text-osvauld-fieldText max-h-3/4 overflow-y-scroll px-1 scrollbar-thin"
				role="list">
				{#each dataState.filteredNotes as note (note.id)}
					{@const hoveredOrSelected =
						hoveredCredential === note.id ||
						(dataState.currentNote && dataState.currentNote.id === note.id)}
					<li>
						<button
							class="w-full flex items-center justify-between gap-3 p-3 rounded-lg
								transition-colors
								{hoveredOrSelected
								? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
								: ''}"
							onmouseenter={() => (hoveredCredential = note.id)}
							onmouseleave={() => (hoveredCredential = null)}
							onclick={() => selectNote(note)}>
							<div class="flex items-center gap-3 truncate">
								<span class="shrink-0">
									<MobileNote
										color={hoveredOrSelected ? "#F2F2F0" : "#85889C"} />
								</span>
								<span class="truncate">
									{note?.title}
								</span>
							</div>
						</button>
					</li>
				{/each}
			</ul>
		{/if}
	</nav>
{/if}
