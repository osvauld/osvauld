<script lang="ts">
	import { slide, fly, blur } from "svelte/transition";
	import { Add, MobileHome } from "@osvauld/password-manager-common";
	import { onMount, onDestroy } from "svelte";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	import {
		vaults,
		currentVault,
		noteViewLayout,
		type Vault,
	} from "../../store/desktop.ui.store";
	import { LL } from "@osvauld/password-manager-common/i18n/i18n-svelte";

	interface Props {
		vaultManagerActive: boolean;
		instance?: string;
	}

	let { vaultManagerActive = $bindable(), instance = "content" }: Props =
		$props();
	let newVaultInputActive = $state(false);
	let newVaultName = $state("");

	const autofocus = (node: HTMLElement) => {
		node.focus();
	};

	const fetchAllVaults = async () => {
		try {
			const resp = (await sendMessage("getFolder")) as Vault[];
			const updatedVaults: Vault[] = [
				{ id: "all", name: "All Vaults" },
				...resp,
			];
			vaults.set(updatedVaults);
		} catch (e) {
			console.log("Error received", e);
		}
	};

	const handleVaultCreation = async (event: Event) => {
		// Stop event propagation to prevent the modal from closing
		event.stopPropagation();

		// Prevent the default form submission behavior
		event.preventDefault();

		try {
			console.log("sending vault creation request");
			await sendMessage("addFolder", {
				name: newVaultName,
				description: "",
			});
		} catch (e) {
			console.log("Vault creation failed");
		}

		await fetchAllVaults();
		const newVault = $vaults.find((vault) => vault.name === newVaultName);
		if (newVault) {
			currentVault.set(newVault);
		}
		newVaultName = "";
		vaultManagerActive = false;
	};

	const handleVaultSwitch = (vault: Vault) => {
		currentVault.set(vault);
		vaultManagerActive = false;
		noteViewLayout.set(false);
	};

	const handleNewVaultInput = (e: Event) => {
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
	role="presentation"
	onclick={() => (vaultManagerActive = false)}>
	<div
		class={`absolute  w-[20rem] h-[25rem] overflow-hidden scrollbar-thin border border-osvauld-iconblack bg-osvauld-ninjablack rounded-2xl px-2 pt-2 pb-3 flex flex-col gap-2 text-lg ${instance === "content" ? "top-56 left-11 " : "top-56 left-4"}`}
		style="width: calc(360px - 2rem);"
		id="vaultSelector"
		in:fly>
		<div class="h-full flex flex-col">
			<div class="flex-1 overflow-y-auto space-y-2 scrollbar-thin p-1">
				{#each $vaults as vault (vault.id)}
					{@const isActive = $currentVault.id === vault.id}
					<button
						class="h-[48px] w-full p-4 text-mobile-textPrimary flex items-center rounded-lg hover:bg-osvauld-frameblack"
						class:bg-mobile-bgLight={isActive}
						class:text-osvauld-sideListTextActive={isActive}
						onclick={(e) => {
							e.stopPropagation();
							handleVaultSwitch(vault);
						}}>
						<span><MobileHome color={isActive ? "#F2F2F0" : "#85889C"} /></span>
						<span class="grow text-left pl-2 capitalize max-w-full truncate"
							>{vault.id === "all" ? "All Vaults" : vault.name}</span>
					</button>
				{/each}
			</div>
			<div class="p-2 bg-osvauld-ninjablack">
				{#if newVaultInputActive}
					<form
						class="rounded-[20px] border border-mobile-bgLight px-3 pt-3 pb-4 text-mobile-textPrimary flex flex-col gap-3"
						in:slide
						out:slide
						onsubmit={handleVaultCreation}>
						<div
							class="w-full h-full"
							role="none"
							onclick={(e) => e.stopPropagation()}
							onkeydown={(e) =>
								e.key === "Escape" && (vaultManagerActive = false)}>
							<span class="text-lg text-center">New Folder </span>
							<span class="w-full border-b border-osvauld-modalFieldActive"
							></span>
							<div class="flex flex-col grow gap-1">
								<label for="new-vault-name" class="text-sm">Add Title</label>
								<input
									type="text"
									id="new-vault-name"
									class="bg-mobile-bgSeconary p-2 border-0 outline-0 focus:ring-0 rounded-lg"
									autocomplete="off"
									autocorrect="off"
									use:autofocus
									bind:value={newVaultName} />
								<button
									type="submit"
									class="h-[48px] flex justify-center items-center gap-1 rounded-lg bg-mobile-highlightBlue text-mobile-bgPrimary font-medium text-lg mt-6"
									>Create new folder <Add color="#000" /></button>
							</div>
						</div>
					</form>
				{:else}
					<button
						onclick={handleNewVaultInput}
						class="h-[48px] w-full flex justify-center items-center gap-1 rounded-lg border-2 border-mobile-bgHighlight p-4 active:bg-mobile-bgLight text-mobile-textActive"
						>Create new folder<Add color="#85889C" /></button>
				{/if}
			</div>
		</div>
	</div>
</div>
