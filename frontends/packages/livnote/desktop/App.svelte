<script lang="ts">
	import DocumentEditor from "./lib/DocumentEditor.svelte";
	import Welcome from "@osvauld/password-manager-common/components/Welcome.svelte";
	import Signup from "@osvauld/password-manager-common/components/Signup.svelte";
	import DesktopImportPvtKey from "./lib/DesktopImportPvtKey.svelte";
	import { sendMessage } from "@osvauld/password-manager-common";
	import Connector from "./lib/Connector.svelte";
	import { onMount } from "svelte";
	import Initiator from "./lib/Initiator.svelte";
	import RichTextEditor from "./lib/RichTextEditor.svelte";
	import { wsConnector } from "../desktop/lib/store/wsConnectorStore";
	import type { WSConnection } from "../desktop/lib/utils/wsConnector";
	import Loader from "@osvauld/password-manager-common/components/Loader.svelte";
	import { listen, emit } from "@tauri-apps/api/event";
	import { error } from "console";
	let signedUp = false;
	let isLoading = true;
	let showWelcome = false;
	async function handleChange(event) {
		const { getContent } = event.detail;
		const data = getContent();
		await emit("sync-update", data).catch((error) => {
			console.log("errror");
		});
	}

	let wsConnectorInstance: WSConnection;

	wsConnectorInstance = $wsConnector;

	let showConnector = true;
	const handleSignedUp = () => {
		signedUp = true;
		showWelcome = false;
	};

	const handleAuthenticated = async () => {
		showWelcome = false;
		// Registering connection to WS Rendezvous Server
		sendMessage("getUserId")
			.then((userId: string) => {
				wsConnectorInstance.sendRegisterMessage(userId);
			})
			.catch(() => {
				console.error("UserId generation failed");
			});
	};

	let syncRole = ""; // Add this to store the role

	const handleConnectorClose = (event) => {
		const { isInitiator } = event.detail;
		syncRole = isInitiator ? "initiator" : "acceptor";
		showConnector = false;
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
		{console.log("showing loader")}
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
	{:else if showConnector}
		<Connector on:close={handleConnectorClose} />
	{:else}
		<RichTextEditor on:change={handleChange} />
	{/if}
</main>
