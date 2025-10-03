<script lang="ts">
	import Loader from "./Loader.svelte";
	import { ClosedEye, Eye } from "@osvauld/icons";
	import { sendMessage } from "../utils/helper";

	let { authenticated } = $props();
	let passphrase = $state("");
	let showPassword = $state(false);
	let errorMessage = $state(false);
	let isLoaderActive = $state(false);
	let inputElem: HTMLInputElement | undefined = $state();

	const handleToggleShowPassword = () => {
		showPassword = !showPassword;
	};

	const handleInput = (event: Event) => {
		if (event.target instanceof HTMLInputElement) {
			passphrase = event.target.value;
		}
	};

	let type = $derived(showPassword ? "text" : "password");

	async function handleSubmit(e: Event) {
		e.preventDefault();
		isLoaderActive = true;
		let pubkey;
		try {
			pubkey = await sendMessage("login", { passphrase });
		} catch (error) {
			console.error(error);
			errorMessage = true;
			passphrase = "";
		} finally {
			if (pubkey) {
				authenticated?.(true);
			}
			isLoaderActive = false;
			setTimeout(() => {
				errorMessage = false;
			}, 1500);
		}
	}

	// Maintain focus whenever possible - reacts to all state changes
	$effect(() => {
		if (inputElem && !isLoaderActive) {
			// This will refocus when:
			// - showPassword changes (type toggle)
			// - errorMessage changes
			// - passphrase is cleared
			// - component mounts
			inputElem.focus();
		}
	});
</script>

<div
	class="h-auto mt-10 flex justify-center items-center text-base font-normal text-white bg-bgPrimary p-12 rounded-lg"
>
	<form
		class="flex flex-col justify-center items-center"
		onsubmit={handleSubmit}
	>
		<label for="passphrase">Enter Passphrase</label>

		<div
			class="flex px-3 mt-4 border rounded-lg border-osvauld-iconblack focus-within:border-osvauld-activeBorder"
		>
			<input
				class="text-white p-2 border-0 tracking-wider font-normal border-transparent focus:ring-0 focus:border-osvauld-activeBorder focus:outline-none"
				{type}
				id="passphrase"
				autocomplete="off"
				value={passphrase}
				bind:this={inputElem}
				oninput={handleInput}
				aria-label="Passphrase"
				aria-invalid={errorMessage}
				aria-describedby={errorMessage ? "passphrase-error" : undefined}
			/>
			<button
				type="button"
				class="flex justify-center items-center border border-transparent focus:border-osvauld-activeBorder outline-0 rounded-lg p-1 cursor-pointer"
				onclick={handleToggleShowPassword}
				aria-label={showPassword ? "Hide passphrase" : "Show passphrase"}
				onmousedown={(e) => e.preventDefault()}
			>
				{#if showPassword}
					<ClosedEye />
				{:else}
					<Eye />
				{/if}
			</button>
		</div>
		<span
			id="passphrase-error"
			class="text-xs text-red-500 font-light mt-2 {errorMessage
				? 'visible'
				: 'invisible'}"
			role="alert">Wrong Passphrase</span
		>
		<button
			class="bg-livnotePink py-2 px-10 mt-8 rounded-lg text-primarydark font-medium w-[150px] flex justify-center items-center whitespace-nowrap"
			type="submit"
		>
			{#if isLoaderActive}
				<Loader size={24} color="#1F242A" duration={1} />
			{:else}
				<span>Submit</span>
			{/if}</button
		>
	</form>
</div>
