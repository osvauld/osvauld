<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import WebsiteBuilder from "./lib/WebsiteBuilder.svelte";
	import ModeSwitcher from "./components/ModeSwitcher.svelte";
	import ViewerMode from "./components/ViewerMode.svelte";
	import { dataState } from "./store.svelte";
	import { dataState as authDataState } from "./state/data.svelte";
	import { uiState } from "./state/ui.svelte";
	import Signup from "./common/Signup.svelte";
	import Welcome from "./common/Welcome.svelte";
	import Loader from "./common/Loader.svelte";
	import { sendMessage } from "./utils/helper";

	let signedUp = $state(false);
	let isLoading = $state(true);
	let showWelcome = $state(false);

	const handleSignedUp = async () => {
		signedUp = true;
		showWelcome = false;
		await authDataState.initializeState();
	};

	const handleAuthenticated = async () => {
		showWelcome = false;
		await authDataState.initializeState();
	};

	onMount(async () => {
		try {
			const response = await sendMessage("isSignedUp");
			const checkPvtLoad = await sendMessage("checkPvtLoaded");
			signedUp = response.isSignedUp;

			if (checkPvtLoad === false) {
				showWelcome = true;
			} else {
				await handleAuthenticated();
			}
		} catch (error) {
			console.error("Error during initialization:", error);
		} finally {
			isLoading = false;
		}
	});

	onDestroy(() => {
		// Clean up event listener
		authDataState.cleanupReactiveUpdates();
	});
</script>

<style>
	main {
		width: 100vw;
		height: 100vh;
	}

	.app-container {
		display: flex;
		flex-direction: column;
		height: 100vh;
		width: 100%;
	}

	header {
		background: #010409;
		color: #c9d1d9;
		padding: 1rem 2rem;
		border-bottom: 1px solid #292a36;
	}

	.header-content {
		display: flex;
		justify-content: space-between;
		align-items: center;
		max-width: 1400px;
		margin: 0 auto;
	}

	h1 {
		margin: 0;
		font-size: 1.75rem;
		font-weight: 600;
		color: #c9d1d9;
	}

	.actions {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.content {
		flex: 1;
		display: flex;
		overflow: hidden;
		position: relative;
	}
</style>

<main>
	{#if isLoading}
		<div class="h-screen w-screen flex items-center justify-center bg-mobile-bgPrimary">
			<Loader size={24} color="#F472B6" duration={1} />
		</div>
	{:else if !signedUp}
		<Signup onSignedUp={handleSignedUp} />
	{:else if showWelcome}
		<div class="h-screen w-screen flex items-center justify-center bg-mobile-bgPrimary overflow-hidden">
			<Welcome authenticated={handleAuthenticated} />
		</div>
	{:else}
		<div class="app-container">
			<header>
				<div class="header-content">
					<h1>{uiState.mode === 'builder' ? '🎨 Website Builder' : '👀 Website Viewer'}</h1>
					<div class="actions">
						<ModeSwitcher />
					</div>
				</div>
			</header>

			<div class="content">
				{#if uiState.mode === 'builder'}
					<WebsiteBuilder />
				{:else}
					<ViewerMode />
				{/if}
			</div>
		</div>
	{/if}
</main>
