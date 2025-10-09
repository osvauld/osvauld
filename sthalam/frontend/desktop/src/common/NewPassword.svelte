
<script lang="ts">
	import Loader from "./Loader.svelte";
	import { ClosedEye, Eye } from "../lib/icons/src";
	import PasswordStrengthValidator from "./PasswordStrengthValidator.svelte";

	let {
		onReturn,
		isLoaderActive
	}: {
		onReturn: (passphrase: string) => void;
		isLoaderActive: boolean;
	} = $props();


	// State variables
	let passphrase = $state("");
	let reenteredPassPhrase = $state("");
	let showPassword = $state(false);
	let showReenteredPassword = $state(false);
	let isPassphraseAcceptable = $state(false);

	// Derived values
	// TODO: for dev disabling this
	// let submitDisabled = $derived(
	// 	passphrase.length === 0 || passphrase.length < 6 ||
	// 		passphrase !== reenteredPassPhrase 
	// );

	let submitDisabled =  $derived(
		passphrase.length === 0 ||
			passphrase !== reenteredPassPhrase 
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


	const handleSubmit = async (event: Event) => {
		event.preventDefault();

		if (submitDisabled) return;
		onReturn?.(passphrase);
	};

	const preventDefault = (e: Event) => e.preventDefault();
</script>


<form onsubmit={handleSubmit} class="h-full flex flex-col items-center justify-around py-10">
	<div class="flex flex-col items-center justify-center mb-4">
		<h1 class="text-xl font-semibold text-white">Set passphrase</h1>
		<p class="text-sm font-inter font-extralight text-mobile-textActive  text-center">This will be used to encrypt and decrypt your data. <br/> This will not leave your device.</p>
    </div>
	<div class="mb-4">
		<label
			for="new-passphrase"
			class="font-normal mb-2 text-white self-start block"
			>Enter passphrase</label>
		<div
			class="w-[24rem] flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-livnotePink ">
			<input
				class="w-full h-[3.3rem] text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal focus:ring-0 focus:focus:outline-none"
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
			class="font-normal mb-2 text-white self-start block"
			>Confirm passphrase</label>
		<div
			class="w-[24rem] flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-livnotePink ">
			<input
				class="w-full h-[3.3rem] text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal focus:ring-0 focus:focus:outline-none"
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
		class="w-[24rem] h-12 px-10 rounded-lg font-medium flex justify-center items-center whitespace-nowrap cursor-pointer border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300"
		class:bg-livnotePink={!submitDisabled}
		class:text-black={!submitDisabled}
		class:bg-signupGray={submitDisabled}
		class:text-white={submitDisabled}
		type="submit"
		disabled={submitDisabled}>
		{#if isLoaderActive}
			<Loader color="#000" size={20} />
		{:else}
			<span>Submit</span>
		{/if}
	</button>

</form>