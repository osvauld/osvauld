<script lang="ts">
	import { slide, fly, blur } from "svelte/transition";
	import Add from "../../../icons/add.svelte";
	import MobileHome from "../../../icons/mobileHome.svelte";
	import { onMount, onDestroy } from "svelte";
	import { sendMessage } from "../../../lib/components/dashboard/helper";
	import { vaults, currentVault } from "../store/desktop.ui.store";
	import { LL } from "../../../i18n/i18n-svelte";

	export let vaultManagerActive;
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

	const handleVaultCreation = async (event) => {
		event.preventDefault();
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
		vaultManagerActive = false;
	};

	const handleVaultSwitch = (vault) => {
		currentVault.set(vault);
		vaultManagerActive = false;
	};

	const handleNewVaultInput = (e) => {
		e.preventDefault();
		e.stopPropagation();
		newVaultInputActive = !newVaultInputActive;
	};

	onMount(async () => {
		await fetchAllVaults();
	});
</script>

<div
	class="fixed inset-0 bg-transparent z-[999]"
	on:click="{() => (vaultManagerActive = false)}">
	<div
		class="absolute top-56 left-4 w-[360px] h-[25rem] overflow-hidden scrollbar-thin border border-osvauld-iconblack bg-osvauld-ninjablack rounded-2xl px-2 pt-2 pb-3 flex flex-col gap-2 text-lg"
		style="width: calc(360px - 2rem);"
		id="vaultSelector"
		in:fly
		on:click|stopPropagation>
		<div class="h-full flex flex-col">
			<div class="flex-1 overflow-y-auto space-y-2 scrollbar-thin p-1">
				{#each $vaults as vault (vault.id)}
					{@const isActive = $currentVault.id === vault.id}
					<button
						class="h-[48px] w-full p-4 text-mobile-textPrimary flex items-center rounded-lg hover:bg-osvauld-frameblack"
						class:bg-mobile-bgLight="{isActive}"
						class:text-mobile-textTertiary="{isActive}"
						on:click|stopPropagation="{() => handleVaultSwitch(vault)}">
						<span
							><MobileHome color="{isActive ? '#F2F2F0' : '#85889C'}" /></span>
						<span class="grow text-left pl-2 capitalize max-w-full truncate"
							>{vault.id === "all" ? $LL.all() : vault.name}</span>
					</button>
				{/each}
			</div>
			<div class="p-2 bg-osvauld-ninjablack">
				{#if newVaultInputActive}
					<form
						class="rounded-[20px] border border-mobile-bgLight px-3 pt-3 pb-4 text-mobile-textPrimary flex flex-col gap-3"
						in:slide
						out:slide
						on:submit|preventDefault|stopPropagation="{handleVaultCreation}">
						<span class="text-lg text-center">{$LL.newVault()} </span>
						<hr class="h-px border-0 bg-mobile-bgLight" />
						<div class="flex flex-col grow gap-1">
							<label for="new-vault-name" class="text-sm"
								>{$LL.addATitle()}</label>
							<input
								type="text"
								id="new-vault-name"
								class="bg-mobile-bgSeconary border-0 outline-0 focus:ring-0 rounded-lg"
								autocomplete="off"
								autocorrect="off"
								use:autofocus
								bind:value="{newVaultName}" />
							<button
								type="submit"
								class="h-[48px] flex justify-center items-center gap-1 rounded-lg bg-mobile-highlightBlue text-mobile-bgPrimary font-medium text-lg mt-6"
								>{$LL.createNewVault()} <Add color="#000" /></button>
						</div>
					</form>
				{:else}
					<button
						on:click="{handleNewVaultInput}"
						class="h-[48px] w-full flex justify-center items-center gap-1 rounded-lg border-2 border-mobile-bgHighlight p-4 active:bg-mobile-bgLight text-mobile-textActive"
						>{$LL.createNewVault()} <Add color="#85889C" /></button>
				{/if}
			</div>
		</div>
	</div>
</div>
