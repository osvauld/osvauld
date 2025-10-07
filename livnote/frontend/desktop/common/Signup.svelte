<script lang="ts">
	// @ts-ignore: Image import for Svelte, handled by bundler
	import LivnoteLogo from "../assets/logo.png";
	import BaseImportPvtKey from "./BaseImportPvtKey.svelte";
	import NewPassword from "./NewPassword.svelte";
	import CollectUsername from "./CollectUsername.svelte";
	import ProvidePrivateKey from "./ProvidePrivateKey.svelte";
	import FlowContainer from "./FlowContainer.svelte";
	import InitiationScreen from "./InitiationScreen.svelte";
	import { sendMessage } from "../utils/helper";

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
	let isLoaderActive = $state(false);

	const navigateTo = (view: ViewState): void => {
		viewHistory = [...viewHistory, currentView];
		currentView = view;
	};

	const goBack = (): void => {
		if (viewHistory.length > 0) {
			const previousView = viewHistory[viewHistory.length - 1];
			currentView = previousView;
			viewHistory = viewHistory.slice(0, -1);

			// Clear states based on the target view to prevent cross-flow contamination
			clearStatesForView(previousView);
		}
	};

	const clearStatesForView = (targetView: ViewState): void => {
		// Always clear loader state when navigating
		isLoaderActive = false;

		switch (targetView) {
			case "welcome":
				// Reset all flow-related states when going back to welcome
				userFlow = null;
				collectedRecoveryString = "";
				collectedUsername = "";
				break;

			case VIEW_STATES.EXISTING_USER.IMPORT:
				// Clear recovery string when going back to import step
				collectedRecoveryString = "";
				break;

			case VIEW_STATES.NEW_USER.COLLECT_USERNAME:
				// Clear username when going back to username collection
				collectedUsername = "";
				// Also clear recovery string if it was set during new user flow
				collectedRecoveryString = "";
				break;

			case VIEW_STATES.EXISTING_USER.SET_PASSPHRASE:
			case VIEW_STATES.NEW_USER.SET_PASSPHRASE:
				// Keep existing data for password setup steps
				// Only clear loader state (handled above)
				break;

			case VIEW_STATES.NEW_USER.PROVIDE_PRIVATE_KEY:
				// Keep recovery string for private key provision
				// Only clear loader state (handled above)
				break;
		}
	};

	const triggerOnboardingFlow = (isImport: boolean): void => {
		// Set the user flow based on selection
		userFlow = isImport ? "EXISTING_USER" : "NEW_USER";

		if (isImport) {
			navigateTo(VIEW_STATES.EXISTING_USER.IMPORT);
		} else {
			navigateTo(VIEW_STATES.NEW_USER.COLLECT_USERNAME);
		}
	};

	const handleImportProceed = (): void => {
		navigateTo(VIEW_STATES.EXISTING_USER.SET_PASSPHRASE);
	};

	const handleUserSignUpComplete = (isLoggedin: boolean): void => {
		if (isLoggedin) {
			onSignedUp?.();
		}
	};

	const handleUsernameCollected = (): void => {
		// not using this value as of now
		navigateTo(VIEW_STATES.NEW_USER.SET_PASSPHRASE);
	};

	const handleRecoveryPasswordSetup = async (
		passphrase: string,
	): Promise<void> => {
		isLoaderActive = true;

		try {
			let parsedRecoveryData;
			try {
				parsedRecoveryData = JSON.parse(collectedRecoveryString);
			} catch (error) {
				console.error("Error parsing recovery data:", error);
				// No check on validity of private key imported on previous step
				// So error will occur here
				// need to disable button here
				collectedRecoveryString = "";
				return;
			}

			const result = await sendMessage("addDevice", {
				passphrase,
				certificate: parsedRecoveryData.certificate,
				username: parsedRecoveryData.username,
				device_id: parsedRecoveryData.deviceId,
			});
			//TODO: add username to addDevice API for collecting username here and setting it on the dashboard
			await sendMessage("login", { passphrase });
			handleUserSignUpComplete(true);
		} catch (error) {
			console.error("Error during recovery password setup:", error);
		} finally {
			isLoaderActive = false;
		}
	};

	const handleNewUserPasswordSetup = async (
		passphrase: string,
	): Promise<void> => {
		isLoaderActive = true;

		try {
			const response = await sendMessage("savePassphrase", {
				passphrase,
				username: collectedUsername,
			});
			const privatekey = await sendMessage("login", { passphrase });
			const certificate = await sendMessage("exportCertificate", {
				passphrase,
			});
			collectedRecoveryString = JSON.stringify(certificate);
			navigateTo(VIEW_STATES.NEW_USER.PROVIDE_PRIVATE_KEY);
		} catch (error) {
			console.error("Error during new user password setup:", error);
		} finally {
			isLoaderActive = false;
		}
	};

	const handleReturnedNewPassword = async (
		passphrase: string,
	): Promise<void> => {
		// Route to appropriate handler based on user flow
		if (userFlow === "EXISTING_USER") {
			await handleRecoveryPasswordSetup(passphrase);
		} else {
			await handleNewUserPasswordSetup(passphrase);
		}
	};
</script>

<div
	class="h-full w-full flex justify-center items-center text-base text-mobile-textPrimary bg-mobile-bgPrimary ring-offset-mobile-textActive px-32 py-16 relative overflow-y-auto"
>
	<div class="h-full flex flex-col items-center">
		<img src={LivnoteLogo} alt="Livnote Logo" class="mb-10 select-none" />
		{#if currentView === "welcome"}
			<InitiationScreen onFlowSelect={triggerOnboardingFlow} />
		{:else if currentView === VIEW_STATES.EXISTING_USER.IMPORT}
			<FlowContainer onBack={goBack}>
				<BaseImportPvtKey
					onProceed={handleImportProceed}
					bind:collectedRecoveryString
				/>
			</FlowContainer>
		{:else if currentView === VIEW_STATES.EXISTING_USER.SET_PASSPHRASE}
			<FlowContainer onBack={goBack}>
				<NewPassword {isLoaderActive} onReturn={handleReturnedNewPassword} />
			</FlowContainer>
		{:else if currentView === VIEW_STATES.NEW_USER.COLLECT_USERNAME}
			<FlowContainer onBack={goBack}>
				<CollectUsername
					onProceed={handleUsernameCollected}
					bind:collectedUsername
				/>
			</FlowContainer>
		{:else if currentView === VIEW_STATES.NEW_USER.SET_PASSPHRASE}
			<FlowContainer onBack={goBack}>
				<NewPassword {isLoaderActive} onReturn={handleReturnedNewPassword} />
			</FlowContainer>
		{:else if currentView === VIEW_STATES.NEW_USER.PROVIDE_PRIVATE_KEY}
			<FlowContainer onBack={goBack} showBack={false}>
				<ProvidePrivateKey
					onLogin={handleUserSignUpComplete}
					bind:collectedRecoveryString
				/>
			</FlowContainer>
		{/if}
	</div>
</div>
