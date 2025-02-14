<script lang="ts">
	import Initiator from "../views/Initiator.svelte";
	import Acceptor from "@osvauld/password-manager-common/components/Acceptor.svelte";
	import MobileLeftArrow from "@osvauld/password-manager-common/icons/mobileLeftArrow.svelte";
	import { currentLayout } from "../../store/mobile.ui.store";

	let isInitiator = false;

	const toggleMode = () => {
		isInitiator = !isInitiator;
	};

	const goBack = () => {
		currentLayout.set("home");
	};
</script>

<main
	class="w-screen h-screen bg-mobile-bgPrimary flex flex-col relative pb-[60px] overflow-hidden">
	<!-- Tab Navigation -->
	<nav class="w-full h-[48px] px-3 flex items-center gap-2 flex-shrink-0">
		<button on:click={goBack} class="p-2.5 rounded-lg bg-mobile-bgSeconary">
			<MobileLeftArrow />
		</button>
	</nav>
	<div class="w-full h-[48px] flex justify-between items-center px-4">
		<div class="w-full flex border-b border-mobile-bgHighlight">
			<button
				class="flex-1 h-[48px] text-center {!isInitiator
					? 'text-mobile-textActive border-b-2 border-osvauld-carolinablue'
					: 'text-mobile-textSecondary'}"
				on:click={toggleMode}>
				Receive
			</button>
			<button
				class="flex-1 h-[48px] text-center {isInitiator
					? 'text-mobile-textActive border-b-2 border-osvauld-carolinablue'
					: 'text-mobile-textSecondary'}"
				on:click={toggleMode}>
				Send
			</button>
		</div>
	</div>

	<!-- Content Area -->
	<div class="grow overflow-y-auto">
		{#if isInitiator}
			<Initiator />
		{:else}
			<Acceptor />
		{/if}
	</div>
</main>
