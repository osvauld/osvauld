<script lang="ts">
	import { slide, fly } from "svelte/transition";
	import { Add, MobileHome } from "../../icons";
	import { sendMessage } from "../../utils/helper";
	import { dataState, uiState } from "../../state/";
	// import { LL } from "../../icons/i18n/i18n-svelte";
	import type { Vault } from "../../state/data.svelte.ts";

	// No need for vaultManagerActive prop anymore
	let newVaultInputActive = $state(false);
	let newVaultName = $state("");
	let { position = "noteList" }: { position: "navigationPanel" | "noteList" } =
		$props();
	let isCreationDisabled = $derived(newVaultName.trim().length === 0);

	const autofocus = (node: HTMLElement) => {
		node.focus();
	};

	const handleVaultCreation = async (event: Event) => {
		// Stop event propagation to prevent the modal from closing
		event.stopPropagation();

		// Prevent the default form submission behavior
		event.preventDefault();

		try {
			await sendMessage("addFolder", {
				name: newVaultName.trim(),
				description: "",
			});
			await dataState.fetchVaults();
			const newVault = dataState.vaults.find(
				(vault) => vault.name === newVaultName.trim(),
			);
			if (newVault) {
				dataState.switchVault(newVault);
			}
			// Reset form state reactively
			newVaultName = "";
			newVaultInputActive = false;
			uiState.toggleVaultManager();
		} catch (e) {
			console.log("Vault creation failed", e);
		}
	};

	const handleVaultSwitch = (vault: Vault) => {
		dataState.switchVault(vault);
		uiState.toggleVaultManager();
	};

	const handleNewVaultInput = (e: Event) => {
		e.preventDefault();
		e.stopPropagation();
		newVaultInputActive = !newVaultInputActive;
	};
</script>

<div
	class="fixed inset-0 bg-transparent z-[999]"
	role="presentation"
	onclick={() => uiState.toggleVaultManager()}>
	<div
		class={`absolute w-[20rem] h-[25rem] overflow-hidden scrollbar-thin border border-osvauld-iconblack bg-osvauld-ninjablack rounded-2xl px-2 pt-2 pb-3 flex flex-col gap-2 text-lg ${position === "noteList" ? "top-60 left-22" : "top-56 left-4"}`}
		style="width: calc(360px - 2rem);"
		id="vaultSelector"
		in:fly>
		<div class="h-full flex flex-col">
			<div class="flex-1 overflow-y-auto space-y-2 scrollbar-thin p-1">
				{#each dataState.vaults as vault (vault.id)}
					{@const isActive = dataState.currentVault.id === vault.id}
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
							>{vault.id === "all" ? "All Folders" : vault.name}</span>
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
								e.key === "Escape" && uiState.toggleVaultManager()}>
							<span class="text-sm text-center">New Folder </span>
							<span class="w-full border-b border-osvauld-modalFieldActive"
							></span>
							<div class="flex flex-col grow gap-1">
								<label for="new-vault-name" class="sr-only">Add Title</label>
								<input
									type="text"
									id="new-vault-name"
									class="bg-mobile-bgSeconary p-2 border-0 outline-0 focus:ring-0 rounded-lg placeholder:text-sm placeholder:text-osvauld-iconblack"
									placeholder="Title"
									autocomplete="off"
									autocorrect="off"
									use:autofocus
									bind:value={newVaultName} />
								<button
									type="submit"
									class="h-[48px] flex justify-center items-center gap-1 rounded-lg mt-6 text-base cursor-pointer"
									class:bg-signupGray={isCreationDisabled}
									class:text-white={isCreationDisabled}
									class:bg-livnotePink={!isCreationDisabled}
									class:text-black={!isCreationDisabled}
									disabled={isCreationDisabled}
									>Create new folder <Add
										color={isCreationDisabled ? "#fff" : "#000"} /></button>
							</div>
						</div>
					</form>
				{:else}
					<button
						onclick={handleNewVaultInput}
						class="h-[48px] w-full flex justify-center items-center gap-1 rounded-lg border-2 border-mobile-bgHighlight p-4 active:bg-mobile-bgLight text-mobile-textActive text-base cursor-pointer"
						>Create new folder<Add color="#85889C" /></button>
				{/if}
			</div>
		</div>
	</div>
</div>
