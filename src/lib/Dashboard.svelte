<script lang="ts">
	import { onMount } from "svelte";
	import LeftContainer from "./components/dashboard/LeftContainer.svelte";
	import RightContainer from "./components/dashboard/RightContainer.svelte";
	import MoreActions from "./components/dashboard/components/MoreActions.svelte";
	import CredentialDeleteModal from "./components/dashboard/credentials/CredentialDeleteModal.svelte";
	import FolderDeleteModal from "./components/dashboard/folders/FolderDeleteModal.svelte";
	import {
		showMoreOptions,
		DeleteConfirmationModal,
		modalManager,
		accessListSelected,
		showMoreGroupOptions,
		toastStore,
		promptPassword,
	} from "./store/ui.store";
	import { sendMessage } from "./components/dashboard/helper";
	import Welcome from "./components/popup/Welcome.svelte";
	import Signup from "./components/popup/Signup.svelte";
	import { getUser } from "./apis/user.api";
	import AccessListModal from "./components/dashboard/credentials/AccessListModal.svelte";
	import MoreActionsGroup from "./components/dashboard/components/MoreActionsGroup.svelte";
	import GroupDeleteModal from "./components/dashboard/groups/GroupDeleteModal.svelte";
	import ShareToast from "./components/dashboard/components/ShareToast.svelte";
	import { setFolderStore } from "./store/storeHelper";
	import { selectedFolder } from "./store/folder.store";
	import PasswordPromptModal from "./components/dashboard/components/PasswordPromptModal.svelte";
	let showWelcome = false;
	let signedUp = true;
	onMount(async () => {
		const response = await sendMessage("isSignedUp");
		const checkPvtLoad = await sendMessage("checkPvtLoaded");
		signedUp = response.isSignedUp;
		if (checkPvtLoad === false) {
			showWelcome = true;
		} else {
			await setFolderStore();
		}
	});

	const handleSignedUp = () => {
		signedUp = true;
	};
	const handleAuthenticated = async () => {
		const user = await getUser();
		localStorage.setItem("user", JSON.stringify(user.data));
		await setFolderStore();
		showWelcome = false;
	};
</script>

<main
	class="
    bg-osvauld-frameblack
   w-screen h-screen text-macchiato-text text-lg overflow-hidden !font-sans"
>
	{#if !signedUp}
		<Signup on:signedUp="{handleSignedUp}" />
	{:else if showWelcome}
		<div class="overflow-hidden flex justify-center items-center w-full h-full">
			<Welcome on:authenticated="{handleAuthenticated}" />
		</div>
	{:else}
		<div class="flex h-full">
			<div
				class="w-1/5 h-full scrollbar-thin overflow-y-hidden overflow-x-hidden relative z-10"
			>
				<LeftContainer />
			</div>
			<!-- Right container -->
			<div
				class="w-4/5 h-full overflow-hidden border-l border-osvauld-iconblack"
			>
				<RightContainer />
			</div>
		</div>

		{#if $showMoreOptions}
			<MoreActions />
		{/if}
		{#if $promptPassword}
			<PasswordPromptModal />
		{/if}
		{#if $modalManager}
			{#if $DeleteConfirmationModal && $modalManager.type === "Credential"}
				<CredentialDeleteModal />
			{:else if $DeleteConfirmationModal && $modalManager.type === "Folder"}
				<FolderDeleteModal />
			{:else if $DeleteConfirmationModal && $modalManager.type === "Group"}
				<GroupDeleteModal />
			{/if}
		{/if}
		{#if $accessListSelected && $selectedFolder}
			<AccessListModal />
		{/if}
		{#if $showMoreGroupOptions}
			<MoreActionsGroup />
		{/if}
		{#if $toastStore.show}
			<!-- {#if true} -->
			<div class="z-100">
				<ShareToast />
			</div>
		{/if}
	{/if}
</main>
