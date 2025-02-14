<script lang="ts">
	import Arrow from "@osvauld/password-manager-common/icons/rightArrow.svelte";
	import Home from "@osvauld/password-manager-common/icons/mobileHome.svelte";
	import Star from "@osvauld/password-manager-common/icons/star.svelte";
	import VaultManager from "../views/VaultManager.svelte";
	import { currentVault, selectedCategory } from "../../store/desktop.ui.store";
	import { CATEGORIES } from "@osvauld/password-manager-common/utils/credentialUtils";
	import { LL } from "@osvauld/password-manager-common/i18n/i18n-svelte";

	let selectedSection = "home";
	let localSelectedCategory = "";
	let vaultManagerActive = false;

	const handleSectionChange = (section) => {
		localSelectedCategory = "";
		selectedSection = section;
		selectedCategory.set("");
	};

	const handleFavourite = (section) => {
		localSelectedCategory = "";
		selectedSection = section;
		selectedCategory.set("favourites");
	};

	const handleCategoryFilter = (type, id) => {
		selectedSection = "";
		localSelectedCategory = id;
		selectedCategory.set(type);
	};

	$: if ($currentVault) {
		localSelectedCategory = "";
		selectedSection = "home";
		selectedCategory.set("");
	}
</script>

<nav
	class="w-[360px] py-10 px-4 whitespace-nowrap"
	aria-label="Main Navigation">
	<div class="relative">
		<button
			class="w-full text-[26px] text-osvauld-fieldText font-medium leading-6 bg-osvauld-frameblack rounded-lg border border-osvauld-defaultBorder px-4 py-2 flex justify-between items-center capitalize trun"
			aria-label="Switch Vault"
			aria-controls="vaultSelector"
			aria-expanded="false"
			on:click="{() => (vaultManagerActive = !vaultManagerActive)}">
			<span class="flex-1 truncate text-left py-1"
				>{$currentVault.id === "all" ? $LL.all() : $currentVault.name}</span
			><span
				class="shrink-0 transition-transform duration-300 {vaultManagerActive
					? '-rotate-90'
					: 'rotate-90'}"><Arrow color="#F2F2F0" size="24" /></span
			></button>
		{#if vaultManagerActive}
			<VaultManager bind:vaultManagerActive />
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
					on:click="{() => handleSectionChange('home')}"
					aria-current="{selectedSection === 'home' ? 'page' : undefined}">
					<Home color="{selectedSection === 'home' ? '#F2F2F0' : '#85889C'}" />
					<span>{$LL.tabs.home()}</span>
				</button>
			</li>
			<li>
				<button
					class="w-full flex items-center gap-3 p-3 rounded-lg
                       {selectedSection === 'favourites'
						? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
						: ''}"
					on:click="{() => handleFavourite('favourites')}"
					aria-current="{selectedSection === 'favourites'
						? 'page'
						: undefined}">
					<Star
						color="{selectedSection === 'favourites'
							? '#F2F2F0'
							: '#85889C'}" />
					<span>{$LL.nav.favourites()}</span>
				</button>
			</li>
		</ul>
	</div>
	<ul class="font-light text-base space-y-1 text-osvauld-fieldText" role="list">
		{#each CATEGORIES as category}
			<li>
				<button
					class="w-full flex items-center gap-3 p-3 rounded-lg
                       transition-colors
						  {localSelectedCategory === category.id
						? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
						: ''}"
					on:click="{() => handleCategoryFilter(category.type, category.id)}"
					aria-current="{localSelectedCategory === category.id
						? 'page'
						: undefined}">
					<svelte:component
						this="{category.icon}"
						color="{localSelectedCategory === category.id
							? '#F2F2F0'
							: '#85889C'}" />
					<span>{$LL.types[category.id]()}</span>
				</button>
			</li>
		{/each}
	</ul>
</nav>
