<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { onMount } from "svelte";
	import { sendMessage } from "@osvauld/password-manager-common";

	import Loader from "@osvauld/password-manager-common/components/Loader.svelte";
	import Welcome from "@osvauld/password-manager-common/components/Welcome.svelte";
	import Signup from "@osvauld/password-manager-common/components/Signup.svelte";

	import { loadLocaleAsync } from "@osvauld/password-manager-common/i18n/i18n-util.async";
	import { setLocale } from "@osvauld/password-manager-common/i18n/i18n-svelte";

	import Toast from "./components/ui/Toast.svelte";
	import DefaultLayout from "./components/layout/DefaultLayout.svelte";
	import AddCredentialModal from "./components/views/AddCredentialModal.svelte";
	import AddDeviceView from "./components/views/AddDeviceView.svelte";
	import CredentialEditorModal from "./components/views/CredentialEditorModal.svelte";
	import CredentialViewModal from "./components/views/CredentialViewModal.svelte";
	import DeleteConfirmationModal from "./components/ui/DeleteConfirmationModal.svelte";
	import { SUPPORTED_LANGUAGES } from "@osvauld/password-manager-common/utils/translationUtils";
	import {
		addCredentialModal,
		credentialEditorModal,
		addDeviceModal,
		viewCredentialModal,
		deleteConfirmationModal,
		toastStore,
	} from "./store/desktop.ui.store";

	import { setFolderStore } from "@osvauld/password-manager-common/utils/storeHelper";
	import DesktopImportPvtKey from "./components/ui/DesktopImportPvtKey.svelte";

	let showWelcome = false;
	let signedUp = false;
	let isLoading = true;

	async function initializeLanguage() {
		try {
			const locale = await invoke("get_system_locale");
			const deviceLanguage = String(locale).split(/[-_]/)[0].toLowerCase();
			const languageToUse = SUPPORTED_LANGUAGES.includes(deviceLanguage)
				? deviceLanguage
				: "en";
			// const languageToUse = "fr";
			await loadLocaleAsync(languageToUse);
			setLocale(languageToUse);
		} catch (error) {
			console.error("Error detecting system language:", error);
			await loadLocaleAsync("en");
			setLocale("en");
		}
	}

	const handleSignedUp = () => {
		signedUp = true;
		showWelcome = false;
	};

	const handleAuthenticated = async () => {
		await setFolderStore();
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
				await setFolderStore();
			}

			initializeLanguage();
		} catch (error) {
			console.error("Error during initialization:", error);
		} finally {
			isLoading = false;
		}
	});
</script>

<style>
	:root {
		overflow: hidden;
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
		<Signup
			ImportComponent={DesktopImportPvtKey}
			on:signedUp={handleSignedUp} />
	{:else if showWelcome}
		<div class="overflow-hidden flex justify-center items-center w-full h-full">
			<Welcome on:authenticated={handleAuthenticated} />
		</div>
	{:else}
		<DefaultLayout />

		<!-- AddCredentialModal opens up folder or/and category type selection modal -->
		{#if $addCredentialModal}
			<AddCredentialModal />
		{/if}

		<!-- Actual credential entering happens here in CredentialEditorModal-->
		{#if $credentialEditorModal}
			<CredentialEditorModal />
		{/if}

		{#if $viewCredentialModal}
			<CredentialViewModal />
		{/if}

		{#if $deleteConfirmationModal.show}
			<DeleteConfirmationModal />
		{/if}

		{#if $addDeviceModal}
			<AddDeviceView />
		{/if}

		<!-- {#if $showMoreOptions}
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
			{/if}
		{/if}

	
		-->
		{#if $toastStore.show}
			<div class="z-100">
				<Toast />
			</div>
		{/if}
	{/if}
</main>
