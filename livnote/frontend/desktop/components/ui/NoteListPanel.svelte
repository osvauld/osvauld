<script lang="ts">
	import { uiState, dataState } from "../../state";
	import ShareFolder from "../modals/ShareFolder.svelte";

	import {
		BinIcon as Bin,
		Star as EmptyStar,
		FavStar as Star,
		RightArrow as Arrow,
		MobileHome as Home,
		Add,
		TwoPeople,
	} from "@osvauld/icons";

	let deleteBtnHoved = $state(false);
	let showShareFolderList = $state(false);
	const handleDeleteBtn = (item: "folder" | "note") => {
		uiState.showDeleteConfirmation(item);
	};

	const handleAddNote = async () => {
		// If in "All Notes" view, use the default folder
		if (dataState.currentVault.id === "all") {
			const defaultFolder = dataState.vaults.find((v) => v.default === true);
			if (defaultFolder) {
				await dataState.addNote(defaultFolder.id);
				return;
			}
			// Fallback: if no default folder exists, show toast
			uiState.showToast("Please select a folder", false);
			return;
		}
		await dataState.addNote();
	};
</script>

<div class="py-6 pr-4 flex items-center justify-start shrink-0">
	<!-- <div class="relative shrink-0">
		<button
			class="w-[20.25rem] max-w-[20.25rem] text-[26px] text-osvauld-fieldText font-light leading-6 rounded-lg border border-osvauld-defaultBorder px-4 py-2 flex justify-between items-center capitalize truncate cursor-pointer"
			aria-label="Switch Vault"
			aria-expanded={uiState.vaultManagerActive}
			onclick={() => uiState.toggleVaultManager()}
		>
		<span class="flex-1 truncate text-left py-1"
			>{dataState.currentVault.id === "all"
				? "All Notes"
				: dataState.currentVault.name}</span
			><span
				class="shrink-0 transition-transform duration-300 {uiState.vaultManagerActive
					? '-rotate-90'
					: 'rotate-90'}"
			>
				<Arrow color="#F2F2F0" size={24} />
			</span>
		</button>
		{#if uiState.vaultManagerActive}
			<FolderManager position="noteList" />
		{/if}
	</div> -->
	<div class="mr-auto flex items-center gap-6 text-base min-w-0 flex-1">
		<!-- Current folder title -->
		<div
			class="text-4xl py-2 text-osvauld-sideListTextActive font-light capitalize min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap"
			aria-label="Current folder: {dataState.currentVault.id === 'all'
				? 'All Notes'
				: dataState.currentVault.name}"
			title={dataState.currentVault.id === "all"
				? "All Notes"
				: dataState.currentVault.name}
		>
			{dataState.currentVault.id === "all"
				? "All Notes"
				: dataState.currentVault.name}
		</div>

		<!-- Favourites button -->
		<button
			class="flex items-center gap-2 px-3 py-1.5 rounded-lg cursor-pointer
				   transition-colors text-textActive
				   {dataState.favoriteSelected
				? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
				: 'hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive'}"
			onclick={() => dataState.toggleFavoriteView(!dataState.favoriteSelected)}
			aria-current={dataState.favoriteSelected ? "page" : undefined}
		>
			<span>Favourites</span>
			{#if dataState.favoriteSelected}
				<Star size={20} />
			{:else}
				<EmptyStar size={20} />
			{/if}
		</button>
	</div>
	<div
		class="relative ml-auto shrink-0 gap-2 flex justify-end items-center text-base"
	>
		{#if dataState.currentVault.id !== "all" && dataState.currentVault.default !== true}
			<button
				class="rounded-lg p-2 text-sm font-semibold flex justify-center items-center bg-livnotelavender text-primarydark border border-osvauld-iconblack cursor-pointer"
				onclick={() => (showShareFolderList = true)}
				aria-label="Share Folder"
				aria-haspopup="dialog"
				aria-expanded={showShareFolderList}
			>
				<span class="mr-2 pl-2 whitespace-nowrap">Share Folder</span>
				<TwoPeople size={20} />
			</button>
			<button
				class="cursor-pointer rounded-lg p-2 flex justify-center items-center bg-osvauld-fieldActive"
				onclick={(e) => {
					e.stopPropagation();
					handleDeleteBtn("folder");
				}}
				onmouseenter={() => (deleteBtnHoved = true)}
				onmouseleave={() => (deleteBtnHoved = false)}
				aria-label="Delete Folder"
				><Bin
					color={deleteBtnHoved ? "#FF6A6A" : "#a3a4b5"}
					size={20}
				/></button
			>
		{/if}
		<button
			class="rounded-lg p-2 text-sm flex justify-center items-center cursor-pointer transition-colors duration-150 bg-osvauld-fieldActive text-textActive hover:bg-livnotelavender hover:text-primarydark group"
			onclick={handleAddNote}
		>
			<span class="mr-2 pl-2">New Note</span>
			<span class="">
				<Add color="currentColor" size={20} />
			</span>
		</button>
	</div>

	{#if showShareFolderList}
		<div
			class="bg-transparent fixed inset-0"
			role="presentation"
			aria-hidden="true"
			onclick={(e) => {
				e.stopPropagation();
				showShareFolderList = false;
			}}
		></div>
		<ShareFolder bind:showShareList={showShareFolderList} />
	{/if}
</div>
