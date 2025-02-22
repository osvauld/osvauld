<script lang="ts">
	import { onMount } from "svelte";
	import Initiator from "./Initiator.svelte";
	import { sendMessage } from "@osvauld/password-manager-common";
	import Acceptor from "@osvauld/password-manager-common/components/Acceptor.svelte";
	import { createEventDispatcher } from "svelte";

	const dispatch = createEventDispatcher();

	function handleClose() {
		// Dispatch a "close" event to the parent component
		dispatch("close", { isInitiator });
	}
	let isInitiator = true;

	function toggleView() {
		isInitiator = !isInitiator;
	}
</script>

<!-- Place button here, outside any potentially covering container -->
<button
	class="bg-blue-500 text-white px-4 py-2 rounded m-2 z-50"
	on:click={toggleView}>
	{isInitiator ? "Switch to Acceptor" : "Switch to Initiator"}
</button>

<!-- Main container -->
<div class="grow overflow-y-auto relative">
	{#if isInitiator}
		<Initiator bind:isInitiator />
	{:else}
		<Acceptor />
	{/if}
	<button class="bg-blue-500" on:click={handleClose}> close </button>
</div>
