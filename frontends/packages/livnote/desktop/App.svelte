<script lang="ts">
	import {
		Welcome,
		Signup,
		Loader,
		sendMessage,
	} from "@osvauld/password-manager-common";
	import DefaultLayout from "./components/layout/DefaultLayout.svelte";
	import DesktopImportPvtKey from "./components/connection/DesktopImportPvtKey.svelte";
	import { onMount, onDestroy } from "svelte";
	import AppModals from "./components/modals/Modals.svelte";
	import { dataState, uiState } from "./state/";

	let signedUp = $state(false);
	let isLoading = $state(true);

	// Handle escape key to close modals
	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Escape") {
			uiState.closeAllModals();
		}
	}

	const handleSignedUp = async () => {
		signedUp = true;
		uiState.setWelcomeScreen(false);
		// const userId = await sendMessage("getUserId");
	};

	const handleAuthenticated = async () => {
		uiState.setWelcomeScreen(false);
	};

	onMount(async () => {
		try {
			const response = await sendMessage("isSignedUp");
			const checkPvtLoad = await sendMessage("checkPvtLoaded");
			signedUp = response.isSignedUp;

			if (checkPvtLoad === false) {
				uiState.setWelcomeScreen(true);
			} else {
				await dataState.initializeState();
			}

			// Add global event listener for escape key
			window.addEventListener("keydown", handleKeydown);
		} catch (error) {
			console.error("Error during initialization:", error);
		} finally {
			isLoading = false;
		}
	});

	onDestroy(() => {
		// Clean up event listener
		window.removeEventListener("keydown", handleKeydown);
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
	class="bg-osvauld-frameblack w-screen h-screen text-macchiato-text text-lg !font-sans">
	{#if isLoading}
		<div class="flex justify-center items-center w-full h-full">
			<Loader size={24} color="#1F242A" duration={1} />
		</div>
	{:else if !signedUp}
		<Signup ImportComponent={DesktopImportPvtKey} onSignedUp={handleSignedUp} />
	{:else if uiState.showWelcome}
		<div class="overflow-hidden flex justify-center items-center w-full h-full">
			<Welcome authenticated={handleAuthenticated} />
		</div>
	{:else}
		<DefaultLayout />
		<AppModals />
	{/if}
</main>
