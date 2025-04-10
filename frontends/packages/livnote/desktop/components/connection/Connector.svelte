<script lang="ts">
	import Initiator from "./Initiator.svelte";
	import Acceptor from "@osvauld/password-manager-common/components/Acceptor.svelte";
	import Welcome from "@osvauld/password-manager-common/components/Welcome.svelte";

	let isInitiator = $state(true);
	let passwordCollected = $state("");
	let { onClose } = $props();

	function handleClose() {
		// Dispatch a "close" event to the parent component
		onClose?.({ isInitiator });
	}

	function toggleView() {
		isInitiator = !isInitiator;
	}

	function handlePasswordReturn(e: CustomEvent) {
		passwordCollected = e.detail;
	}
</script>

<div
	class="p-4 inset-0 items-center justify-center z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[2px] fixed flex flex-col gap-4">
	{#if isInitiator}
		<Initiator />
	{:else if passwordCollected}
		<Acceptor {passwordCollected} />
	{:else}
		<Welcome
			passwordReturn={true}
			on:passphraseCollected={handlePasswordReturn} />
	{/if}

	<div class="flex justify-between">
		<button
			class="bg-blue-500 text-white px-4 py-2 rounded m-2 z-50"
			onclick={toggleView}>
			{isInitiator ? "Switch to Acceptor" : "Switch to Initiator"}
		</button>
		<button
			class="bg-mobile-bgHighlight text-white px-4 py-2 rounded m-2 z-50"
			onclick={handleClose}>
			Close
		</button>
	</div>
</div>
