<script lang="ts">
	import Loader from "./Loader.svelte";
	import { ClosedEye, Eye } from "@osvauld/password-manager-common";
	import { sendMessage } from "../../../common/utils/helper";
	import { StorageService } from "../../../common/utils/storageHelper";
	let { authenticated } = $props();
	let passphrase = $state("");
	let showPassword = $state(false);
	let errorMessage = $state(false);
	let isLoaderActive = $state(false);
	let inputElem: any = $state();

	function toggleShowPassword() {
		showPassword = !showPassword;
	}

	const onInput = (event: any) => {
		passphrase = event.target.value;
	};

	function autofocus(node: any) {
		node.focus();
	}

	let type = $derived(showPassword ? "text" : "password");

	async function handleSubmit(e: any) {
		e.preventDefault();
		isLoaderActive = true;
		const pubkey = await sendMessage("login", { passphrase });
		if (pubkey) {
			authenticated?.(true);
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
	class="h-auto mt-10 flex justify-center items-center text-base font-normal text-osvauld-sheffieldgrey p-12 rounded-lg">
	<form
		class="flex flex-col justify-center items-center"
		onsubmit={handleSubmit}>
		<label class="text-lg font-normal text-white mb-3 " for="passphrase">Enter Passphrase</label>

		<div
			class="flex px-3 mt-4 border rounded-lg border-osvauld-iconblack focus-within:border-livnotePink ">
			<input
				class="text-white h-[3.3rem] p-2 border-0 tracking-wider font-normal border-transparent focus:ring-0 focus:border-osvauld-activeBorder focus:outline-none"
				{type}
				id="passphrase"
				autocomplete="off"
				value={passphrase}
				use:autofocus
				bind:this={inputElem}
				oninput={onInput} />
			<button
				type="button"
				class="flex justify-center items-center border border-transparent focus:border-livnotePink outline-0 rounded-lg p-1 cursor-pointer"
				onclick={toggleShowPassword}>
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
			class="bg-livnotePink py-2 px-10 mt-8 rounded-lg text-primarydark font-medium w-[150px] flex justify-center items-center whitespace-nowrap "
			type="submit">
			{#if isLoaderActive}
				<Loader size={24} color="#1F242A" duration={1} />
			{:else}
				<span>Submit</span>
			{/if}</button>
	</form>
</div>
