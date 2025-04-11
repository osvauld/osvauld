<script>
	import { preventDefault, stopPropagation } from 'svelte/legacy';

	import { fly } from "svelte/transition";
	import { onMount } from "svelte";

	import { Warning, ClosePanel } from "@osvauld/password-manager-common";

	import {
		currentNote,
		currentVault,
		deleteConfirmationModal,
		toastStore,
		refreshCredentialList,
		noteViewLayout,
	} from "../../store/desktop.ui.store";

	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";

	const deleteConfirmation = async () => {
		if ($deleteConfirmationModal.item == "folder") {
			await sendMessage("deleteFolder", {
				folderId: $currentVault.id,
			});
			currentVault.set({ id: "all", name: "all vaults" });
		} else if ($deleteConfirmationModal.item == "note") {
			console.log("Current note", $currentNote);
			await sendMessage("deleteResource", {
				resourceId: $currentNote.id,
			});
			noteViewLayout.set(false);
			refreshCredentialList.set(true);
		}

		// We need to collect response above and show toggle accordingly
		//
		toastStore.set({
			show: true,
			message: `${$deleteConfirmationModal.item} deleted successfully`,
			success: true,
		});
		deleteConfirmationModal.set({ item: "", show: false });
	};

	const withdrawCredentialDeleteModal = () => {
		deleteConfirmationModal.set({ item: "", show: false });
	};
</script>

<div
	class="fixed inset-0 flex items-center justify-center z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[2px]"
	onclick={preventDefault(withdrawCredentialDeleteModal)}
	role="presentation">
	<form
		class="p-4 bg-osvauld-frameblack border border-osvauld-activeBorder rounded-3xl w-[32rem] h-[14rem] flex flex-col items-start justify-center gap-3"
		in:fly
		onsubmit={stopPropagation(preventDefault(deleteConfirmation))}>
		<div class="flex justify-between items-center w-full">
			<span class="text-[21px] font-medium text-osvauld-quarzowhite capitalize"
				>Delete
				{$deleteConfirmationModal.item === "note" ? "note" : $currentVault.name}
				?
			</span>
			<button
				class="cursor-pointer p-2"
				onclick={stopPropagation(withdrawCredentialDeleteModal)}>
				<ClosePanel />
			</button>
		</div>
		<div
			class="border-b border-osvauld-iconblack w-[calc(100%+2rem)] -translate-x-4">
		</div>
		<div
			class=" w-full font-normal text-base flex justify-start items-center bg-osvauld-fieldActive rounded-lg gap-3 p-2">
			<div class="justify-center items-center flex">
				<Warning />
			</div>
			<div class="text-osvauld-textActive text-left">
				This action cannot be undone
			</div>
		</div>
		<div
			class="border-b border-osvauld-iconblack w-[calc(100%+2rem)] -translate-x-4">
		</div>
		<div class="flex justify-end items-center gap-4 w-full">
			<button
				class="font-medium text-base rounded-md py-[5px] px-[15px] text-osvauld-fadedCancel hover:bg-osvauld-cancelBackground hover:text-osvauld-quarzowhite transition-all"
				onclick={withdrawCredentialDeleteModal}>Cancel</button>
			<button
				class="border border-osvauld-dangerRed py-[5px] px-[15px] text-base font-medium text-osvauld-dangerRed rounded-md hover:bg-osvauld-dangerRed hover:text-osvauld-frameblack transition-all"
				type="submit"
				onclick={deleteConfirmation}
				>Delete {$deleteConfirmationModal.item}</button>
		</div>
	</form>
</div>
