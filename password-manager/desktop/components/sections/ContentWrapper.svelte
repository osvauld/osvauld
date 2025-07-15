<script>
	import {
		currentVault,
		selectedCategory,
		addCredentialModal,
		deleteConfirmationModal,
	} from "../../store/desktop.ui.store";
	import Add from "@osvauld/password-manager-common/icons/add.svelte";
	import Menu from "@osvauld/password-manager-common/icons/Menu.svelte";
	import Bin from "@osvauld/password-manager-common/icons/binIcon.svelte";
	import DownArrow from "@osvauld/password-manager-common/icons/downArrow.svelte";
	import CredentialList from "../views/CredentialList.svelte";
	import { LL } from "@osvauld/password-manager-common/i18n/i18n-svelte";

	let addCredentialHovered = false;
	let deleteBtnHoved = false;

	const handleDeleteBtn = () => {
		deleteConfirmationModal.set({ item: "folder", show: true });
	};
</script>

<div class="flex-1 flex flex-col overflow-hidden">
	<div class="py-10 px-16 flex items-center justify-between shrink-0">
		<h1
			class="flex-1 h-full truncate text-4xl font-light text-osvauld-sideListTextActive text-left capitalize">
			{$selectedCategory
				? $selectedCategory
				: $currentVault.id === "all"
					? $LL.all()
					: $currentVault.name}
		</h1>

		<div class="shrink-0 gap-4 flex justify-between items-center ml-3">
			{#if $currentVault.id !== "all"}
				<button
					class="p-2"
					on:click|stopPropagation="{handleDeleteBtn}"
					on:mouseenter="{() => (deleteBtnHoved = true)}"
					on:mouseleave="{() => (deleteBtnHoved = false)}"
					aria-label="Delete Folder"
					><Bin color="{deleteBtnHoved ? '#FF6A6A' : '#85889C'}" /></button>
			{/if}
			<span><Menu /></span>
			<button
				class="bg-osvauld-frameblack text-osvauld-textPassive flex justify-center items-center py-2 px-3 text-sm rounded-md ml-4"
				aria-label="Sort by latest">
				<span class="mr-2 pl-2">{$LL.latest()}</span>
				<span><DownArrow type="common" /></span>
			</button>
			<button
				class="rounded-md py-3 px-4 mx-2 flex justify-center items-center whitespace-nowrap text-sm border text-osvauld-textActive border-osvauld-iconblack hover:text-osvauld-frameblack hover:bg-osvauld-carolinablue transition-colors"
				on:mouseenter="{() => (addCredentialHovered = true)}"
				on:mouseleave="{() => (addCredentialHovered = false)}"
				on:click="{() => addCredentialModal.set(true)}">
				<span class="mr-2">{$LL.addNewCredential()}</span>
				<Add color="{addCredentialHovered ? '#000' : '#A3A4B5'}" />
			</button>
		</div>
	</div>
	<CredentialList />
</div>
