<script lang="ts">
	import { onMount } from "svelte";
	import Initiator from "./Initiator.svelte";
	import Acceptor from "@osvauld/password-manager-common/components/Acceptor.svelte";
	import { sendMessage } from "@osvauld/password-manager-common";

	let isInitiator = true;

	function toggleView() {
		isInitiator = !isInitiator;
	}
</script>

<!-- Place button here, outside any potentially covering container -->
<button
	class="bg-blue-500 text-white px-4 py-2 rounded m-2 z-50"
	on:click="{toggleView}">
	{isInitiator ? "Switch to Acceptor" : "Switch to Initiator"}
</button>

<!-- Main container -->
<div class="grow overflow-y-auto relative">
	{#if isInitiator}
		<Initiator bind:isInitiator />
	{:else}
		<Acceptor />
	{/if}
</div>
