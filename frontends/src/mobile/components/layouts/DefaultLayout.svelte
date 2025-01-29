<script lang="ts">
	import { onMount } from "svelte";
	import { sendMessage } from "../../../lib/components/dashboard/helper";
	import TopNavBar from "../sections/TopNavBar.svelte";
	import RecentsAndCategories from "../views/RecentsAndCategories.svelte";
	import VaultManager from "../views/VaultManager.svelte";
	import EmptyVault from "../ui/EmptyVault.svelte";
	import {
		credentialListWithType,
		vaultSwitchActive,
		vaults,
		favoriteCredentials,
	} from "../../store/mobile.ui.store";
	import FilteredCredentialListComponent from "../views/FilteredCredentialListComponent.svelte";
	import VaultHeader from "../ui/VaultHeader.svelte";

	const NUM_CREDS = 1;
	// vaultSwitchActive.subscribe((value) => {
	// 	console.log(value);
	// });
</script>

<main
	class="w-screen h-screen relative bg-mobile-bgPrimary flex flex-col pb-[60px] overflow-hidden">
	{#if $credentialListWithType}
		<FilteredCredentialListComponent />
	{:else}
		<VaultHeader />
		<TopNavBar />
		<RecentsAndCategories />
	{/if}
</main>
