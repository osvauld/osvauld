<script lang="ts">
	import { ClosedEye, Eye } from "@osvauld/password-manager-common";
	import Loader from "./Loader.svelte";
	import { sendMessage } from "../../../common/utils/helper";
	import { StorageService } from "../../../common/utils/storageHelper";
	import PasswordStrengthValidator from "./PasswordStrengthValidator.svelte";

	// Replace createEventDispatcher with callback props
	let { onSignedUp } = $props();

	// Reactive state variables
	let username = $state("");
	let passphrase = $state("");
	let confirmPassphrase = $state("");
	let showFirstPassword = $state(false);
	let showSecondPassword = $state(false);
	let showPassphraseMismatchError = $state(false);
	let passphraseEmpty = $state(false);
	let isLoaderActive = $state(false);
	let isPassphraseAcceptable = $state(false);

	// Derived values
	let firstInputType = $derived(showFirstPassword ? "text" : "password");
	let secondInputType = $derived(showSecondPassword ? "text" : "password");
	//TODO: for dev disabling password strength check
	// let submitDisabled = $derived(
	// 	passphrase.length === 0 ||
	// 		passphrase !== confirmPassphrase ||
	// 		!isPassphraseAcceptable ||
	// 		username.length < 4,
	// );
	let submitDisabled = false;

	// Handle password strength changes from the validator component
	const handleStrengthChange = (isAcceptable: boolean) => {
		isPassphraseAcceptable = isAcceptable;
	};

	const handlePassPhraseSubmit = async (event) => {
		event.preventDefault();

		if (passphrase.length === 0 || username.length < 4) {
			passphraseEmpty = true;
			isLoaderActive = false;
			setTimeout(() => {
				passphraseEmpty = false;
			}, 1500);
			return;
		}

		isLoaderActive = true;

		if (passphrase === confirmPassphrase) {
			try {
				const response = await sendMessage("savePassphrase", {
					passphrase,
					username,
				});

				isLoaderActive = false;
				const pubkey = await sendMessage("login", { passphrase });

				await StorageService.setIsLoggedIn("true");

				// Call the callback prop instead of dispatching an event
				onSignedUp?.();
			} catch (error) {
				showPassphraseMismatchError = true;
				isLoaderActive = false;
				setTimeout(() => {
					showPassphraseMismatchError = false;
				}, 1500);
			}
		}
	};

	function onInput(event: any, type: string) {
		if (type === "passphrase") passphrase = event.target.value;
		else confirmPassphrase = event.target.value;
	}

	const togglePassword = (identification: boolean) => {
		identification
			? (showFirstPassword = !showFirstPassword)
			: (showSecondPassword = !showSecondPassword);
	};
</script>

<form
	class="flex flex-col justify-center items-center"
	onsubmit={handlePassPhraseSubmit}>
	<label for="passphrase" class="font-normal mt-6">Enter Passphrase</label>

	<div
		class="w-[300px] flex bg-osvauld-frameblack px-3 mt-4 border rounded-lg border-osvauld-iconblack">
		<input
			class="text-white bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:border-transparent focus:ring-0 outline-0 p-2 w-full"
			type={firstInputType}
			autocomplete="off"
			autocapitalize="off"
			autocorrect="off"
			id="password"
			oninput={(e) => onInput(e, "passphrase")} />

		<button
			type="button"
			class="flex justify-center items-center"
			onclick={() => togglePassword(true)}>
			{#if showFirstPassword}
				<ClosedEye />
			{:else}
				<Eye />
			{/if}
		</button>
	</div>

	<!-- Updated to use onStrengthChange callback instead of binding -->
	<PasswordStrengthValidator
		{passphrase}
		onStrengthChange={handleStrengthChange} />

	<label for="confirmPassphrase" class="font-normal">Confirm Passphrase</label>

	<div
		class="w-[300px] flex bg-osvauld-frameblack px-3 mt-4 border rounded-lg border-osvauld-iconblack">
		<input
			class="text-white bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:border-transparent focus:ring-0 p-2 outline-0 w-full"
			type={secondInputType}
			autocomplete="off"
			autocapitalize="off"
			autocorrect="off"
			id="confirmPassphrase"
			oninput={(e) => onInput(e, "confirmPassphrase")} />

		<button
			type="button"
			class="flex justify-center items-center"
			onclick={() => togglePassword(false)}>
			{#if showSecondPassword}
				<ClosedEye />
			{:else}
				<Eye />
			{/if}
		</button>
	</div>
	<label for="username" class="font-normal mt-6">Enter Username</label>
	<div
		class="w-[300px] flex bg-osvauld-frameblack px-3 mt-4 border rounded-lg border-osvauld-iconblack">
		<input
			class="text-white bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:border-transparent focus:ring-0 p-2 outline-0 w-full"
			type="text"
			autocomplete="off"
			autocapitalize="off"
			autocorrect="off"
			id="username"
			bind:value={username} />
	</div>

	{#if passphraseEmpty}
		<span
			class="mt-2 text-xs text-red-400 font-light {passphraseEmpty
				? 'visible'
				: 'invisible'}">Username should have more than 4 letters</span>
	{:else}
		<span
			class="mt-2 text-xs text-red-400 font-light {showPassphraseMismatchError
				? 'visible'
				: 'invisible'}">Passphrase doesn't match</span>
	{/if}

	<button
		class="{submitDisabled
			? 'border border-osvauld-iconblack text-osvauld-sheffieldgrey'
			: 'bg-osvauld-carolinablue text-osvauld-ninjablack'} py-2 px-10 mt-8 rounded-lg font-medium w-[150px] flex justify-center items-center whitespace-nowrap"
		type="submit"
		disabled={submitDisabled}>
		{#if isLoaderActive}
			<Loader size={24} color="#1F242A" duration={1} />
		{:else}
			<span>Submit</span>
		{/if}</button>
</form>
