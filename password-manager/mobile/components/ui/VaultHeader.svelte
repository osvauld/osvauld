<script lang="ts">
	import BinIcon from "@osvauld/password-manager-common/icons/binIcon.svelte";
	import Menu from "@osvauld/password-manager-common/icons/Menu.svelte";
	import {
		currentVault,
		deleteConfirmationModal,
		bottomNavActive,
		vaultSwitchActive,
	} from "../../store/mobile.ui.store";
	import Location from "@osvauld/password-manager-common/icons/location.svelte";

	let vaultOptionsmodal = false;

	const handleVaultDeletion = () => {
		vaultOptionsmodal = false;
		deleteConfirmationModal.set({ item: "vault", show: true });
	};

	const handleVaultOptions = () => {
		vaultOptionsmodal = !vaultOptionsmodal;
	};

	const showVaultManager = () => {
		if ($vaultSwitchActive) {
			bottomNavActive.set(true);
		}

		vaultSwitchActive.set(!$vaultSwitchActive);
	};
</script>

<div
	class="fixed top-0 w-full flex justify-start items-center text-mobile-textPrimary text-2xl font-semibold px-4 py-2">
	<button
		class="p-2 bg-mobile-bgSeconary rounded-lg m-1"
		on:click|stopPropagation="{showVaultManager}">
		<Location />
	</button>
	<span class="max-w-3/4 truncate capitalize">{$currentVault.name}</span>

	{#if $currentVault.id !== "all"}
		<div class="relative ml-auto">
			<button
				class="bg-mobile-bgSeconary rounded-lg w-11 h-11 rotate-90 flex justify-center items-center"
				on:click="{handleVaultOptions}">
				<Menu />
			</button>

			{#if vaultOptionsmodal}
				<div
					class="bg-transparent fixed inset-0 z-40"
					role="presentation"
					aria-hidden="true"
					on:click|stopPropagation="{() => (vaultOptionsmodal = false)}">
				</div>
				<div
					class="w-[140px] h-[40px] border border-mobile-textSecondary bg-mobile-bgPrimary rounded-lg flex flex-col justify-start absolute top-[110%] right-0 text-base font-normal active:text-red-300 z-50">
					<button
						class="w-full flex justify-between p-2"
						on:click|stopPropagation="{handleVaultDeletion}"
						>Delete vault <BinIcon /></button>
				</div>
			{/if}
		</div>
	{/if}
</div>
