<script lang="ts">
	import Loader from "./Loader.svelte";
	import ClosedEye from "@osvauld/password-manager-common/icons/closedEye.svelte";
	import Eye from "@osvauld/password-manager-common/icons/eye.svelte";
	import Tick from "@osvauld/password-manager-common/icons/tick.svelte";
	import { createEventDispatcher } from "svelte";
	const dispatch = createEventDispatcher();

	import PasswordStrengthValidator from "./PasswordStrengthValidator.svelte";

	let passphrase = "";
	let reenteredPassPhrase = "";
	let showPassword = false;
	let showReenteredPassword = false;
	let isLoaderActive = false;
	let isPassphraseAcceptable = false;

	$: submitDisabled =
		passphrase.length === 0 || passphrase !== reenteredPassPhrase;

	const togglePasswordVisibility = (isInitialResponse: boolean) => {
		if (isInitialResponse) {
			showPassword = !showPassword;
		} else {
			showReenteredPassword = !showReenteredPassword;
		}
	};

	const autofocus = (node: any) => {
		node.focus();
	};

	const handleInputChange = (event: any) => {
		passphrase = event.target.value;
	};

	const handleConfirmationInputChange = (event: any) => {
		reenteredPassPhrase = event.target.value;
	};

	const handleSubmit = () => {
		isLoaderActive = true;
		dispatch("submit", { passphrase });
	};
</script>

<form
	on:submit|preventDefault="{handleSubmit}"
	class="flex flex-col items-center justify-center">
	<label for="passphrase" class="font-normal mt-6 mb-2 text-osvauld-quarzowhite"
		>Enter New Passphrase</label>
	<div
		class="flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-osvauld-activeBorder">
		<input
			class="text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:ring-0 focus:border-osvauld-activeBorder focus:outline-none"
			type="{showPassword ? 'text' : 'password'}"
			id="passphrase"
			autocomplete="off"
			autocorrect="off"
			use:autofocus
			on:input="{handleInputChange}" />

		{#if isPassphraseAcceptable}
			<span class="pr-2"><Tick /></span>
		{/if}
		<button
			type="button"
			class="flex justify-center items-center"
			on:click="{() => togglePasswordVisibility(true)}">
			{#if showPassword}
				<ClosedEye />
			{:else}
				<Eye />
			{/if}
		</button>
	</div>
	<PasswordStrengthValidator {passphrase} bind:isPassphraseAcceptable />
	<label for="passphrase" class="font-normal mt-2 mb-2 text-osvauld-quarzowhite"
		>Confirm New Passphrase</label>
	<div
		class="flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-osvauld-activeBorder">
		<input
			class="text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:ring-0 focus:border-osvauld-activeBorder focus:outline-none"
			type="{showReenteredPassword ? 'text' : 'password'}"
			id="passphrase"
			autocomplete="off"
			autocorrect="off"
			on:input="{handleConfirmationInputChange}" />

		<button
			type="button"
			class="flex justify-center items-center"
			on:click="{() => togglePasswordVisibility(false)}">
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
		disabled="{submitDisabled}">
		{#if isLoaderActive}
			<Loader color="#fff" size="{32}" />
		{:else}
			<span>Submit</span>
		{/if}
	</button>
</form>
