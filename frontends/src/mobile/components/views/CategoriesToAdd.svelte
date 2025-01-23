<script lang="ts">
	import { CATEGORIES } from "../../../utils/CredentialUtils";
	import LL from "../../../i18n/i18n-svelte";
	import {
		currentLayout,
		selectedCredentialType,
		vaults,
		currentVault,
		vaultSwitchActive,
	} from "../../store/mobile.ui.store";
	import RightArrow from "../../../icons/rightArrow.svelte";
	import Add from "../../../icons/add.svelte";
	import { onMount } from "svelte";
	let vaultNotSelected = false;

	const handleClick = (categoryId: string) => {
		if ($currentVault.id === "all") {
			vaultNotSelected = true;
			setTimeout(() => {
				vaultNotSelected = false;
			}, 1000);
			return;
		}
		selectedCredentialType.set(categoryId);
		currentLayout.set("credential");
	};

	onMount(() => {
		console.log("Available Vaults", $vaults);
	});
</script>

<div class="p-3">
	<div class="text-mobile-textPrimary font-semibold text-2xl">
		Add New Secret
	</div>
	<div class="text-base font-normal text-mobile-textSecondary">
		Choose our secret category to add
	</div>
</div>

{#if $vaults.length === 1}
	<button
		class="text-osvauld-dangerRed text-base flex justify-between items-center mx-3 px-3 py-2 mb-2 bg-mobile-bgSeconary rounded-md active:bg-mobile-bgHighlight/10 transition-colors touch-manipulation"
		class:shadow-sm="{vaultNotSelected}"
		class:shadow-red-400="{vaultNotSelected}"
		on:click="{() => vaultSwitchActive.set(true)}"
		>Please add vault to proceed <RightArrow /></button>
{/if}

<div
	class="text-base grid grid-cols-2 gap-3 px-1 py-2 mx-2 text-mobile-textPrimary">
	{#each CATEGORIES as category (category.id)}
		<button
			class=" bg-mobile-bgSeconary rounded-lg px-3 py-2.5 h-[100px]
               active:bg-mobile-bgHighlight/10 transition-colors touch-manipulation"
			on:click="{() => handleClick(category.id)}">
			<div class="flex justify-between items-center">
				<svelte:component this="{category.icon}" color="#85889C" />
				<span> <Add color="#85889C" /></span>
			</div>
			<div class="text-left text-base pt-2 leading-tight">
				{$LL.types[category.id]()}
			</div>
		</button>
	{/each}
</div>
