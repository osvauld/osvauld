<script lang="ts">
	import { slide } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
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

	const setSignupIsImport = (isImport: boolean) => {
		importPvtKeyFlag = isImport;
		showSelection = false;
	};

  // Custom slide-in-from-right transition
  function slideFromRight(node, { duration = 400, easing = cubicOut } = {}) {
    return {
      duration,
      easing,
      css: (t) => {
        const translateX = (1 - t) * 100;
        return `
          transform: translateX(${translateX}%);
          opacity: ${t};
        `;
      }
    };
  }


</script>

<div
	class="h-full w-full flex justify-center items-center text-base text-mobile-textPrimary">
	{#if showSelection}
	<div
	class="w-full h-full bg-mobile-bgPrimary py-16">
	<div class="h-full flex flex-col items-center justify-between pt-[9.5rem]">
		<img src={LivnoteLogo} alt="Livnote Logo" class="" />
		<h1 class="font-extralight mb-4 font-Jakarta text-7xl text-center text-white">
			Write, Connect & Collaborate <br/> <span class="text-livnotePink">without</span> servers. 
			<br />
			<span class="text-mobile-textActive text-[76px]">
				Encrypted, open & free
			</span>
		</h1>
		<div class="flex gap-13 text-md font-medium">
			<button
			class="py-3.5 px-5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg whitespace-nowrap cursor-pointer hover:text-white hover:border-mobile-textActive transition-colors duration-300"
			onclick={() => setSignupIsImport(true)}>
			I already have the key
		</button>
		<button
			onclick={() => setSignupIsImport(false)}
			class="w-[13.75rem] py-3.5 px-5 bg-signupGray text-white rounded-md cursor-pointer transition-colors duration-300 hover:bg-livnotePink hover:text-mobile-bgPrimary">
			I am new here
		</button>

	</div>
	<p class="font-inter text-disclaimerGray text-center text-sm">By continuing you agree to our Terms of Use and Privacy Policy</p>
</div>
</div>
{:else if importPvtKeyFlag}
    <div in:slideFromRight={{duration: 600}} >
        <BaseImportPvtKey onLogin={handleSignedUp} />
    </div>
{:else}
    <div in:slideFromRight={{duration: 600}} >
        <SetPassPhrase onSignedUp={handleSignedUp} />
    </div>
{/if}
</div>
