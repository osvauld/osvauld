<script lang="ts">
	import { fly } from "svelte/transition";
	import { Warning, ClosePanel } from "@osvauld/icons";
	// Import the centralized state
	import { dataState, uiState } from "../../state";
	import { sendMessage } from "../../utils/helper";

	const deleteConfirmation = async (e: Event) => {
		// Prevent default form submission
		e.preventDefault();

		// Get current deleteConfirmation data
		const item = uiState.deleteConfirmationModal.item;

		if (item === "folder") {
			await sendMessage("deleteFolder", {
				folderId: dataState.currentVault.id,
			});
			// Refresh vaults list to reflect the deletion
			await dataState.fetchVaults();
			await dataState.fetchAllNotes();
			// Reset to All Vaults
			dataState.switchVault({ id: "all", name: "Home" });
		} else if (item === "note") {
			const currentNoteId = dataState.getCurrentNoteId();
			if (!currentNoteId) return;

			await sendMessage("deleteResource", {
				resourceId: currentNoteId,
			});

			// Clear the current note and update UI
			dataState.clearCurrentNote();

			// Refresh all notes
			await dataState.fetchAllNotes();
		}

		// Show success toast
		uiState.showToast(`${item} deleted successfully`, true);

		// Close the confirmation modal
		uiState.hideDeleteConfirmation();
	};

	const handleModalBackdropClick = (e: MouseEvent) => {
		// Close modal when clicking the backdrop (but not its children)
		if (e.target === e.currentTarget) {
			uiState.hideDeleteConfirmation();
		}
	};

	const handleCancelClick = (e: MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		uiState.hideDeleteConfirmation();
	};
</script>

<div
	class="fixed inset-0 flex items-center justify-center z-50 bg-osvauld-backgroundBlur/80 backdrop-filter backdrop-blur-[2px]"
	onclick={handleModalBackdropClick}
	role="presentation">
	<form
		class="p-4 bg-osvauld-frameblack border border-osvauld-activeBorder rounded-3xl w-[32rem] h-[14rem] flex flex-col items-start justify-center gap-3"
		in:fly
		onsubmit={deleteConfirmation}>
		<div class="flex justify-between items-center w-full">
			<span class="text-[21px] font-medium text-osvauld-quarzowhite capitalize"
				>Delete
				{uiState.deleteConfirmationModal.item === "note"
					? "note"
					: dataState.currentVault.name}
				?
			</span>
			<button
				class="cursor-pointer p-2"
				onclick={handleCancelClick}
				type="button">
				<ClosePanel />
			</button>
		</div>
		<div
			class="border-b border-osvauld-iconblack w-[calc(100%+2rem)] -translate-x-4">
		</div>
		<div
			class="w-full font-normal text-base flex justify-start items-center bg-osvauld-fieldActive rounded-lg gap-3 p-2">
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
				type="button"
				onclick={handleCancelClick}>
				Cancel
			</button>
			<button
				class="border border-osvauld-dangerRed py-[5px] px-[15px] text-base font-medium text-osvauld-dangerRed rounded-md hover:bg-osvauld-dangerRed hover:text-osvauld-frameblack transition-all"
				type="submit">
				Delete {uiState.deleteConfirmationModal.item}
			</button>
		</div>
	</form>
</div>
