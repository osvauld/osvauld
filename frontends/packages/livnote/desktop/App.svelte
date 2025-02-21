<script>
	import RichTextEditor from "./lib/RichTextEditor.svelte";
	import Welcome from "@osvauld/password-manager-common/components/Welcome.svelte";
	import Signup from "@osvauld/password-manager-common/components/Signup.svelte";
	import Acceptor from "@osvauld/password-manager-common/components/Acceptor.svelte";
	import DesktopImportPvtKey from "./lib/DesktopImportPvtKey.svelte";
	import { sendMessage } from "@osvauld/password-manager-common";
	import { onMount } from "svelte";

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
		showWelcome.set(false);
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
	{:else}
		<RichTextEditor placeholder="Start writing..." on:change={handleChange} />
	{/if}
</main>
