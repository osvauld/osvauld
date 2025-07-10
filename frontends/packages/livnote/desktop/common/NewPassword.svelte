<script lang="ts">
	import Loader from "./Loader.svelte";
	import { ClosedEye, Eye, Tick } from "@osvauld/password-manager-common";
	import PasswordStrengthValidator from "./PasswordStrengthValidator.svelte";
	import { sendMessage } from "../../../common/utils/helper";

	// Replace createEventDispatcher with callback props
	let { onLogin, recoveryData } = $props();

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
	// 	passphrase.length === 0 ||
	// 		passphrase !== reenteredPassPhrase ||
	// 		!isPassphraseAcceptable,
	// );
	let submitDisabled = false;

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


	const triggerAccountRecovery = async (passphrase: string) => {
		return onLogin?.(true);
		let recovery = JSON.parse(recoveryData);
		const result = await sendMessage("addDevice", {
			passphrase,
			certificate: recovery.certificate,
		});
		await sendMessage("login", { passphrase });
		console.log("sending first device connect message");
		await sendMessage("firstDeviceConnect", { ticket: recovery.ticket });
    // need error handling here
		onLogin?.(true);
	};

	const handleSubmit = (event: Event) => {
		event.preventDefault();

		if (submitDisabled) return;

		isLoaderActive = true;
		triggerAccountRecovery(passphrase)
	};

	const preventDefault = (e: Event) => e.preventDefault();
</script>

<form onsubmit={handleSubmit} class="flex flex-col items-center justify-center select-none">
	<h1 class="text-xl font-semibold text-white mb-3 -mt-10">Add your name</h1>
	<p class="text-sm font-inter font-extralight text-mobile-textActive mb-14 text-center">Only seen by people you share something with. <br/>There is no central registry for these names.</p>
	<label
		for="new-passphrase"
		class="font-normal mt-6 mb-2 text-osvauld-quarzowhite self-start"
		>Enter passphrase</label>
	<div
		class="flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-livnotePink ">
		<input
			class="select-none w-[24rem] h-[3.3rem] text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:ring-0  outline-none"
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
		class="font-normal mt-2 mb-2 text-osvauld-quarzowhite self-start"
		>Confirm passphrase</label>
	<div
		class="flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-livnotePink ">
		<input
			class=" w-[24rem] h-[3.3rem] text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent ring-0 outline-none"
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

	<button
		class="w-full  py-2 px-10 mt-8 rounded-lg font-medium  flex justify-center items-center whitespace-nowrap cursor-pointer bg-signupGray text-white  border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300  enabled:hover:bg-livnotePink enabled:hover:text-mobile-bgPrimary"
		type="submit"
		disabled={submitDisabled}>
		{#if isLoaderActive}
			<Loader color="#fff" size={32} />
		{:else}
			<span>Submit</span>
		{/if}
	</button>
</form>
