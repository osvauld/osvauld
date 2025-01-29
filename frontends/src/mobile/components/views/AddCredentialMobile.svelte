<script lang="ts">
	import { onMount } from "svelte";
	import MobileDownArrow from "../../../icons/mobileDownArrow.svelte";
	import {
		CATEGORIES,
		credentialFieldsUpdater,
	} from "../../../utils/CredentialUtils";

	import {
		selectedCredentialType,
		categorySelection,
		currentVault,
		currentLayout,
		bottomNavActive,
		vaults,
	} from "../../store/mobile.ui.store";

	import { addCredentialHandler } from "../../../utils/addCredentialHelper";
	let selectedCategory = {};
	let defaultVaultId;

	const directToHome = () => {
		selectedCredentialType.set("");
		categorySelection.set(false);
		currentLayout.set("home");
		bottomNavActive.set(true);
	};

	let credentialFields = [];
	const addCredentialHandlerFunc = async () => {
		await addCredentialHandler(
			{
				name: "test",
				description: "test",
				credentialFields,
				credentialType: selectedCategory.type,
			},
			// if current vault.id is "all", get id of Default Folder and replace it here, that way, if user tries to add a credential from Default/All Vaults view, It goes somewhere (Default folder)
			$currentVault.id === "all" ? defaultVaultId : $currentVault.id,
		);
		directToHome();
	};

	onMount(() => {
		selectedCategory = CATEGORIES.find(
			(item) => item.id === $selectedCredentialType,
		);
		const defaultVault = $vaults.find((vault) => vault.name === "Default");

		defaultVaultId = defaultVault?.id;
		credentialFields = credentialFieldsUpdater(selectedCategory.type);
		bottomNavActive.set(false);
	});
</script>

<nav
	class="w-full h-[48px] pr-3 flex justify-start items-center gap-1 flex-shrink-0">
	<button on:click="{directToHome}" class="p-2 pr-1">
		<span class="inline-block rotate-90">
			<MobileDownArrow />
		</span>
	</button>
	<svelte:component this="{selectedCategory.icon}" color="#85889C" />
	<h1 class="text-mobile-textPrimary text-2xl font-medium">
		{selectedCategory?.type}
	</h1>
</nav>

<div class="grow p-4 text-mobile-textPrimary">
	<span>Enter your {selectedCategory?.type} details</span>
	<div
		class="max-h-full overflow-y-auto scrollbar-thin flex flex-col gap-4 mt-2">
		{#each credentialFields as field (field.fieldName)}
			<input
				type="text"
				bind:value="{field.fieldValue}"
				placeholder="{field.fieldName}"
				class="w-full bg-mobile-bgPrimary border rounded-lg border-mobile-bgHighlight focus:border-mobile-borderActive focus:ring-0" />
		{/each}
		<textarea
			name="note"
			id="details"
			rows="5"
			class="bg-mobile-bgPrimary border rounded-lg border-mobile-bgHighlight focus:border-mobile-borderActive focus:ring-0"
			placeholder="Enter description about credential here"></textarea>
	</div>
</div>

<div
	class="h-[68px] flex-shrink-0 p-3 flex justify-between items-center text-mobile-bgPrimary">
	<button
		class="px-10 py-2.5 text-mobile-textSecondary bg-mobile-bgSeconary rounded-lg font-medium"
		on:click="{directToHome}">Cancel</button>
	<button
		class="px-10 py-2.5 bg-osvauld-carolinablue rounded-lg font-medium"
		on:click="{addCredentialHandlerFunc}">Add Secret</button>
</div>
