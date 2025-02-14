<script lang="ts">
	import { slide } from "svelte/transition";
	import Add from "@osvauld/password-manager-common/icons/add.svelte";
	import RoundedInfo from "@osvauld/password-manager-common/icons/roundedInfo.svelte";
	import RightArrow from "@osvauld/password-manager-common/icons/rightArrow.svelte";
	import {
		vaultSwitchActive,
		currentVault,
		vaults,
		bottomNavActive,
		credentialListWithType,
		selectedCredential,
		currentLayout,
		vaultSwitchForAddingCredential,
	} from "../../store/mobile.ui.store";
	import { onMount } from "svelte";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";

	let newVaultInputActive = false;
	let newVaultName = "";

	const autofocus = (node) => {
		node.focus();
	};

	const fetchAllVaults = async () => {
		try {
			const resp = await sendMessage("getFolder");
			const updatedVaults = [{ id: "all", name: "All Vaults" }, ...resp];
			vaults.set(updatedVaults);
		} catch (e) {
			console.log("Error received", e);
		}
	};

	const handleFolderCreation = async () => {
		// bottomNavActive.set(true);
		try {
			await sendMessage("addFolder", {
				name: newVaultName,
				description: "",
			});
		} catch (e) {
			console.log("Vault creation failed");
		}

		await fetchAllVaults();
		currentVault.set($vaults.find((vault) => vault.name === newVaultName));
		newVaultName = "";
		newVaultInputActive = false;
		vaultSwitchActive.set(false);
	};

	const selectVault = (vault) => {
		vaultSwitchActive.set(false);
		newVaultInputActive = false;
		currentVault.set(vault);
		bottomNavActive.set(true);
		credentialListWithType.set("");
		selectedCredential.set({});
		if ($vaultSwitchForAddingCredential) {
			vaultSwitchForAddingCredential.set(false);
			return;
		} else {
			currentLayout.set("home");
		}
	};

	const goBack = () => {
		vaultSwitchActive.set(false);
	};

	onMount(async () => {
		await fetchAllVaults();
	});
</script>

<button
	class="fixed top-0 left-0 w-full h-full bg-osvauld-backgroundBlur backdrop-blur-[0.1px]"
	on:click|stopPropagation|preventDefault="{goBack}"
	type="button"
	role="dialog"
	aria-hidden="true"
	aria-label="Close overlay">
</button>
<div
	class="absolute w-full h-auto bottom-5 border-t-[1px] border-mobile-textSecondary bg-mobile-bgPrimary rounded-t-2xl px-2 pt-2 pb-16 flex flex-col gap-2 text-lg"
	in:slide
	on:click|preventDefault|stopPropagation>
	{#if newVaultInputActive}
		<div
			class="h-[250px] rounded-[20px] border border-mobile-bgLight px-3 pt-3 pb-4 text-mobile-textPrimary flex flex-col gap-3"
			in:slide
			out:slide>
			<span class="text-lg text-center">New Vault</span>
			<hr class="h-px border-0 bg-mobile-bgLight" />
			<div class="flex flex-col grow gap-1">
				<label for="new-vault-name" class="text-sm">Add a title</label>
				<input
					type="text"
					id="new-vault-name"
					class="bg-mobile-bgSeconary border-0 outline-0 focus:ring-0 rounded-lg"
					use:autofocus
					bind:value="{newVaultName}" />
				<button
					type="button"
					class="h-[48px] flex justify-center items-center gap-1 rounded-lg bg-mobile-highlightBlue text-mobile-bgPrimary font-medium text-lg mt-6"
					on:click|stopPropagation="{handleFolderCreation}"
					>Create New Vault <Add color="#000" /></button>
			</div>
		</div>
	{:else}
		{#each $vaults as vault (vault.id)}
			{@const isActive = $currentVault.name === vault.name}
			<button
				on:click="{() => selectVault(vault)}"
				class="h-[48px] p-4 text-mobile-textPrimary flex items-center rounded-lg"
				class:bg-mobile-bgLight="{isActive}"
				class:text-mobile-textTertiary="{isActive}">
				<span><RoundedInfo color="{isActive ? '#F2F2F0' : '#85889C'}" /></span>
				<span class="grow text-left pl-2 capitalize">{vault.name}</span>
				<span><RightArrow color="{isActive ? '#F2F2F0' : '#85889C'}" /></span>
			</button>
		{/each}
		<button
			type="submit"
			on:click="{() => (newVaultInputActive = true)}"
			class="h-[48px] flex justify-center items-center gap-1 rounded-lg border-2 border-mobile-bgHighlight p-4 active:bg-mobile-bgLight text-mobile-textActive"
			>Create New Vault <Add color="#85889C" /></button>
	{/if}
</div>
