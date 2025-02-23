<script lang="ts">
	import Arrow from "@osvauld/password-manager-common/icons/rightArrow.svelte";
	import Home from "@osvauld/password-manager-common/icons/mobileHome.svelte";
	import Star from "@osvauld/password-manager-common/icons/star.svelte";
	import VaultManager from "../views/VaultManager.svelte";
	import { currentVault, selectedCategory } from "../../store/desktop.ui.store";
	import { CATEGORIES } from "@osvauld/password-manager-common/utils/credentialUtils";
	import { LL } from "@osvauld/password-manager-common/i18n/i18n-svelte";
	import { LocalStorageService } from "@osvauld/password-manager-common";
	import { StorageService } from "@osvauld/password-manager-common";
	import MobileNote from "@osvauld/password-manager-common/icons/mobileNote.svelte";
	import FavStar from "@osvauld/password-manager-common/icons/favStar.svelte";

	let selectedSection = "home";
	let localSelectedCredential = 0;
	let vaultManagerActive = false;
	let hoveredCredential = null;

	let availablecredentials = [
		{ id: 1, favourite: true },
		{ id: 2, favourite: false },
		{ id: 3, favourite: true },
		{ id: 4, favourite: false },
		{ id: 5, favourite: true },
		{ id: 6, favourite: false },
		{ id: 7, favourite: true },
		{ id: 8, favourite: false },
		{ id: 9, favourite: true },
		{ id: 10, favourite: false },
		{ id: 11, favourite: true },
		{ id: 12, favourite: true },
		{ id: 13, favourite: false },
		{ id: 14, favourite: true },
		{ id: 15, favourite: false },
		{ id: 16, favourite: true },
	];

	// const handleSectionChange = (section) => {
	// 	localSelectedCredential = "";
	// 	selectedSection = section;
	// 	selectedCategory.set("");
	// };

	// const handleFavourite = (section) => {
	// 	localSelectedCredential = "";
	// 	selectedSection = section;
	// 	selectedCategory.set("favourites");
	// };

	// const handleCategoryFilter = (type, id) => {
	// 	selectedSection = "";
	// 	localSelectedCredential = id;
	// 	selectedCategory.set(type);
	// };

	// $: if ($currentVault) {
	// 	localSelectedCredential = "";
	// 	selectedSection = "home";
	// 	selectedCategory.set("");
	// 	// Storing Current vault for persisting
	// 	(async () => {
	// 		const currentVaultString = JSON.stringify($currentVault);
	// 		await StorageService.setCurrentVault(currentVaultString);
	// 	})();
	// }
</script>

<nav
	class="w-[360px] h-full py-10 px-4 whitespace-nowrap"
	aria-label="Main Navigation">
	<div class="relative">
		<button
			class="w-full text-[26px] text-osvauld-fieldText font-medium leading-6 bg-osvauld-frameblack rounded-lg border border-osvauld-defaultBorder px-4 py-2 flex justify-between items-center capitalize trun"
			aria-label="Switch Vault"
			aria-controls="vaultSelector"
			aria-expanded="false"
			on:click="{() => (vaultManagerActive = !vaultManagerActive)}">
			<span class="flex-1 truncate text-left py-1"
				>{$currentVault.id === "all" ? "All Vaults" : $currentVault.name}</span
			><span
				class="shrink-0 transition-transform duration-300 {vaultManagerActive
					? '-rotate-90'
					: 'rotate-90'}"><Arrow color="#F2F2F0" size="{24}" /></span
			></button>
		{#if vaultManagerActive}
			<VaultManager bind:vaultManagerActive instance="nav" />
		{/if}
	</div>
	<div
		class="border-y border-osvauld-borderColor text-osvauld-fieldText flex flex-col my-6 py-1 gap-1">
		<ul class="space-y-1 font-light text-base text-" role="list">
			<li>
				<button
					class="w-full flex items-center gap-3 p-3 rounded-lg
                       transition-colors
                       {selectedSection === 'home'
						? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
						: ''}"
					on:click="{() => {}}"
					aria-current="{selectedSection === 'home' ? 'page' : undefined}">
					<Home color="{selectedSection === 'home' ? '#F2F2F0' : '#85889C'}" />
					<span>Home</span>
				</button>
			</li>
			<li>
				<button
					class="w-full flex items-center gap-3 p-3 rounded-lg
                       {selectedSection === 'favourites'
						? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
						: ''}"
					on:click="{() => {}}"
					aria-current="{selectedSection === 'favourites'
						? 'page'
						: undefined}">
					<Star
						color="{selectedSection === 'favourites'
							? '#F2F2F0'
							: '#85889C'}" />
					<span>Favourites</span>
				</button>
			</li>
		</ul>
	</div>
	<ul
		class="font-light text-base space-y-1 text-osvauld-fieldText max-h-3/4 overflow-y-scroll px-1 scrollbar-thin"
		role="list">
		{#each availablecredentials as credential}
			<li>
				<button
					class="w-full flex items-center justify-start gap-3 p-3 rounded-lg
                       transition-colors
						  {hoveredCredential === credential.id
						? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
						: ''}"
					on:mouseenter="{() => (hoveredCredential = credential.id)}"
					on:mouseleave="{() => (hoveredCredential = null)}"
					on:click="{() => {}}">
					<MobileNote
						color="{hoveredCredential === credential.id
							? '#F2F2F0'
							: '#85889C'}" />
					<span>{credential.id}</span>
					<span class="ml-auto">
						{#if credential.favourite}
							<FavStar />
						{:else}
							<Star />
						{/if}</span>
				</button>
			</li>
		{/each}
	</ul>
</nav>
