<script lang="ts">
	// @ts-ignore: Image import for Svelte, handled by bundler
	import LivnoteLogo from "../../../../assets/Livnote_logo.png";
	import BaseImportPvtKey from "./BaseImportPvtKey.svelte";
	import NewPassword from './NewPassword.svelte';
	import CollectUsername from './CollectUsername.svelte';
	import ProvidePrivateKey from './ProvidePrivateKey.svelte';
	import FlowContainer from './FlowContainer.svelte';
	import InitiationScreen from './InitiationScreen.svelte';

	let { onSignedUp } = $props();

	// Define view states
	const VIEW_STATES = {
		EXISITING_USER: {	WELCOME: 'welcome', IMPORT: 'import', SET_PASSPHRASE: 'existing_passphrase'},
		NEW_USER: { WELCOME: 'welcome', COLLECT_USERNAME: "username", SET_PASSPHRASE: 'new_passphrase',	PROVIDE_PRIVATE_KEY: "privateKey" }
	} as const;

	type ViewState = typeof VIEW_STATES.EXISITING_USER[keyof typeof VIEW_STATES.EXISITING_USER] | typeof VIEW_STATES.NEW_USER[keyof typeof VIEW_STATES.NEW_USER];

	let currentView = $state<ViewState>('welcome');
	let viewHistory = $state<ViewState[]>([]);
	let userFlow = $state<'EXISITING_USER' | 'NEW_USER' | null>(null);

	let collectedRecoveryString = $state("")
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
		// Set the user flow based on selection
		userFlow = isImport ? 'EXISITING_USER' : 'NEW_USER';
		
		// Navigate to the appropriate first step based on flow
		if (isImport) {
			navigateTo(VIEW_STATES.EXISITING_USER.IMPORT);
		} else {
			navigateTo(VIEW_STATES.NEW_USER.COLLECT_USERNAME);
		}
	};

	const handleImportProceed = (recoveryData: string) => {
		collectedRecoveryString = recoveryData;
		navigateTo(VIEW_STATES.EXISITING_USER.SET_PASSPHRASE);
	};

	const handleRecoveryFlowComplete = (isLoggedin: boolean) => {
		if(isLoggedin){
			console.log("Signed up and logged in, navigating to home");
			// TODO: Navigate to home
			// onSignedUp?.();
		}
	};

	const handleUsernameCollected = (username: string) => {
		collectedUsernameString = username;
		navigateTo(VIEW_STATES.NEW_USER.SET_PASSPHRASE);
	};

	const handlePassphraseSet = (isLoggedin: boolean) => {
		if (isLoggedin) {
			console.log("Passphrase set, navigating to private key step");
			navigateTo(VIEW_STATES.NEW_USER.PROVIDE_PRIVATE_KEY);
		}
	};

</script>

<div
	class="h-full w-full flex justify-center items-center text-base text-mobile-textPrimary bg-mobile-bgPrimary ring-offset-mobile-textActive px-32 relative">
	<div class="h-full flex flex-col items-center pt-[13.5rem]"
	>
	 <img src={LivnoteLogo} alt="Livnote Logo" class="mb-10" />
	{#if currentView === 'welcome'}
		<InitiationScreen onFlowSelect={triggerOnboardingFlow} />
  {:else if currentView === VIEW_STATES.EXISITING_USER.IMPORT}
		<FlowContainer onBack={goBack}>
			<BaseImportPvtKey onProceed={handleImportProceed} />
		</FlowContainer>
  {:else if currentView === VIEW_STATES.EXISITING_USER.SET_PASSPHRASE}
		<FlowContainer onBack={goBack}>
			<NewPassword onLogin={handleRecoveryFlowComplete} recoveryData={collectedRecoveryString} />
		</FlowContainer>
	{:else if currentView === VIEW_STATES.NEW_USER.COLLECT_USERNAME}
		<FlowContainer onBack={goBack}>
			<CollectUsername onProceed={handleUsernameCollected} />
		</FlowContainer>
	{:else if currentView === VIEW_STATES.NEW_USER.SET_PASSPHRASE}
		<FlowContainer onBack={goBack}>
			<NewPassword onLogin={handlePassphraseSet} recoveryData={collectedRecoveryString} />
		</FlowContainer>
    {:else if currentView === VIEW_STATES.NEW_USER.PROVIDE_PRIVATE_KEY}
		<FlowContainer onBack={goBack}>
			<ProvidePrivateKey />
		</FlowContainer>
  {/if}
	</div>
</div>
