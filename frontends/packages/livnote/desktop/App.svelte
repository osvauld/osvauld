<script lang="ts">
	import Welcome from "@osvauld/password-manager-common/components/Welcome.svelte";
	import Signup from "@osvauld/password-manager-common/components/Signup.svelte";
	import Toast from "./components/ui/Toast.svelte";
	import DeleteConfirmationModal from "./components/ui/DeleteConfirmationModal.svelte";
	import DefaultLayout from "./components/layout/DefaultLayout.svelte";
	import Connector from "./components/connection/Connector.svelte";
	import DesktopImportPvtKey from "./components/connection/DesktopImportPvtKey.svelte";
	import Loader from "@osvauld/password-manager-common/components/Loader.svelte";
	import AddUserModal from "./components/modals/AddUserModal.svelte";
	import PasswordPromptModal from "@osvauld/password-manager-common/components/PasswordPromptModal.svelte";

	import { sendMessage } from "@osvauld/password-manager-common";
	import { onMount } from "svelte";

	import {
		toastStore,
		showWelcome,
		showConnector,
		showAddUser,
		deleteConfirmationModal,
		passwordPromptModal,
	} from "./store/desktop.ui.store";

	let signedUp = $state(false);
	let isLoading = $state(true);
	let syncRole = ""; // Add this to store the role

	const handleSignedUp = async () => {
		signedUp = true;
		showWelcome.set(false);
		// const userId = await sendMessage("getUserId");
	};

	const handleAddUser = async (event) => {
		const userResponse = await sendMessage("addKnownUser", event.detail);
		console.log("initiating first connection");
		const firstConnectionResponse = await sendMessage(
			"initiateFirstConnection",
			{
				user: userResponse.user,
				device: userResponse.device,
			},
		);
		console.log(firstConnectionResponse);
	};

	const handleAuthenticated = async () => {
		showWelcome.set(false);
		//const userId = await sendMessage("getUserId");
	};

	const handleConnectorClose = (event) => {
		const { isInitiator } = event.detail;
		syncRole = isInitiator ? "initiator" : "acceptor";
		showConnector.set(false);
	};

	const handlePasswordModalClose = (event) => {
		passwordPromptModal.set({ isChangePassword: false, show: !event.detail });
	};

	onMount(async () => {
		try {
			const response = await sendMessage("isSignedUp");
			console.log("is signedup response", response);
			const checkPvtLoad = await sendMessage("checkPvtLoaded");
			signedUp = response.isSignedUp;
			if (checkPvtLoad === false) {
				showWelcome.set(true);
			} else {
				// await vaultInitlization();
			}
		} catch (error) {
			console.error("Error during initialization:", error);
		} finally {
			isLoading = false;
		}
	});
</script>

<style>
	:root {
		box-sizing: border-box;
		margin: 0;
		padding: 0;
	}
</style>

<main
	class="
    bg-osvauld-frameblack
   w-screen h-screen text-macchiato-text text-lg !font-sans">
	{#if isLoading}
		<div class="flex justify-center items-center w-full h-full">
			<Loader size={24} color="#1F242A" duration={1} />
		</div>
	{:else if !signedUp}
		<Signup ImportComponent={DesktopImportPvtKey} onSignedUp={handleSignedUp} />
	{:else if $showWelcome}
		<div class="overflow-hidden flex justify-center items-center w-full h-full">
			<Welcome on:authenticated={handleAuthenticated} />
		</div>
	{:else}
		<DefaultLayout />

		{#if $deleteConfirmationModal.show}
			<DeleteConfirmationModal />
		{/if}

		{#if $passwordPromptModal.show}
			<PasswordPromptModal
				changePassword={$passwordPromptModal.isChangePassword}
				onClose={handlePasswordModalClose} />
		{/if}

		{#if $showAddUser}
			<AddUserModal
				userAdd={handleAddUser}
				close={() => {
					showAddUser.set(false);
				}} />
		{/if}

		{#if $showConnector}
			<Connector onClose={handleConnectorClose} />
		{/if}

		{#if $toastStore.show}
			<div class="z-100">
				<Toast />
			</div>
		{/if}
	{/if}
</main>
