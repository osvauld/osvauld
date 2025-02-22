<script>
	import Welcome from "@osvauld/password-manager-common/components/Welcome.svelte";
	import Signup from "@osvauld/password-manager-common/components/Signup.svelte";
	import Acceptor from "@osvauld/password-manager-common/components/Acceptor.svelte";
	import Toast from "./components/ui/Toast.svelte";
	import DefaultLayout from "./components/layout/DefaultLayout.svelte";
	import AddDeviceView from "./components/views/AddDeviceView.svelte";
	import DeleteConfirmationModal from "./components/ui/DeleteConfirmationModal.svelte";
	import {
		addCredentialModal,
		credentialEditorModal,
		addDeviceModal,
		viewCredentialModal,
		deleteConfirmationModal,
		toastStore,
		showSyncQr,
		language,
		currentVault,
		showWelcome1,
	} from "./store/desktop.ui.store";
	import DesktopImportPvtKey from "./components/lib/DesktopImportPvtKey.svelte";
	import { sendMessage } from "@osvauld/password-manager-common";
	import { onMount } from "svelte";
	import DocumentEditor from "./components/lib/DocumentEditor.svelte";

	import Loader from "@osvauld/password-manager-common/components/Loader.svelte";
	let signedUp = false;
	let isLoading = true;
	let showWelcome = false;
	function handleChange(event) {
		const { getContent } = event.detail;
		console.log("Content updated:", getContent());
	}

	const handleSignedUp = () => {
		signedUp = true;
		showWelcome1.set(false);
	};

	const handleAuthenticated = async () => {
		showWelcome = false;
	};

	onMount(async () => {
		try {
			const response = await sendMessage("isSignedUp");
			const checkPvtLoad = await sendMessage("checkPvtLoaded");
			signedUp = response.isSignedUp;
			if (checkPvtLoad === false) {
				showWelcome = true;
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
			<Loader size="{24}" color="#1F242A" duration="{1}" />
		</div>
	{:else if !signedUp}
		<Signup
			ImportComponent="{DesktopImportPvtKey}"
			on:signedUp="{handleSignedUp}" />
	{:else if showWelcome}
		<div class="overflow-hidden flex justify-center items-center w-full h-full">
			<Welcome on:authenticated="{handleAuthenticated}" />
		</div>
	{:else}
		<!-- <DocumentEditor /> -->
		<DefaultLayout />
		<!-- 
		{#if $deleteConfirmationModal.show}
			<DeleteConfirmationModal />
		{/if}

		{#if $addDeviceModal}
			<AddDeviceView />
		{/if}

		{#if $showSyncQr}
			<Acceptor />
		{/if}

	
		
		-->
		{#if $toastStore.show}
			<div class="z-100">
				<Toast />
			</div>
		{/if}
	{/if}
</main>
