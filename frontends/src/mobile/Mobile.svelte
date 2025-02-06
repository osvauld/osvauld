<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { onMount } from "svelte";
	import Home from "./components/Home.svelte";
	import Welcome from "../lib/components/popup/Welcome.svelte";
	import { Logo } from "../lib/components/dashboard/icons";
	import Signup from "../lib/components/popup/Signup.svelte";
	import Loader from "../lib/components/dashboard/components/Loader.svelte";
	import { sendMessage } from "../lib/components/dashboard/helper";
	// call detach() if you do not want to print logs to the console anymore
	let loggedIn = true;
	let isLoaderActive = false;
	let isSignedUp = false;
	onMount(async () => {
		isLoaderActive = true;
		const response = await invoke("check_signup_status");
		isSignedUp = response.isSignedUp;
		const checkPvtLoad = await invoke("check_private_key_loaded");
		if (checkPvtLoad) {
			loggedIn = true;
		} else {
			loggedIn = false;
		}
		isLoaderActive = false;
		// isSignedUp.set(false);
	});

	const handleSignUp = async () => {
		// try {
		// 	await Promise.all([
		// 		sendMessage("addFolder", {
		// 			name: "Personal",
		// 			description: "",
		// 		}),
		// 		sendMessage("addFolder", {
		// 			name: "Work",
		// 			description: "",
		// 		}),
		// 	]);
		// } catch (e) {
		// 	console.log("error detected", e);
		// }

		isSignedUp = true;
	};

	const checkAuth = (event: CustomEvent) => {
		loggedIn = event.detail;
	};
</script>

<main
	class="w-dvw h-dvh p-2 pt-3 bg-mobile-bgPrimary flex flex-col !font-sans {isSignedUp &&
	!loggedIn
		? 'justify-center'
		: 'justify-start'} items-center">
	{#if isLoaderActive}
		<Loader />
	{:else if !isSignedUp}
		<Signup on:signedUp="{handleSignUp}" />
	{:else if loggedIn}
		<Home />
	{:else}
		<div class="mb-12 flex justify-center items-center">
			<Logo />
		</div>
		<Welcome on:authenticated="{checkAuth}" />
	{/if}
</main>
