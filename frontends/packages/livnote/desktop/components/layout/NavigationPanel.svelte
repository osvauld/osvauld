<script lang="ts">
	import {
		RightArrow as Arrow,
		MobileHome as Home,
		Star,
		MobileNote,
	} from "@osvauld/password-manager-common";

	import { LL } from "@osvauld/password-manager-common/i18n/i18n-svelte";

	// Import the centralized state
	import { dataState } from "../../state/data.state";
	import { uiState } from "../../state/ui.state";

	// Import VaultManager
	import VaultManager from "../ui/VaultManager.svelte";

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
	function selectNote(note) {
		dataState.switchNote(note);
	}
</script>

<nav
	class="w-[22.5rem] shrink-0 h-full max-h-full py-10 px-4 whitespace-nowrap"
	aria-label="Main Navigation">
	<div class="relative">
		<button
			class="w-full text-[26px] text-osvauld-fieldText font-medium leading-6 bg-osvauld-frameblack rounded-lg border border-osvauld-defaultBorder px-4 py-2 flex justify-between items-center capitalize trun"
			aria-label="Switch Vault"
			aria-controls="vaultSelector"
			aria-expanded={uiState.vaultManager.isActive}
			onclick={() => uiState.toggleVaultManager("nav")}>
			<span class="flex-1 truncate text-left py-1"
				>{dataState.currentVault.id === "all"
					? "All Vaults"
					: dataState.currentVault.name}</span
			><span
				class="shrink-0 transition-transform duration-300 {uiState.vaultManager
					.isActive
					? '-rotate-90'
					: 'rotate-90'}"><Arrow color="#F2F2F0" size={24} /></span
			></button>
		{#if uiState.vaultManager.isActive && uiState.vaultManager.source === "nav"}
			<VaultManager />
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
								<MobileNote color={hoveredOrSelected ? "#F2F2F0" : "#85889C"} />
							</span>
							<span class="truncate">
								{note?.data.title ? note.data.title : "untitled note"}
							</span>
						</div>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</nav>
