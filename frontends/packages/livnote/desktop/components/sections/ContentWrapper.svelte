<script>
	import {
		currentVault,
		selectedCategory,
		addCredentialModal,
		deleteConfirmationModal,
		noteViewLayout,
	} from "../../store/desktop.ui.store";
	import Add from "@osvauld/password-manager-common/icons/add.svelte";
	import Menu from "@osvauld/password-manager-common/icons/Menu.svelte";
	import Bin from "@osvauld/password-manager-common/icons/binIcon.svelte";
	import DownArrow from "@osvauld/password-manager-common/icons/downArrow.svelte";
	import Arrow from "@osvauld/password-manager-common/icons/rightArrow.svelte";
	import CredentialList from "../views/CredentialList.svelte";
	import { LL } from "@osvauld/password-manager-common/i18n/i18n-svelte";
	import VaultManager from "../views/VaultManager.svelte";
	import Star from "@osvauld/password-manager-common/icons/star.svelte";
	import { MobileHome } from "@osvauld/password-manager-common";

	let addCredentialHovered = false;
	let deleteBtnHoved = false;
	let vaultManagerActive = false;
	let selectedSection = "home";

	// const handleDeleteBtn = () => {
	// 	deleteConfirmationModal.set({ item: "folder", show: true });
	// };

	const handleSectionChange = (section) => {
		selectedSection = section;
	};
</script>

<div class="flex-1 flex flex-col overflow-hidden">
	<div class="py-5 px-16 flex items-center justify-start shrink-0">
		{#if $noteViewLayout}
			<h1
				class="flex-1 h-full truncate text-4xl font-light text-osvauld-sideListTextActive text-left capitalize">
				{$selectedCategory
					? $selectedCategory
					: $currentVault.id === "all"
						? "All Vaults"
						: $currentVault.name}
			</h1>
		{:else}
			<div class="relative shrink-0">
				<button
					class="min-w-[20.25rem] text-[26px] text-osvauld-fieldText font-medium leading-6 bg-osvauld-frameblack rounded-lg border border-osvauld-defaultBorder px-4 py-2 flex justify-between items-center capitalize truncate cursor-pointer"
					aria-label="Switch Vault"
					aria-controls="vaultSelector"
					aria-expanded="false"
					on:click="{() => (vaultManagerActive = !vaultManagerActive)}">
					<span class="flex-1 truncate text-left py-1"
						>{$currentVault.id === "all"
							? "All Vaults"
							: $currentVault.name}</span
					><span
						class="shrink-0 transition-transform duration-300 {vaultManagerActive
							? '-rotate-90'
							: 'rotate-90'}"><Arrow color="#F2F2F0" size="{24}" /></span
					></button>
				{#if vaultManagerActive}
					<VaultManager bind:vaultManagerActive />
				{/if}
			</div>
			<div
				class="mx-6 px-6 border-x border-osvauld-borderColor text-osvauld-fieldText flex gap-6 text-base">
				<button
					class="w-full flex items-center gap-2 px-3 py-3 rounded-lg
                       transition-colors
                       {selectedSection === 'home'
						? 'text-osvauld-fieldTextActive bg-osvauld-fieldActive'
						: ''}"
					on:click="{() => {
						handleSectionChange('home');
					}}"
					aria-current="{selectedSection === 'home' ? 'page' : undefined}">
					<MobileHome
						size="20"
						color="{selectedSection === 'home' ? '#BFC0CC' : '#85889C'}" />
					<span>Home</span>
				</button>

				<button
					class="w-full flex items-center gap-2 px-3 py-3 rounded-lg
                       {selectedSection === 'favourites'
						? 'text-osvauld-fieldTextActive bg-osvauld-fieldActive'
						: ''}"
					on:click="{() => {
						handleSectionChange('favourites');
					}}"
					aria-current="{selectedSection === 'favourites'
						? 'page'
						: undefined}">
					<Star
						color="{selectedSection === 'favourites' ? '#BFC0CC' : '#85889C'}"
						size="20" />
					<span>Favourites</span>
				</button>
			</div>
		{/if}

		<div
			class="ml-auto shrink-0 gap-4 flex justify-between items-center text-base">
			{#if $currentVault.id !== "all"}
				<button
					class="p-2"
					on:click|stopPropagation="{() => {}}"
					on:mouseenter="{() => (deleteBtnHoved = true)}"
					on:mouseleave="{() => (deleteBtnHoved = false)}"
					aria-label="Delete Folder"
					><Bin color="{deleteBtnHoved ? '#FF6A6A' : '#85889C'}" /></button>
			{/if}
			<span><Menu /></span>
			<button
				class="bg-osvauld-frameblack text-osvauld-textPassive flex justify-center items-center py-3 px-3 rounded-md ml-4"
				aria-label="Sort by latest">
				<span class="mr-2 pl-2">Latest</span>
				<span><DownArrow type="common" /></span>
			</button>
			<button
				class="rounded-md py-3 px-4 mx-2 flex justify-center items-center whitespace-nowrap border text-osvauld-textActive border-osvauld-iconblack hover:text-osvauld-frameblack hover:bg-osvauld-carolinablue transition-colors"
				on:mouseenter="{() => (addCredentialHovered = true)}"
				on:mouseleave="{() => (addCredentialHovered = false)}"
				on:click="{() => addCredentialModal.set(true)}">
				<span class="mr-2">Add new note</span>
				<Add color="{addCredentialHovered ? '#000' : '#A3A4B5'}" />
			</button>
		</div>
	</div>

	<CredentialList />
</div>
