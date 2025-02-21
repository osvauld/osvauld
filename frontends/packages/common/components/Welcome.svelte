<script lang="ts">
	import Eye from "../icons/eye.svelte";
	import Loader from "./Loader.svelte";
	import { createEventDispatcher } from "svelte";
	import ClosedEye from "../icons/closedEye.svelte";
	import { sendMessage } from "../utils/helper";
	import { StorageService } from "../utils/storageHelper";
	const dispatch = createEventDispatcher();

	let passphrase = "";
	let showPassword = false;
	let errorMessage = false;
	let isLoaderActive = false;
	let inputElem: any;

	function toggleShowPassword() {
		showPassword = !showPassword;
	}

	const onInput = (event: any) => {
		passphrase = event.target.value;
	};
	function autofocus(node: any) {
		node.focus();
	}

	$: type = showPassword ? "text" : "password";

	async function handleSubmit() {
		isLoaderActive = true;
		const pubkey = await sendMessage("login", { passphrase });
		if (pubkey) {
			dispatch("authenticated", true);
		} else {
			isLoaderActive = false;
			errorMessage = true;
			passphrase = "";
			autofocus(inputElem);
			setTimeout(() => {
				errorMessage = false;
			}, 1500);
		}
	}
</script>

<div
	class="h-auto mt-10 flex justify-center items-center text-base font-normal text-osvauld-sheffieldgrey">
	<form
		class="flex flex-col justify-center items-center"
		on:submit|preventDefault={handleSubmit}>
		<label for="passphrase">Enter Passphrase</label>

		<div
			class="flex bg-osvauld-frameblack px-3 mt-4 border rounded-lg border-osvauld-iconblack focus-within:border-osvauld-activeBorder">
			<input
				class="text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:ring-0 focus:border-osvauld-activeBorder focus:outline-none"
				{type}
				id="passphrase"
				autocomplete="off"
				value={passphrase}
				use:autofocus
				bind:this={inputElem}
				on:input={onInput} />
			<button
				type="button"
				class="flex justify-center items-center"
				on:click={toggleShowPassword}>
				{#if showPassword}
					<ClosedEye />
				{:else}
					<Eye />
				{/if}
			</button>
		</div>
		<span
			class="text-xs text-red-500 font-light mt-2 {errorMessage
				? 'visible'
				: 'invisible'}">Wrong Passphrase</span>
		<button
			class="bg-osvauld-carolinablue py-2 px-10 mt-8 rounded-lg text-osvauld-ninjablack font-medium w-[150px] flex justify-center items-center whitespace-nowrap"
			type="submit">
			{#if isLoaderActive}
				<Loader size={24} color="#1F242A" duration={1} />
			{:else}
				<span>Submit</span>
			{/if}</button>
	</form>
</div>
