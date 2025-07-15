<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { onMount } from "svelte";
	import { sendMessage } from "@osvauld/password-manager-common";

	import Loader from "@osvauld/password-manager-common/components/Loader.svelte";
	import Welcome from "@osvauld/password-manager-common/components/Welcome.svelte";
	import Signup from "@osvauld/password-manager-common/components/Signup.svelte";
	import Acceptor from "@osvauld/password-manager-common/components/Acceptor.svelte";

	import { loadLocaleAsync } from "@osvauld/password-manager-common/i18n/i18n-util.async";
	import { setLocale } from "@osvauld/password-manager-common/i18n/i18n-svelte";
	import { type Locales } from "@osvauld/password-manager-common/i18n/i18n-types.js";

	import Toast from "./components/ui/Toast.svelte";
	import DefaultLayout from "./components/layout/DefaultLayout.svelte";
	import AddCredentialModal from "./components/views/AddCredentialModal.svelte";
	import AddDeviceView from "./components/views/AddDeviceView.svelte";
	import CredentialEditorModal from "./components/views/CredentialEditorModal.svelte";
	import CredentialViewModal from "./components/views/CredentialViewModal.svelte";
	import DeleteConfirmationModal from "./components/ui/DeleteConfirmationModal.svelte";
	import { LANGUAGE_CODES } from "@osvauld/password-manager-common/utils/translationUtils";
	import {
		addCredentialModal,
		credentialEditorModal,
		addDeviceModal,
		viewCredentialModal,
		deleteConfirmationModal,
		toastStore,
		showWelcome,
		showSyncQr,
		language,
		currentVault,
	} from "./store/desktop.ui.store";

	import { StorageService } from "@osvauld/password-manager-common/utils/storageHelper";

	import DesktopImportPvtKey from "./components/ui/DesktopImportPvtKey.svelte";

	let signedUp = false;
	let isLoading = true;

	const initializeLanguage = async () => {
		try {
			const locale = await invoke("get_system_locale");
			const deviceLanguage = String(locale).split(/[-_]/)[0].toLowerCase();
			const languageToUse: Locales = LANGUAGE_CODES.includes(deviceLanguage)
				? (deviceLanguage as Locales)
				: "en";
			// const languageToUse = "it";
			language.set(languageToUse);
			await loadLocaleAsync(languageToUse);
			setLocale(languageToUse);
		} catch (error) {
			console.error("Error detecting system language:", error);
			await loadLocaleAsync("en");
			setLocale("en");
		}
	};

	const vaultInitlization = async () => {
		const currentVaultState = await StorageService.getCurrentVault();
		if (!currentVaultState) return;
		const currentVaultJSON = JSON.parse(currentVaultState);
		if (currentVaultJSON?.id) currentVault.set(currentVaultJSON);
	};

	const handleSignedUp = () => {
		signedUp = true;
		showWelcome.set(false);
	};

	const handleAuthenticated = async () => {
		await vaultInitlization();

		showWelcome.set(false);
	};

	$: if ($language) {
		(async () => {
			await loadLocaleAsync($language);
			setLocale($language);
		})();
	}

	onMount(async () => {
		try {
			const response = await sendMessage("isSignedUp");
			const checkPvtLoad = await sendMessage("checkPvtLoaded");
			signedUp = response.isSignedUp;
			if (checkPvtLoad === false) {
				showWelcome.set(true);
			} else {
				await vaultInitlization();
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
		{console.log("showing loader")}
		<div class="flex justify-center items-center w-full h-full">
			<Loader size={24} color="#1F242A" duration={1} />
		</div>
	{:else if !signedUp}
		<Signup
			ImportComponent={DesktopImportPvtKey}
			on:signedUp={handleSignedUp} />
	{:else if $showWelcome}
		<div class="overflow-hidden flex justify-center items-center w-full h-full">
			<Welcome on:authenticated={handleAuthenticated} />
		</div>
	{:else}
		{console.log("attempting to show default layout")}
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
		{#if $showSyncQr}
			<Acceptor />
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
