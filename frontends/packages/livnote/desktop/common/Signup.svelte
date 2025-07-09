<script lang="ts">
	import { slide } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
	import BaseImportPvtKey from "./BaseImportPvtKey.svelte";
	import SetPassPhrase from "./SetPassPhrase.svelte";
	// @ts-ignore: Image import for Svelte, handled by bundler
	import LivnoteLogo from "../../../../assets/Livnote_logo.png";
	import { GoBack } from "@osvauld/password-manager-common";
	import NewPassword from './NewPassword.svelte';

	let { onSignedUp } = $props();

	// Define view states
	const VIEW_STATES = {
		WELCOME: 'welcome',
		IMPORT: 'import',
		PASSPHRASE: 'passphrase'
	} as const;

	type ViewState = typeof VIEW_STATES[keyof typeof VIEW_STATES];

	let currentView = $state<ViewState>(VIEW_STATES.WELCOME);
	let viewHistory = $state<ViewState[]>([]);

	let collectedRecoveryString = "";
	let collectedUsernameString = "";

	const navigateTo = (view: ViewState) => {
		viewHistory = [...viewHistory, currentView];
		currentView = view;
	};

	const goBack = () => {
		if (viewHistory.length > 0) {
			currentView = viewHistory[viewHistory.length - 1];
			viewHistory = viewHistory.slice(0, -1);
		}
	};

	const triggerOnboardingFlow = (isImport: boolean) => {
		navigateTo(isImport ? VIEW_STATES.IMPORT : VIEW_STATES.PASSPHRASE);
	};

	const handleImportProceed = (recoveryData: string) => {
		collectedRecoveryString = recoveryData;
		navigateTo(VIEW_STATES.PASSPHRASE);
	};

	const handleRecoveryFlowComplete = (isLoggedin: boolean) => {
		if(isLoggedin){
			console.log("Signed up and logged in, navigating to home");
			// TODO: Navigate to home
			// onSignedUp?.();
		}
	};

  // Custom slide-in-from-right transition
  const slideFromRight = (node: HTMLElement, { duration = 400, easing = cubicOut } = {}) => {
    return {
      duration,
      easing,
      css: (t: number) => {
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
	class="h-full w-full flex justify-center items-center text-base text-mobile-textPrimary bg-mobile-bgPrimary ring-offset-mobile-textActive relative">
	<div class="h-full flex flex-col items-center pt-[13.5rem]"
	>
	 <img src={LivnoteLogo} alt="Livnote Logo" class="" />
  {#if currentView === VIEW_STATES.WELCOME}
	   <div class="grow flex flex-col items-center justify-around pt-16">
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
					onclick={() => triggerOnboardingFlow(true)}>
					I already have the key
				</button>
				<button
					onclick={() => triggerOnboardingFlow(false)}
					class="w-[13.75rem] py-3.5 px-5 bg-signupGray text-white rounded-md cursor-pointer transition-colors duration-300 hover:bg-livnotePink hover:text-mobile-bgPrimary">
					I am new here
				</button>
			</div>
		  <p class="font-inter text-disclaimerGray text-center text-sm ">By continuing you agree to our Terms of Use and Privacy Policy</p>
	  </div>
  {:else if currentView === VIEW_STATES.IMPORT}
	 <div class="grow flex flex-col justify-center items-center " >
			 <button class="absolute top-1/2 left-0 -translate-y-1/2 cursor-pointer border border-transparent focus:border-livnotePink outline-0 rounded-lg p-1" onclick={goBack}><GoBack/></button>
        <BaseImportPvtKey onProceed={handleImportProceed} />
    </div>
  {:else if currentView === VIEW_STATES.PASSPHRASE}
		<div class="grow flex flex-col justify-center items-center " >
			<button class="absolute top-1/2 left-0 -translate-y-1/2 cursor-pointer border border-transparent focus:border-livnotePink outline-0 rounded-lg p-1" onclick={goBack}><GoBack/></button>
			<NewPassword  onLogin={handleRecoveryFlowComplete} recoveryData={collectedRecoveryString} />
		</div>
  {/if}
</div>
</div>
