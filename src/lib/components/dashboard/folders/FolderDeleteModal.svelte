<script>
	import ClosePanel from "../../basic/icons/closePanel.svelte";
	import {
		modalManager,
		DeleteConfirmationModal,
		showMoreOptions,
		folderStore,
		selectedFolder,
		credentialStore,
	} from "../store";
	import { fly } from "svelte/transition";
	import { Warning } from "../icons";
	import { removeFolder } from "../apis";
	import { setFolderStore } from "../../../store/storeHelper";

	function withdrawFolderDeleteModal() {
		showMoreOptions.set(false);
		DeleteConfirmationModal.set(false);
		modalManager.set(null);
	}

	async function DeleteConfirmation() {
		if ($selectedFolder && $selectedFolder.id === $modalManager.id) {
			selectedFolder.set(null);
		}
		await removeFolder($modalManager.id);
		withdrawFolderDeleteModal();
		await setFolderStore();
		credentialStore.set([]);
	}
</script>

<button
	class="fixed inset-0 flex items-center justify-center z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[2px]"
	on:click|preventDefault={withdrawFolderDeleteModal}
>
	<div
		class="p-4 bg-osvauld-frameblack border border-osvauld-activeBorder rounded-3xl w-[32rem] h-[17.6rem] flex flex-col items-start justify-center gap-3"
		in:fly
	>
		<div class="flex justify-between items-center w-full">
			<span class="text-[21px] font-medium text-osvauld-quarzowhite"
				>Delete {$modalManager.name} folder</span
			>
			<button
				class="cursor-pointer p-2"
				on:click|preventDefault={withdrawFolderDeleteModal}
			>
				<ClosePanel />
			</button>
		</div>
		<div
			class="border-b border-osvauld-iconblack w-[calc(100%+2rem)] -translate-x-4"
		></div>
		<div
			class=" h-[140px] w-full font-normal text-base flex justify-center items-start bg-osvauld-fieldActive rounded-lg gap-3 p-2"
		>
			<div class="w-16 h-9 justify-center items-center flex">
				<Warning />
			</div>
			<div class="text-osvauld-textActive text-left">
				Are you sure? <br /> All
				<span class="font-semibold text-osvauld-quarzowhite"
					>credentials</span
				> within this folder will be permanently deleted. Users and groups
				with access to this folder will no longer be able to view or manage
				the credentials
			</div>
		</div>
		<div
			class="border-b border-osvauld-iconblack w-[calc(100%+2rem)] -translate-x-4"
		></div>
		<div class="flex justify-end items-center gap-6 w-full">
			<button
				class="text-osvauld-fadedCancel font-medium text-base"
				on:click|preventDefault={withdrawFolderDeleteModal}
				>Cancel</button
			>
			<button
				class="border border-osvauld-iconblack py-[5px] px-[15px] text-base font-medium text-osvauld-dangerRed rounded-md"
				on:click|preventDefault={DeleteConfirmation}
				>Delete Folder</button
			>
		</div>
	</div>
</button>
