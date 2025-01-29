<script lang="ts">
	import {
		addCredentialModal,
		currentVault,
		credentialEditorModal,
		selectedCategoryForInput,
		selectedVaultForInput,
	} from "../store/desktop.ui.store";
	import ClosePanel from "../../../icons/closePanel.svelte";
	import RightArrow from "../../../icons/rightArrow.svelte";
	import VaultSelector from "../ui/VaultSelector.svelte";
	import Add from "../../../icons/add.svelte";
	import { CATEGORIES } from "../../utils/credentialUtils";

	let vaultSwitchActive = false;
	let selectedVault;
	let vaultNotASelected = false;

	const handleVaultSelection = (e) => {
		vaultSwitchActive = false;
		selectedVault = e.detail;
		selectedVaultForInput.set(selectedVault);
	};

	const handleCatergorySelection = (category) => {
		if ($currentVault.id === "all" && !selectedVault?.id) {
			vaultNotASelected = true;
			setTimeout(() => {
				vaultNotASelected = false;
			}, 1000);
			return;
		}
		selectedCategoryForInput.set(category);
		addCredentialModal.set(false);
		credentialEditorModal.set(true);
	};
</script>

<div
	class="fixed inset-0 z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[0.5px] flex justify-center items-center"
	on:click|preventDefault="{() => addCredentialModal.set(false)}">
	<div
		class="w-[49.5rem] h-[35.3rem] bg-osvauld-frameblack rounded-3xl border border-osvauld-activeBorder p-10">
		<div
			class="flex justify-between items-center text-osvauld-sideListTextActive text-3xl">
			<span>Add Credential to Osvauld</span><button
				on:click="{() => addCredentialModal.set(false)}"
				><ClosePanel size="{24}" /></button>
		</div>
		<hr
			class=" border-t border-osvauld-defaultBorder w-[calc(100%+5rem)] -translate-x-10 my-5" />

		{#if $currentVault.id === "all"}
			<div class="relative mb-5">
				<button
					class="w-full text-[26px] text-osvauld-fieldText font-medium leading-6 bg-osvauld-frameblack rounded-xl border border-osvauld-defaultBorder px-4 py-3 flex justify-between items-center capitalize whitespace-nowrap truncate"
					class:border-red-700="{vaultNotASelected}"
					aria-label="Switch Vault"
					aria-controls="vaultSelector"
					aria-expanded="false"
					on:click|stopPropagation="{() =>
						(vaultSwitchActive = !vaultSwitchActive)}"
					>{selectedVault?.id ? selectedVault.name : "Select Vault"}<span
						class="transition-transform duration-300 {vaultSwitchActive
							? '-rotate-90'
							: 'rotate-90'}"><RightArrow color="#F2F2F0" /></span
					></button>
				{#if vaultSwitchActive}
					<VaultSelector on:select="{handleVaultSelection}" />
				{/if}
			</div>
		{/if}
		<span class="text-sm text-osvauld-activeBorder ml-6">Select Category</span>
		<div
			class="text-base grid grid-cols-2 gap-3 px-1 py-2 mx-2 text-mobile-textPrimary"
			on:click|stopPropagation>
			{#each CATEGORIES as category (category.id)}
				<button
					class=" bg-mobile-bgSeconary px-3 py-2.5 h-11
               active:bg-mobile-bgHighlight/10 border border-osvauld-iconblack rounded-lg"
					on:click|stopPropagation="{() =>
						handleCatergorySelection(category.id)}">
					<div class="flex justify-start items-center">
						<svelte:component this="{category.icon}" color="#85889C" />
						<span class="text-left text-base pl-2 leading-tight"
							>{category.type}</span>
						<span class="ml-auto"> <Add color="#85889C" /></span>
					</div>
				</button>
			{/each}
		</div>
	</div>
</div>
