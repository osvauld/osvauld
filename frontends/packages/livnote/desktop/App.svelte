<script lang="ts">
	import {
		Welcome,
		Signup,
		Loader,
		sendMessage,
	} from "@osvauld/password-manager-common";
	import NotesListView from "./components/notes/NotesListView.svelte";
	import NotesWorkspace from "./components/layout/NotesWorkspace.svelte";
	import HeaderSection from "./components/layout/HeaderSection.svelte";
	import { onMount, onDestroy } from "svelte";
	import AppModals from "./components/modals/Modals.svelte";
	import { dataState, uiState } from "./state/";
	import { listen } from "@tauri-apps/api/event";
	import { mergeDocuments } from "./components/notes/documentUtils";

	let signedUp = $state(false);
	let isLoading = $state(true);
	let unsubscribeResourceUpdate = $state<Function | null>(null);

	// Handle escape key to close modals
	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Escape") {
			uiState.closeAllModals();
		}
	}

	const handleSignedUp = async () => {
		signedUp = true;
		uiState.setWelcomeScreen(false);
		await dataState.initializeState();
	};

	const handleAuthenticated = async () => {
		uiState.setWelcomeScreen(false);
		await dataState.initializeState();
	};

	onMount(async () => {
		try {
			const response = await sendMessage("isSignedUp");
			const checkPvtLoad = await sendMessage("checkPvtLoaded");
			signedUp = response.isSignedUp;
			console.log(checkPvtLoad);

			if (checkPvtLoad === false) {
				uiState.setWelcomeScreen(true);
			} else {
				await handleAuthenticated();
				// Set up merge update listener
				unsubscribeResourceUpdate = await listen(
					"merge-update",
					async (event) => {
						// Handle merge updates
						const payload = event.payload;
						let mergedDocument = mergeDocuments(
							payload.local_resource,
							payload.remote_resource,
						);

						// This will be handled in the appropriate component
						document.dispatchEvent(
							new CustomEvent("merge-complete", {
								detail: {
									mergedDocument,
									deviceId: payload.device_id,
									userId: payload.user_id,
									vectorClock: payload.vector_clock,
									resourceId: mergedDocument.resource_id,
								},
							}),
						);
					},
				);
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

		if (unsubscribeResourceUpdate) {
			unsubscribeResourceUpdate();
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
	class="bg-osvauld-frameblack w-screen h-screen text-macchiato-text text-lg !font-sans">
	{#if isLoading}
		<div class="flex justify-center items-center w-full h-full">
			<Loader size={24} color="#1F242A" duration={1} />
		</div>
	{:else if !signedUp}
		<Signup onSignedUp={handleSignedUp} />
	{:else if uiState.showWelcome}
		<div class="overflow-hidden flex justify-center items-center w-full h-full">
			<Welcome authenticated={handleAuthenticated} />
		</div>
	{:else}
		<div
			class="w-full h-full bg-osvauld-ninjablack flex flex-col overflow-hidden">
			<HeaderSection />
			<!-- App modals right after the header section -->
			<AppModals />

			<div class="grow flex overflow-hidden">
				{#if uiState.noteViewLayout}
					<!-- Note editing mode: Show NotesWorkspace with its own Navigation panel -->
					<NotesWorkspace />
				{:else}
					<NotesListView />
				{/if}
			</div>
		</div>
	{/if}
</main>
