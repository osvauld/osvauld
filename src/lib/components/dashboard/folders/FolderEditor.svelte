<script lang="ts">
	import { createFolder, renameFolder } from "../apis";
	import {
		showAddFolderDrawer,
		folderStore,
		selectedFolder,
		toastStore,
		showFolderRenameDrawer,
	} from "../store";
	import { ClosePanel } from "../icons";
	import { setFolderStore } from "../../../store/storeHelper";
	import { fly } from "svelte/transition";
	import Toggle from "../Toggle.svelte";

	let name = $showFolderRenameDrawer ? $selectedFolder.name : "";
	let selectedTab = "shared";

	let description = $showFolderRenameDrawer ? $selectedFolder.description : "";

	function autofocus(node: any) {
		node.focus();
	}

	const renameFolderFunc = async () => {
		const payload = {
			name: name,
			description: description,
		};
		const renameResponse = await renameFolder(payload, $selectedFolder.id);
		await setFolderStore();
		showFolderRenameDrawer.set(false);
		const actionMessage = renameResponse.success
			? "Folder Successfully Renamed"
			: "Failed to Rename folder";
		toastStore.set({
			type: renameResponse.success,
			message: actionMessage,
			show: true,
		});
	};

	const addFolderFunc = async () => {
		const payload = {
			name: name,
			description: description,
			sharedFolder: selectedTab === "shared",
		};
		const folderResponse = await createFolder(payload);
		await setFolderStore();
		for (const folder of $folderStore) {
			if (folder.id == folderResponse.data.folderId) {
				selectedFolder.set(folder);
				break;
			}
		}
		showAddFolderDrawer.set(false);
		const actionMessage = folderResponse.success
			? "Folder Successfully Created"
			: "Failed to Create folder";
		toastStore.set({
			type: folderResponse.success,
			message: actionMessage,
			show: true,
		});
	};

	const handleClose = (message) => {
		showFolderRenameDrawer.set(false);
		showAddFolderDrawer.set(false);
	};

	const toggleSelect = (e) => {
		selectedTab = e.detail;
	};
</script>

<form
	class="p-4 bg-osvauld-frameblack border border-osvauld-activeBorder rounded-3xl w-[32rem] h-[25rem] flex flex-col items-start justify-center gap-3"
	on:submit|preventDefault="{$showFolderRenameDrawer
		? renameFolderFunc
		: addFolderFunc}"
	in:fly
	out:fly
>
	<div class="flex justify-between items-center w-full">
		<span class="text-[21px] font-medium text-osvauld-quarzowhite"
			>{$showFolderRenameDrawer ? "Rename Folder" : "Create Folder"}</span
		>
		<button
			class="cursor-pointer p-2"
			type="button"
			on:click|preventDefault="{handleClose}"
		>
			<ClosePanel />
		</button>
	</div>
	<Toggle on:select="{toggleSelect}" tabs="{['shared', 'private']}" />
	<div
		class="border-b border-osvauld-iconblack w-[calc(100%+2rem)] -translate-x-4"
	></div>

	<label for="name" class="font-bold text-base text-osvauld-textActive"
		>{$showFolderRenameDrawer ? "Folder Name" : "Name:"}</label
	>
	<input
		id="name"
		type="text"
		use:autofocus
		required
		bind:value="{name}"
		class="py-1 rounded-sm items-center text-base bg-osvauld-frameblack border-osvauld-iconblack w-[95%] h-10 mx-2 focus:border-osvauld-iconblack focus:ring-0 form-input"
		autocomplete="off"
	/>

	<label for="description" class="font-bold text-base text-osvauld-textActive"
		>Description:</label
	>
	<textarea
		id="description"
		bind:value="{description}"
		class="py-1 rounded-sm items-center text-base bg-osvauld-frameblack border-osvauld-iconblack w-[95%] h-20 mx-2 focus:border-osvauld-iconblack focus:ring-0 form-textarea"
	></textarea>
	<div
		class="border-b border-osvauld-iconblack w-[calc(100%+2rem)] -translate-x-4"
	></div>

	<div class="flex justify-end items-center gap-4 w-full">
		<button
			class="text-osvauld-fadedCancel font-medium text-base py-[5px] px-[15px] rounded-md hover:text-osvauld-quarzowhite hover:bg-osvauld-cancelBackground"
			type="button"
			on:click|preventDefault="{handleClose}">Cancel</button
		>
		<button
			class="border border-osvauld-iconblack py-[5px] px-[15px] text-base font-medium text-osvauld-textActive rounded-md hover:bg-osvauld-carolinablue hover:border-osvauld-carolinablue hover:text-osvauld-frameblack"
			type="submit"
			disabled="{!name}"
			>{$showFolderRenameDrawer ? "Save Changes" : "Add Folder"}</button
		>
	</div>
</form>
