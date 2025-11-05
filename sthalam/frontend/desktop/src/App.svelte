<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import BuilderApp from "./builder/BuilderApp.svelte";
	import ViewerApp from "./viewer/ViewerApp.svelte";
	import PublisherApp from "./publisher/PublisherApp.svelte";
	import { dataState as authDataState } from "./state/data.svelte";
	import { uiState } from "./state/ui.svelte";
	import Signup from "./common/Signup.svelte";
	import Welcome from "./common/Welcome.svelte";
	import Loader from "./common/Loader.svelte";
	import { sendMessage } from "./utils/helper";
	import { loadCELEvaluator } from "./lib/services/celEvaluator";
	import { loadHUMLParser } from "./lib/services/humlParser";

	let signedUp = $state(false);
	let isLoading = $state(true);
	let showWelcome = $state(false);

	const handleSignedUp = async () => {
		signedUp = true;
		showWelcome = false;
		// Initialize the unified dataState (handles resources AND coordinator)
		await authDataState.initializeState();
	};

	const handleAuthenticated = async () => {
		showWelcome = false;
		// Initialize the unified dataState (handles resources AND coordinator)
		await authDataState.initializeState();
	};

	onMount(async () => {
		try {
			// Load WASM modules (CEL evaluator & HUML parser)
			console.log("📦 Loading WASM modules...");
			await Promise.all([
				loadCELEvaluator(),
				loadHUMLParser()
			]);
			console.log("✅ WASM modules loaded");

			// Test Loro CRDT compatibility with Tauri + Vite + WASM
			console.log("🧪 Testing Loro CRDT...");

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

	.content {
		flex: 1;
		display: flex;
		overflow: hidden;
		position: relative;
	}

	.builder-placeholder {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		color: #8b949e;
		background: #010409;
	}

	.builder-placeholder h2 {
		font-size: 1.5rem;
		margin-bottom: 0.5rem;
		color: #c9d1d9;
	}

	.builder-placeholder p {
		font-size: 1rem;
	}
</style>

<main>
	{#if isLoading}
		<div
			class="h-screen w-screen flex items-center justify-center bg-mobile-bgPrimary"
		>
			<Loader size={24} color="#F472B6" duration={1} />
		</div>
	{:else if !signedUp}
		<Signup onSignedUp={handleSignedUp} />
	{:else if showWelcome}
		<div
			class="h-screen w-screen flex items-center justify-center bg-mobile-bgPrimary overflow-hidden"
		>
			<Welcome authenticated={handleAuthenticated} />
		</div>
	{:else}
		<div class="app-container">
			<div class="content">
				{#if uiState.mode === "builder"}
					<BuilderApp />
				{:else if uiState.mode === "publisher"}
					<PublisherApp />
				{:else if uiState.mode === "viewer"}
					<ViewerApp />
				{:else}
					<!-- Default to builder -->
					<BuilderApp />
				{/if}
			</div>
		</div>
	{/if}
</main>
