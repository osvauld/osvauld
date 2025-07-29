<script lang="ts">
	import { uiState, dataState } from "../../state";
	import VaultManager from "./VaultManager.svelte";

	import {
		BinIcon as Bin,
		Star as EmptyStar,
		RightArrow as Arrow,
		MobileHome as Home,
		Add,
	} from "../../icons";

	let deleteBtnHoved = $state(false);
	let addCredentialHovered = $state(false);

	const handleDeleteBtn = (item: "folder" | "note") => {
		uiState.showDeleteConfirmation(item);
	};

	const handleAddNote = async () => {
		if (!dataState.currentVault || dataState.currentVault.id === "all") {
			uiState.showToast("Please add/select vault", false);
			return;
		}
		await dataState.addNote();
	};
</script>

<div class="py-10 px-11 flex items-center justify-start shrink-0">
	<div class="relative shrink-0">
		<button
			class="w-[20.25rem] max-w-[20.25rem] text-[26px] text-osvauld-fieldText font-medium leading-6 bg-osvauld-frameblack rounded-lg border border-osvauld-defaultBorder px-4 py-2 flex justify-between items-center capitalize truncate cursor-pointer"
			aria-label="Switch Vault"
			aria-expanded={uiState.vaultManagerActive}
			onclick={() => uiState.toggleVaultManager()}>
			<span class="flex-1 truncate text-left py-1"
				>{dataState.currentVault.id === "all"
					? "All Vaults"
					: dataState.currentVault.name}</span
			><span
				class="shrink-0 transition-transform duration-300 {uiState.vaultManagerActive
					? '-rotate-90'
					: 'rotate-90'}">
				<Arrow color="#F2F2F0" size={24} />
			</span>
		</button>
		{#if uiState.vaultManagerActive}
			<VaultManager position="noteList" />
		{/if}
	</div>
	<div
		class="mx-6 px-6 border-x border-osvauld-borderColor text-osvauld-fieldText flex gap-6 text-base">
		<button
			class="w-full flex items-center gap-2 px-3 py-3 rounded-lg
				   transition-colors
				   {!dataState.favoriteSelected
				? 'text-osvauld-fieldTextActive bg-osvauld-fieldActive'
				: ''}"
			onclick={() => dataState.toggleFavoriteView(false)}
			aria-current={!dataState.favoriteSelected ? "page" : undefined}>
			<Home
				size={20}
				color={!dataState.favoriteSelected ? "#BFC0CC" : "#85889C"} />
			<span>Home</span>
		</button>

		<button
			class="w-full flex items-center gap-2 px-3 py-3 rounded-lg
				   {dataState.favoriteSelected
				? 'text-osvauld-fieldTextActive bg-osvauld-fieldActive'
				: ''}"
			onclick={() => dataState.toggleFavoriteView(true)}
			aria-current={dataState.favoriteSelected ? "page" : undefined}>
			<EmptyStar
				color={dataState.favoriteSelected ? "#BFC0CC" : "#85889C"}
				size={20} />
			<span>Favourites</span>
		</button>
	</div>
	<div
		class="relative ml-auto shrink-0 gap-4 flex justify-end items-center text-base">
		{#if dataState.currentVault.id !== "all"}
			<button
				class="cursor-pointer rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive"
				onclick={(e) => {
					e.stopPropagation();
					handleDeleteBtn("folder");
				}}
				onmouseenter={() => (deleteBtnHoved = true)}
				onmouseleave={() => (deleteBtnHoved = false)}
				aria-label="Delete Folder"
				><Bin
					color={deleteBtnHoved ? "#FF6A6A" : "#85889C"}
					size={24} /></button>
		{/if}
		<button
			class=" rounded-lg p-2.5 flex justify-center items-center cursor-pointer {addCredentialHovered
				? 'bg-livnotelavender text-primarydark'
				: 'bg-osvauld-fieldActive text-osvauld-fieldText'}"
			onmouseenter={() => (addCredentialHovered = true)}
			onmouseleave={() => (addCredentialHovered = false)}
			onclick={handleAddNote}>
			<span class="mr-2 pl-2">New Note</span>
			<Add color={addCredentialHovered ? "#010109" : "#85889C"} size={24} />
		</button>
	</div>
</div>
