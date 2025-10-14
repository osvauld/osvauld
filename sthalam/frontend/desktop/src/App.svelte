<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import WebsiteBuilder from "./lib/WebsiteBuilder.svelte";
	import NoticeBoardBuilder from "./lib/NoticeBoardBuilder.svelte";
	import ViewerMode from "./components/ViewerMode.svelte";
	import { dataState as authDataState } from "./state/data.svelte";
	import { uiState } from "./state/ui.svelte";
	import Signup from "./common/Signup.svelte";
	import Welcome from "./common/Welcome.svelte";
	import Loader from "./common/Loader.svelte";
	import { sendMessage } from "./utils/helper";

	// Get current resource to determine which builder to show
	const currentResource = $derived(
		authDataState.currentResourceData ||
		(authDataState.currentResourceId
			? authDataState.resources.find(r => r.id === authDataState.currentResourceId)
			: null)
	);

	// Debug: Log current resource and type
	$effect(() => {
		console.log("🎨🎨🎨 App.svelte - currentResource EFFECT FIRED:", {
			hasResource: !!currentResource,
			resourceId: currentResource?.id,
			resource_type_field: currentResource?.resource_type,
			resourceType_field: currentResource?.resourceType,
			derivedResourceType: currentResource?.resource_type || currentResource?.resourceType,
			fullCurrentResource: currentResource,
			currentResourceData: authDataState.currentResourceData,
			currentResourceId: authDataState.currentResourceId
		});
	});

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
			<div class="content">
				{#if uiState.mode === 'builder'}
					{#if !currentResource}
						<!-- No resource selected -->
						{@const _ = console.log("🔴 ROUTING: No resource selected, showing WebsiteBuilder")}
						<WebsiteBuilder />
					{:else}
						{@const resourceType = currentResource.resource_type || currentResource.resourceType}
						{@const __ = console.log("🟢 ROUTING: Resource exists, type =", resourceType, "| resource_type =", currentResource.resource_type, "| resourceType =", currentResource.resourceType)}
						{#if resourceType === 'noticeboard'}
							<!-- Thread Builder -->
							{@const ___ = console.log("✅ ROUTING: Showing NoticeBoardBuilder (Thread)")}
							<NoticeBoardBuilder />
						{:else}
							<!-- Both 'website' and 'form' use WebsiteBuilder -->
							{@const ____ = console.log("⚠️ ROUTING: Showing WebsiteBuilder for type:", resourceType)}
							<WebsiteBuilder />
						{/if}
					{/if}
				{:else}
					<ViewerMode />
				{/if}
			</div>
		</div>
	{/if}
</main>
