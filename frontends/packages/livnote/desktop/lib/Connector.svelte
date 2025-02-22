<script lang="ts">
	import { onMount } from "svelte";
	import Initiator from "./Initiator.svelte";
	import { sendMessage } from "@osvauld/password-manager-common";
	import Acceptor from "@osvauld/password-manager-common/components/Acceptor.svelte";
	import { createEventDispatcher } from "svelte";

	const dispatch = createEventDispatcher();

	function handleClose() {
		// Dispatch a "close" event to the parent component
		dispatch("close");
	}
	let isInitiator = true;

	function toggleView() {
		isInitiator = !isInitiator;
	}
</script>

<div
	class="p-4 inset-0 items-center justify-center z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[2px] fixed flex flex-col gap-4">
	<!-- Place button here, outside any potentially covering container -->

	<!-- Main container -->
	<div class="grow overflow-y-auto relative">
		{#if isInitiator}
			<Initiator bind:isInitiator />
		{:else}
			<Acceptor />
		{/if}
	</div>
	<div class="flex justify-between">
		<button
			class="bg-blue-500 text-white px-4 py-2 rounded m-2 z-50"
			on:click={toggleView}>
			{isInitiator ? "Switch to Acceptor" : "Switch to Initiator"}
		</button>
		<button
			class="bg-mobile-bgHighlight text-white px-4 py-2 rounded m-2 z-50"
			on:click={handleClose}>
			Close
		</button>
	</div>
</div>
