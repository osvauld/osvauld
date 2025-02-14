<script lang="ts">
	import SetPassPhrase from "./SetPassPhrase.svelte";
	import ImportPvtKey from "./ImportPvtKey.svelte";
	import { createEventDispatcher } from "svelte";

	let challenge = "";
	let importPvtKeyFlag = false;
	let showSelection = true; // Controls visibility of the selection screen

	const dispatch = createEventDispatcher();

	const handleRecovery = (e: any) => {
		importPvtKeyFlag = e.detail;
	};

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
		<!-- Selection Screen -->
		<div class="w-full h-[112px] rounded-xl bg-mobile-bgSeconary p-4 mx-4">
			<h2 class="text-mobile-textActive font-normal text-xl leading-6 mb-4">
				Welcome to Osvauld
			</h2>
			<div class="flex gap-3">
				<button
					class="flex-1 px-10 py-2.5 bg-osvauld-carolinablue text-mobile-bgPrimary rounded-lg font-medium"
					on:click="{() => handleSelection(false)}">
					Sign Up
				</button>
				<button
					class="flex-1 px-10 py-2.5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg font-medium"
					on:click="{() => handleSelection(true)}">
					Import Key
				</button>
			</div>
		</div>
	{:else}
		<!-- Existing Flow -->
		{#if importPvtKeyFlag}
			<ImportPvtKey on:login="{handleSignedUp}" />
		{:else}
			<SetPassPhrase {challenge} on:signedUp="{handleSignedUp}" />
		{/if}
	{/if}
</div>
