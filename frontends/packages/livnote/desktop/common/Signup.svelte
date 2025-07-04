<script lang="ts">
	import BaseImportPvtKey from "./BaseImportPvtKey.svelte";
	import SetPassPhrase from "./SetPassPhrase.svelte";
	// @ts-ignore: Image import for Svelte, handled by bundler
	import LivnoteLogo from "../../../../assets/Livnote_logo.png";

	let { onSignedUp } = $props();

	let importPvtKeyFlag = $state(false);
	let showSelection = $state(true);

	const handleSignedUp = () => {
		console.log("triggerd handle signup");
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
	<div
	class="w-full h-full bg-mobile-bgPrimary py-16">
	<div class="h-full flex flex-col items-center justify-between pt-[9.5rem]">
		<img src={LivnoteLogo} alt="Livnote Logo" class="" />
		<h1 class=" font-extralight mb-4 font-Jakarta text-7xl text-center text-white">
			Write, Connect & Collaborate <br/> <span class="text-livnotePink">without</span> servers. 
			<br />
			<span class="text-mobile-textActive">
				Encrypted, open & free
			</span>
		</h1>
		<div class="flex gap-3">
			<button
			class="py-3.5 px-5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg font-medium whitespace-nowrap"
			onclick={() => handleSelection(true)}>
			I already have the key
		</button>
		<button
			onclick={() => handleSelection(false)} class="w-[13.75rem] py-3.5 px-5 bg-signupGray text-white rounded-md">
			I am new here
		</button>

	</div>
	<p class="font-inter text-disclaimerGray text-center text-sm">By continuing you agree to our Terms of Use and Privacy Policy</p>
</div>
</div>
	{:else if importPvtKeyFlag}
		<BaseImportPvtKey onLogin={handleSignedUp} />
	{:else}
		<SetPassPhrase onSignedUp={handleSignedUp} />
	{/if}
</div>
