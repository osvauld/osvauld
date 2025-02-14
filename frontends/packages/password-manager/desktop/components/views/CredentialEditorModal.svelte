<script lang="ts">
	import { onMount } from "svelte";
	import More from "@osvauld/password-manager-common//icons/more.svelte";
	import Share from "@osvauld/password-manager-common//icons/FolderShare.svelte";
	import Edit from "@osvauld/password-manager-common//icons/editIcon.svelte";
	import MobileHome from "@osvauld/password-manager-common//icons/mobileHome.svelte";
	import Star from "@osvauld/password-manager-common//icons/star.svelte";
	import ClosePanel from "@osvauld/password-manager-common//icons/closePanel.svelte";
	import {
		CATEGORIES,
		credentialFieldsUpdater,
	} from "@osvauld/password-manager-common/utils/credentialUtils";
	import { addCredentialHandler } from "@osvauld/password-manager-common//utils/addCredentialHelper";

	import {
		credentialEditorModal,
		currentVault,
		selectedCategoryForInput,
		selectedVaultForInput,
		refreshCredentialList,
	} from "../../store/desktop.ui.store";

	let credentialFields = [];
	let credentialType;

	const closeEditorModal = () => {
		selectedVaultForInput.set({});
		selectedCategoryForInput.set("");
		refreshCredentialList.set(true);
		credentialEditorModal.set(false);
	};

	const addCredentialHandlerFunc = async () => {
		await addCredentialHandler(
			{
				name: "test",
				description: "test",
				credentialFields,
				credentialType: credentialType,
			},
			// if current vault.id is "all", get id of Default Folder and replace it here, that way, if user tries to add a credential from Default/All Vaults view, It goes somewhere (Default folder)
			$currentVault.id === "all"
				? $selectedVaultForInput?.id
				: $currentVault.id,
		);
		closeEditorModal();
	};

	onMount(() => {
		credentialType = CATEGORIES.find(
			(item) => item.id === $selectedCategoryForInput,
		).type;
		credentialFields = credentialFieldsUpdater(credentialType);
	});
</script>

<div
	class="fixed inset-0 z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[0.5px] flex justify-end">
	<div
		class="w-[23.5rem] h-full flex flex-col bg-osvauld-ninjablack border-l border-l-osvauld-sideListHighlight p-3">
		<div class="w-full h-[48px] px-3 flex items-center gap-2 flex-shrink-0">
			<button
				on:click={closeEditorModal}
				class="p-2.5 rounded-lg bg-mobile-bgSeconary">
				<ClosePanel size={24} />
			</button>
			<span class="ml-auto p-2.5 rounded-lg bg-mobile-bgSeconary"
				><Edit size={24} /></span>
			<span class="p-2.5 rounded-lg bg-mobile-bgSeconary"><Share /></span>
			<span class="p-2.5 rounded-lg bg-mobile-bgSeconary"><Star /></span>
			<span class="p-2.5 rounded-lg bg-mobile-bgSeconary"
				><More color="#85889C" /></span>
		</div>
		<div
			class="text-mobile-textPrimary text-2xl font-medium flex flex-col pl-4 py-3">
			<span class="text-mobile-textTertiary">{credentialType}</span>
			<div
				class="flex text-sm font-light gap-1 items-center tracking-wider capitalize">
				<span><MobileHome size={14} color={"#85889C"} /></span>
				{$selectedVaultForInput?.id
					? $selectedVaultForInput?.name
					: $currentVault.name}
			</div>
		</div>
		<div
			class="grow p-4 text-mobile-textPrimary overflow-y-auto scrollbar-thin flex flex-col gap-2 mt-2">
			{#each credentialFields as field (field.fieldName)}
				<input
					type="text"
					bind:value={field.fieldValue}
					placeholder={field.fieldName}
					autocorrect="off"
					autocomplete="off"
					class="w-full bg-mobile-bgSeconary border p-2 rounded-lg border-mobile-bgHighlight focus:border-mobile-borderActive focus:ring-0 focus:outline-none" />
			{/each}
		</div>
		<div class="flex justify-center">
			<textarea
				name="note"
				id="details"
				rows="5"
				class="w-[calc(100%-1.5rem)] p-2 bg-mobile-bgSeconary border rounded-lg border-mobile-bgHighlight focus:border-mobile-borderActive focus:ring-0 focus:outline-none"
				placeholder="Enter description about credential here"></textarea>
		</div>
		<div
			class="h-[68px] flex-shrink-0 p-3 flex justify-between items-center text-mobile-bgPrimary">
			<button
				class="px-10 py-2.5 text-mobile-textSecondary"
				on:click={closeEditorModal}>Cancel</button>
			<button
				class="px-10 py-2.5 bg-osvauld-carolinablue rounded-lg font-medium"
				on:click={addCredentialHandlerFunc}>Save</button>
		</div>
	</div>
</div>
