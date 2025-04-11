<script lang="ts">
	import SetPassPhrase from "./SetPassPhrase.svelte";

	// Using $props for component props
	let { ImportComponent, onSignedUp } = $props();

	// Using $state for reactive variables
	let importPvtKeyFlag = $state(false);
	let showSelection = $state(true);

	const handleSignedUp = () => {
		onSignedUp?.();
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
					onclick={() => handleSelection(false)}>
					Sign Up
				</button>
				<button
					class="flex-1 px-10 py-2.5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg font-medium whitespace-nowrap"
					onclick={() => handleSelection(true)}>
					Import Key
				</button>
			</div>
		</div>
	{:else if importPvtKeyFlag}
		<ImportComponent onLogin={handleSignedUp} />
	{:else}
		<SetPassPhrase onSignedUp={handleSignedUp} />
	{/if}
</div>
