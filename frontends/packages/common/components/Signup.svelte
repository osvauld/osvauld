<script lang="ts">
	import SetPassPhrase from "./SetPassPhrase.svelte";
	import { createEventDispatcher } from "svelte";

	export let ImportComponent; // Allow platform-specific import component to be passed

	let importPvtKeyFlag = false;
	let showSelection = true;

	const dispatch = createEventDispatcher();

	const handleSignedUp = () => {
		dispatch("signedUp");
	};

	const handleSelection = (isImport: boolean) => {
		importPvtKeyFlag = isImport;
		showSelection = false;
	};
</script>

<div
	class="h-full w-full flex justify-center items-center text-base text-mobile-textPrimary">
	{#if showSelection}
		<div class="w-full h-[112px] rounded-xl bg-mobile-bgSeconary p-4 mx-4">
			<h2 class="text-mobile-textActive font-normal text-xl leading-6 mb-4">
				Welcome to Osvauld
			</h2>
			<div class="flex gap-3">
				<button
					class="flex-1 px-10 py-2.5 bg-osvauld-carolinablue text-mobile-bgPrimary rounded-lg font-medium whitespace-nowrap"
					on:click="{() => handleSelection(false)}">
					Sign Up
				</button>
				<button
					class="flex-1 px-10 py-2.5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg font-medium whitespace-nowrap"
					on:click="{() => handleSelection(true)}">
					Import Key
				</button>
			</div>
		</div>
	{:else if importPvtKeyFlag}
		<svelte:component this="{ImportComponent}" on:login="{handleSignedUp}" />
	{:else}
		<SetPassPhrase on:signedUp="{handleSignedUp}" />
	{/if}
</div>
