<script lang="ts">
	import { Lens, ClosePanel, EditIcon, InfoIcon } from "../icons";
	import ExistingListParent from "../components/ExistingListParent.svelte";
	import Toggle from "../Toggle.svelte";
	import {
		accessListSelected,
		selectedFolder,
		buttonRef,
		toastStore,
	} from "../store";
	import {
		fetchFolderUsers,
		removeUserFromFolder,
		editFolderPermissionForUser,
		fetchFolderGroups,
		removeGroupFromFolder,
		editFolderPermissionForGroup,
	} from "../apis";
	import { clickOutside } from "../helper";
	import { derived } from "svelte/store";
	import { onMount } from "svelte";
	import { fly } from "svelte/transition";
	let existingUserData = [];
	let existingGroupsData = [];
	let selectedTab = "Groups";
	let editPermissionTrigger = false;
	let infoOnHover = false;
	let showInfoTab = false;

	export const buttonCoords = derived(buttonRef, ($buttonRef) => {
		if ($buttonRef) {
			const rect = $buttonRef.getBoundingClientRect();
			const rightVal = rect.right + window.scrollX;
			return {
				top: rect.top + window.scrollY + rect.height + 10,
				right: rightVal,
			};
		}
		return { top: 0, left: 0 };
	});

	function handleClickOutside() {
		accessListSelected.set(false);
		buttonRef.set(null);
	}

	const toggleSelect = async (e: any) => {
		selectedTab = e.detail;
		await existingItems();
	};

	const existingItems = async () => {
		if ($selectedFolder !== null && selectedTab === "Users") {
			const responseJson = await fetchFolderUsers($selectedFolder.id);
			existingUserData = responseJson.data;
		} else {
			existingUserData.length = 0;
		}
		if ($selectedFolder !== null && selectedTab === "Groups") {
			const reponseJson = await fetchFolderGroups($selectedFolder.id);
			existingGroupsData = reponseJson.data;
		} else {
			existingGroupsData.length = 0;
		}
	};

	const removeExistingUser = async (e: any) => {
		await removeUserFromFolder($selectedFolder.id, e.detail.id);
		await existingItems();
	};

	const removeExistingGroup = async (e: any) => {
		await removeGroupFromFolder($selectedFolder.id, e.detail.groupId);
		await existingItems();
	};

	const handlePermissionChange = async (e: any) => {
		let editPermissionResponse;
		if (selectedTab === "Users") {
			editPermissionResponse = await editFolderPermissionForUser(
				$selectedFolder.id,
				e.detail.item.id,
				e.detail.permission,
			);
		} else {
			editPermissionResponse = await editFolderPermissionForGroup(
				$selectedFolder.id,
				e.detail.item.groupId,
				e.detail.permission,
			);
		}

		if (editPermissionResponse.success) {
			toastStore.set({
				show: editPermissionResponse.success,
				message: editPermissionResponse.message,
				type: true,
			});
		}
		await existingItems();
	};

	onMount(async () => {
		await existingItems();
	});
</script>

<div
	class="absolute w-[29rem] min-h-[30rem] max-h-[40rem] p-4 z-50 bg-osvauld-frameblack border border-osvauld-iconblack rounded-2xl"
	style="top: {$buttonCoords.top}px; right: {window.innerWidth -
		$buttonCoords.right}px;"
	use:clickOutside
	on:clickedOutside="{handleClickOutside}"
	in:fly
>
	<div class="flex justify-between items-center p-3">
		<span class="font-sans text-osvauld-quarzowhite text-2xl font-normal"
			>Access List
			<button
				class="ml-2 pt-1"
				on:mouseenter="{() => (infoOnHover = true)}"
				on:mouseleave="{() => (infoOnHover = false)}"
				on:click="{() => (showInfoTab = !showInfoTab)}"
				><InfoIcon color="{infoOnHover ? '#BFC0CC' : '#4D4F60'}" /></button
			></span
		>
		<button class="p-2" on:click="{handleClickOutside}"><ClosePanel /></button>
	</div>

	{#if showInfoTab}
		<div
			class="relative h-auto w-full px-4 py-2 mx-auto flex justify-between items-start rounded-lg cursor-pointer mb-3 bg-osvauld-fieldActive"
		>
			<span class="w-[12%] mt-1"> <InfoIcon /> </span>
			<p class="text-sm text-osvauld-sheffieldgrey font-normal">
				This folder contains credentials you have access directly or indirectly
				through below assignments
			</p>
		</div>
	{/if}
	<div class="flex justify-around items-center">
		<Toggle on:select="{toggleSelect}" />
		<div class="flex justify-end items-center w-full">
			<button
				class="p-2 rounded-lg {editPermissionTrigger
					? 'bg-osvauld-cardshade'
					: ''}  {$selectedFolder.accessType === 'manager'
					? 'visible'
					: 'hidden'}"
				on:click="{() => {
					editPermissionTrigger = !editPermissionTrigger;
				}}"
			>
				<EditIcon color="{editPermissionTrigger ? '#89B4FA' : '#6E7681'}" />
			</button>
		</div>
	</div>
	<div class="p-2rounded-lg max-h-[65vh]">
		<div
			class="h-[1.875rem] w-full px-2 mx-auto flex justify-start items-center border border-osvauld-iconblack rounded-lg cursor-pointer"
		>
			<Lens />
			<input
				type="text"
				class="h-[1.60rem] w-full bg-osvauld-frameblack border-0 text-osvauld-quarzowhite placeholder-osvauld-placeholderblack border-transparent text-base focus:border-transparent focus:ring-0 cursor-pointer"
				placeholder="Search for users"
			/>
		</div>

		<div
			class="overflow-y-auto scrollbar-thin min-h-0 max-h-[35vh] bg-osvauld-frameblack w-full flex flex-col justify-center items-center"
		>
			{#if selectedTab === "Users"}
				<ExistingListParent
					isUser="{true}"
					{editPermissionTrigger}
					existingItemsData="{existingUserData}"
					on:remove="{(e) => removeExistingUser(e)}"
					on:permissionChange="{(e) => handlePermissionChange(e)}"
				/>
			{:else}
				<ExistingListParent
					{editPermissionTrigger}
					isUser="{false}"
					existingItemsData="{existingGroupsData}"
					on:remove="{(e) => removeExistingGroup(e)}"
					on:permissionChange="{(e) => handlePermissionChange(e)}"
				/>
			{/if}
		</div>
	</div>
</div>
