<script lang="ts">
	// @ts-ignore: Image import for Svelte, handled by bundler
	import LivnoteLogo from "../../../../assets/Livnote_logo.png";
	import BaseImportPvtKey from "./BaseImportPvtKey.svelte";
	import NewPassword from "./NewPassword.svelte";
	import CollectUsername from "./CollectUsername.svelte";
	import ProvidePrivateKey from "./ProvidePrivateKey.svelte";
	import FlowContainer from "./FlowContainer.svelte";
	import InitiationScreen from "./InitiationScreen.svelte";

	let { onSignedUp }: { onSignedUp?: () => void } = $props();

	// Define view states
	const VIEW_STATES = {
		EXISTING_USER: {
			WELCOME: "welcome",
			IMPORT: "import",
			SET_PASSPHRASE: "forExistingUser",
		},
		NEW_USER: {
			WELCOME: "welcome",
			COLLECT_USERNAME: "username",
			SET_PASSPHRASE: "forNewUser",
			PROVIDE_PRIVATE_KEY: "privateKey",
		},
	} as const;

	type ViewState =
		| (typeof VIEW_STATES.EXISTING_USER)[keyof typeof VIEW_STATES.EXISTING_USER]
		| (typeof VIEW_STATES.NEW_USER)[keyof typeof VIEW_STATES.NEW_USER];

	  let currentView = $state<ViewState>("welcome");
		let viewHistory = $state<ViewState[]>([]);
		let userFlow = $state<"EXISTING_USER" | "NEW_USER" | null>(null);
		//let userFlow = "NEW_USER"

	let collectedRecoveryString = $state("");
	let collectedUsername = $state("");


	const navigateTo = (view: ViewState):void => {
		viewHistory = [...viewHistory, currentView];
		currentView = view;
	};

	const goBack = ():void => {
		if (viewHistory.length > 0) {
			currentView = viewHistory[viewHistory.length - 1];
			viewHistory = viewHistory.slice(0, -1);
		}
	};

	const triggerOnboardingFlow = (isImport: boolean):void => {
		// Set the user flow based on selection
		userFlow = isImport ? "EXISTING_USER" : "NEW_USER";

		if (isImport) {
			navigateTo(VIEW_STATES.EXISTING_USER.IMPORT);
		} else {
			navigateTo(VIEW_STATES.NEW_USER.COLLECT_USERNAME);
		}
	};

	const handleImportProceed = ():void => {
		navigateTo(VIEW_STATES.EXISTING_USER.SET_PASSPHRASE);
	};

	const handleUserSignUpComplete = (isLoggedin: boolean):void => {
		if (isLoggedin) {
			console.log("Signed up and logged in, navigating to home");
			 onSignedUp?.();
		}
	};

	const handleUsernameCollected = ():void => {
		// not using this value as of now
		navigateTo(VIEW_STATES.NEW_USER.SET_PASSPHRASE);
	};

	const handlePassphraseSet = (isLoggedin: boolean):void => {
		if (isLoggedin) {
			console.log("Passphrase set, navigating to private key step");
			navigateTo(VIEW_STATES.NEW_USER.PROVIDE_PRIVATE_KEY);
		}
	};
</script>

<div
	class="h-full w-full flex justify-center items-center text-base text-mobile-textPrimary bg-mobile-bgPrimary ring-offset-mobile-textActive px-32 relative">
	<div class="h-full flex flex-col items-center pt-[13.5rem]">
		<img src={LivnoteLogo} alt="Livnote Logo" class="mb-10 select-none" />
		{#if currentView === "welcome"}
			<InitiationScreen onFlowSelect={triggerOnboardingFlow} />
		{:else if currentView === VIEW_STATES.EXISTING_USER.IMPORT}
			<FlowContainer onBack={goBack}>
				<BaseImportPvtKey onProceed={handleImportProceed} bind:collectedRecoveryString />
			</FlowContainer>
		{:else if currentView === VIEW_STATES.EXISTING_USER.SET_PASSPHRASE}
			<FlowContainer onBack={goBack}>
				<NewPassword
					onLogin={handleUserSignUpComplete}
					bind:collectedRecoveryString />
			</FlowContainer>
		{:else if currentView === VIEW_STATES.NEW_USER.COLLECT_USERNAME}
			<FlowContainer onBack={goBack}>
				<CollectUsername onProceed={handleUsernameCollected} bind:collectedUsername />
			</FlowContainer>
		{:else if currentView === VIEW_STATES.NEW_USER.SET_PASSPHRASE}
			<FlowContainer onBack={goBack}>
				<NewPassword
					onLogin={handlePassphraseSet}
					bind:collectedUsername
					bind:collectedRecoveryString 
					/>
			</FlowContainer>
		{:else if currentView === VIEW_STATES.NEW_USER.PROVIDE_PRIVATE_KEY}
			<FlowContainer onBack={goBack}>
				<ProvidePrivateKey onLogin={handleUserSignUpComplete} bind:collectedRecoveryString />
			</FlowContainer>
		{/if}
	</div>
</div>
