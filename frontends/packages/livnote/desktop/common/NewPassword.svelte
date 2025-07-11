<script lang="ts">
	import Loader from "./Loader.svelte";
	import { ClosedEye, Eye, Tick } from "@osvauld/password-manager-common";
	import PasswordStrengthValidator from "./PasswordStrengthValidator.svelte";
	import { sendMessage } from "../../../common/utils/helper";
	import { onMount } from "svelte";
	import { StorageService } from "../../../common/utils/storageHelper";
import { dataState } from "../state";

	// Replace createEventDispatcher with callback props
	let {
		onLogin,
		recoveryData,
	}: {
		onLogin: (isCorrect: boolean) => void;
		recoveryData?: string;
	} = $props();


	// State variables
	let passphrase = $state("");
	let reenteredPassPhrase = $state("");
	let showPassword = $state(false);
	let showReenteredPassword = $state(false);
	let isLoaderActive = $state(false);
	let isPassphraseAcceptable = $state(false);

	// Derived values
	// TODO: for dev disabling this
	// let submitDisabled = $derived(
	// 	passphrase.length === 0 || passphrase.length < 6 ||
	// 		passphrase !== reenteredPassPhrase 
	// );

	let submitDisabled = $state(false);


	const togglePasswordVisibility = (isInitialResponse: boolean) => {
		if (isInitialResponse) {
			showPassword = !showPassword;
		} else {
			showReenteredPassword = !showReenteredPassword;
		}
	};

	const autofocus = (node: HTMLInputElement) => {
		node.focus();
	};

	const handleInputChange = (event: Event) => {
		if (event.target instanceof HTMLInputElement) {
			passphrase = event.target.value;
		}
	};

	const handleConfirmationInputChange = (event: Event) => {
		if (event.target instanceof HTMLInputElement) {
			reenteredPassPhrase = event.target.value;
		}
	};

	const handleStrengthChange = (isAcceptable: boolean) => {
		isPassphraseAcceptable = isAcceptable;
	};


	const triggerSignup = async (passphrase: string) => {
		if (recoveryData) {
			// for recovery flow
			let recovery = JSON.parse(recoveryData);
			const result = await sendMessage("addDevice", {
				passphrase,
				certificate: recovery.certificate,
			});
			//TODO: add username to addDevice API for collecting username here and setting it on the dashboard
			await sendMessage("login", { passphrase });
			console.log("sending first device connect message");
			await sendMessage("firstDeviceConnect", { ticket: recovery.ticket });
			//TODO: need error handling here
			onLogin?.(true);
		} else {
			// for new user flow
			const response = await sendMessage("savePassphrase", {
				passphrase,
				username: dataState.signupUsername,
			});

			const pubkey = await sendMessage("login", { passphrase });

			dataState.signupPubKey = JSON.stringify(pubkey);

			await StorageService.setIsLoggedIn("true");
			onLogin?.(true);
		}
	};

	const handleSubmit = async (event: Event) => {
		event.preventDefault();

		if (submitDisabled) return;

		isLoaderActive = true;
		await triggerSignup(passphrase);
		isLoaderActive = false;
	};

	const preventDefault = (e: Event) => e.preventDefault();
</script>

<form onsubmit={handleSubmit} class="flex flex-col items-center justify-center select-none">
	<h1 class="text-xl font-semibold text-white mb-3 -mt-10">Set passphrase</h1>
	<p class="text-sm font-inter font-extralight text-mobile-textActive mb-14 text-center">This will be used to encrypt and decrypt your data. <br/> This will not leave your device.</p>
	<div class="h-[20rem] mt-6">
		<label
			for="new-passphrase"
			class="font-normal text-osvauld-quarzowhite self-start "
			>Enter passphrase</label>
		<div
			class="flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-livnotePink mt-2">
			<input
				class="select-none w-[20rem] h-[3.3rem] text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:ring-0  outline-none"
				type={showPassword ? "text" : "password"}
				id="new-passphrase"
				autocomplete="off"
				autocorrect="off"
				use:autofocus
				oninput={handleInputChange}
				oncopy={preventDefault}
			/>

			<!-- {#if isPassphraseAcceptable}
			<span class="pr-2"><Tick /></span>
		{/if} -->
			<button
				type="button"
				class="flex justify-center items-center border border-transparent focus:border-livnotePink outline-0 rounded-lg p-1 cursor-pointer"
				onclick={() => togglePasswordVisibility(true)}>
				{#if showPassword}
					<ClosedEye />
				{:else}
					<Eye />
				{/if}
			</button>
		</div>
		<PasswordStrengthValidator
			{passphrase}
			onStrengthChange={handleStrengthChange} />
		<label
			for="confirm-passphrase"
			class="font-normal mt-2 text-osvauld-quarzowhite self-start"
			>Confirm passphrase</label>
		<div
			class="flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-livnotePink mt-2">
			<input
				class=" w-[20rem] h-[3.3rem] text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent ring-0 outline-none"
				type={showReenteredPassword ? "text" : "password"}
				id="confirm-passphrase"
				autocomplete="off"
				autocorrect="off"
				oninput={handleConfirmationInputChange}
				oncopy={preventDefault} />

			<button
				type="button"
				class="flex justify-center items-center border border-transparent focus:border-livnotePink outline-0 rounded-lg p-1 cursor-pointer"
				onclick={() => togglePasswordVisibility(false)}>
				{#if showReenteredPassword}
					<ClosedEye />
				{:else}
					<Eye />
				{/if}
			</button>
		</div>
	</div>

	<button
		class="w-[24rem] py-2 px-10 mt-8 rounded-lg font-medium flex justify-center items-center whitespace-nowrap cursor-pointer border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300"
		class:bg-livnotePink={!submitDisabled}
		class:text-mobile-bgPrimary={!submitDisabled}
		class:bg-signupGray={submitDisabled}
		class:text-white={submitDisabled}
		type="submit"
		disabled={submitDisabled}>
		{#if isLoaderActive}
			<Loader color="#fff" size={32} />
		{:else}
			<span>Submit</span>
		{/if}
	</button>
</form>
