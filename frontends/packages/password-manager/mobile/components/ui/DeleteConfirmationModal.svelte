<script>
	import ClosePanel from "@osvauld/password-manager-common/icons/closePanel.svelte";
	import {
		selectedCredential,
		currentVault,
		deleteConfirmationModal,
		refreshCredentialList,
		currentLayout,
		credentialLayoutType,
		bottomNavActive,
	} from "../../store/mobile.ui.store";
	import LL from "@osvauld/password-manager-common/i18n/i18n-svelte";
	import { renderRelevantHeading } from "@osvauld/password-manager-common/utils/credentialUtils";
	import { fly } from "svelte/transition";
	import Warning from "@osvauld/password-manager-common/icons/warning.svelte";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	import { onMount } from "svelte";

	const DeleteConfirmation = async () => {
		if ($deleteConfirmationModal.item == "vault") {
			await sendMessage("deleteFolder", {
				folderId: $currentVault.id,
			});
			currentVault.set({ id: "all", name: "all vaults" });
		} else if ($deleteConfirmationModal.item == "credential") {
			await sendMessage("deleteCredential", {
				credentialId: $selectedCredential.id,
			});
			refreshCredentialList.set(true);
		}

		bottomNavActive.set(true);
		credentialLayoutType.set("addition");
		currentLayout.set("home");
		deleteConfirmationModal.set({ item: "", show: false });
	};

	const withdrawCredentialDeleteModal = () => {
		deleteConfirmationModal.set({ item: "", show: false });
	};

	// onMount(() => {
	// 	console.log("Inside delete Modal", $selectedCredential);
	// });
</script>

<div
	class="fixed inset-0 flex items-center justify-center z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[2px] px-4"
	on:click|stopPropagation={withdrawCredentialDeleteModal}>
	<div
		class="p-3 bg-mobile-navBlue border border-osvauld-activeBorder rounded-3xl w-full h-[21.12rem] flex flex-col items-start justify-start gap-3 overflow-hidden"
		in:fly>
		<div class="flex justify-between items-center w-full pt-2 pb-5">
			<span
				class="text-[21px] font-medium text-osvauld-quarzowhite capitalize truncate"
				>{$LL.delete()}
				{$deleteConfirmationModal.item === "credential"
					? renderRelevantHeading(
							$selectedCredential.data.credentialFields,
							$selectedCredential.data.credentialType,
							$selectedCredential.id,
						)
					: $currentVault.name} ?
			</span>
			<!-- <button
				class="cursor-pointer p-2"
				on:click|stopPropagation="{withdrawCredentialDeleteModal}">
				<ClosePanel />
			</button> -->
		</div>
		<div
			class="border-b border-osvauld-iconblack w-[calc(100%+1rem)] -translate-x-2">
		</div>
		<div
			class=" w-full font-normal text-base flex flex-col justify-start items-start bg-osvauld-fieldActive rounded-lg gap-3 p-3">
			<div class="justify-center items-center flex">
				<Warning />
			</div>
			<div class="text-osvauld-textActive text-left">
				{$LL.thisActionCannotBeUndone()}
			</div>
		</div>
		<div
			class="border-b border-osvauld-iconblack w-[calc(100%+1rem)] -translate-x-2">
		</div>
		<div class="flex flex-col justify-start items-start gap-4 w-full">
			<button
				class="w-full border border-osvauld-dangerRed py-2.5 px-auto text-lg font-medium text-osvauld-dangerRed rounded-md hover:bg-osvauld-dangerRed hover:text-osvauld-frameblack transition-all"
				type="submit"
				on:click|stopPropagation={DeleteConfirmation}
				>{$LL.delete()} {$deleteConfirmationModal.item}</button>
			<button
				class="w-full font-medium text-lg rounded-md py-2.5 px-auto bg-mobile-bgSeconary text-osvauld-fadedCancel hover:bg-osvauld-cancelBackground hover:text-osvauld-quarzowhite transition-all"
				on:click|stopPropagation={withdrawCredentialDeleteModal}
				>{$LL.cancel()}</button>
		</div>
	</div>
</div>
