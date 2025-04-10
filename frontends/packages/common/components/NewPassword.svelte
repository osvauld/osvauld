<script lang="ts">
	import Loader from "./Loader.svelte";
	import ClosedEye from "@osvauld/password-manager-common/icons/closedEye.svelte";
	import Eye from "@osvauld/password-manager-common/icons/eye.svelte";
	import Tick from "@osvauld/password-manager-common/icons/tick.svelte";
	import PasswordStrengthValidator from "./PasswordStrengthValidator.svelte";

	// Replace createEventDispatcher with callback props
	let { onSubmit } = $props();

	// State variables
	let passphrase = $state("");
	let reenteredPassPhrase = $state("");
	let showPassword = $state(false);
	let showReenteredPassword = $state(false);
	let isLoaderActive = $state(false);
	let isPassphraseAcceptable = $state(false);

	// Derived values
	let submitDisabled = $derived(
		passphrase.length === 0 ||
			passphrase !== reenteredPassPhrase ||
			!isPassphraseAcceptable,
	);

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

	const handleSubmit = (event: Event) => {
		event.preventDefault();

		if (submitDisabled) return;

		isLoaderActive = true;
		onSubmit?.({ passphrase });
	};
</script>

<form onsubmit={handleSubmit} class="flex flex-col items-center justify-center">
	<label
		for="new-passphrase"
		class="font-normal mt-6 mb-2 text-osvauld-quarzowhite"
		>Enter New Passphrase</label>
	<div
		class="flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-osvauld-activeBorder">
		<input
			class="text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:ring-0 focus:border-osvauld-activeBorder focus:outline-none"
			type={showPassword ? "text" : "password"}
			id="new-passphrase"
			autocomplete="off"
			autocorrect="off"
			use:autofocus
			oninput={handleInputChange} />

		{#if isPassphraseAcceptable}
			<span class="pr-2"><Tick /></span>
		{/if}
		<button
			type="button"
			class="flex justify-center items-center"
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
		class="font-normal mt-2 mb-2 text-osvauld-quarzowhite"
		>Confirm New Passphrase</label>
	<div
		class="flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-osvauld-activeBorder">
		<input
			class="text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:ring-0 focus:border-osvauld-activeBorder focus:outline-none"
			type={showReenteredPassword ? "text" : "password"}
			id="confirm-passphrase"
			autocomplete="off"
			autocorrect="off"
			oninput={handleConfirmationInputChange} />

		<button
			type="button"
			class="flex justify-center items-center"
			onclick={() => togglePasswordVisibility(false)}>
			{#if showReenteredPassword}
				<ClosedEye />
			{:else}
				<Eye />
			{/if}
		</button>
	</div>

	<button
		class="{submitDisabled
			? 'border border-osvauld-iconblack text-osvauld-sheffieldgrey'
			: 'bg-osvauld-carolinablue text-osvauld-ninjablack'} py-2 px-10 mt-8 rounded-lg font-medium w-[150px] flex justify-center items-center whitespace-nowrap cursor-pointer"
		type="submit"
		disabled={submitDisabled}>
		{#if isLoaderActive}
			<Loader color="#fff" size={32} />
		{:else}
			<span>Submit</span>
		{/if}
	</button>
</form>
