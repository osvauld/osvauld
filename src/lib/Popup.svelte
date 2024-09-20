<script lang="ts">
	import { onMount } from "svelte";
	import Welcome from "./components/popup/Welcome.svelte";
	import Home from "./components/popup/Home.svelte";
	import { Logo } from "./components/dashboard/icons";
	import Signup from "./components/popup/Signup.svelte";
	import Loader from "./components/dashboard/components/Loader.svelte";
	import { sendMessage } from "./components/dashboard/helper";

	let loggedIn = true;
	let isLoaderActive = false;
	let isSignedUp = false;
	onMount(async () => {
		isLoaderActive = true;
		const response = await sendMessage("isSignedUp");
		isSignedUp = response.isSignedUp;
		const checkPvtLoad = await sendMessage("checkPvtLoaded");
		if (checkPvtLoad) {
			loggedIn = true;
		} else {
			loggedIn = false;
		}
		isLoaderActive = false;
		// isSignedUp.set(false);
	});

	const openFullscreenTab = async () => {
		// Send a message to the background
		await sendMessage("openFullscreenTab");
	};

	const checkAuth = (event: CustomEvent) => {
		loggedIn = event.detail;
	};
</script>

<main>
	<div
		class="w-[22.5rem] h-[36.78rem] p-2 pt-3 flex flex-col !font-sans {isSignedUp &&
		!loggedIn
			? 'justify-center'
			: 'justify-start'} items-center bg-osvauld-frameblack"
	>
		{#if isLoaderActive}
			<Loader />
		{:else if !isSignedUp}
			<Signup on:signedUp="{() => (isSignedUp = true)}" />
		{:else if loggedIn}
			<Home />
		{:else}
			<div class="mb-12 flex justify-center items-center">
				<Logo />
			</div>
			<Welcome on:authenticated="{checkAuth}" />
		{/if}
	</div>
</main>
