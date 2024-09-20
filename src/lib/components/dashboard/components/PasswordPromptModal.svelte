<script lang="ts">
	import { onMount } from "svelte";
	import { sendMessage } from "../helper";
	import { getTokenAndBaseUrl } from "../helper";

	import { fly } from "svelte/transition";
	import { promptPassword } from "../store";
	import { ClosePanel } from "../icons";
	import SuccessView from "../../basic/SuccessView.svelte";

	let password: string = "";
	let success: boolean = false;
	let errorView: boolean = false;

	const closeModal = () => {
		promptPassword.set(false);
	};

	const autofocus = (node: any) => {
		node.focus();
	};

	const handleSubmit = async () => {
		const { baseUrl } = await getTokenAndBaseUrl();
		const certificate = await sendMessage("exportCertificate", {
			passphrase: password,
		});
		if (certificate) {
			const exporter = JSON.stringify({ certificate, baseUrl });
			await navigator.clipboard.writeText(exporter);
			success = true;
		} else {
			errorView = true;
		}
		setTimeout(() => {
			closeModal();
		}, 1500);
	};

	onMount(() => {
		const handleKeydown = (event: KeyboardEvent) => {
			if (event.key === "Escape") {
				closeModal();
			}
		};

		window.addEventListener("keydown", handleKeydown);

		return () => {
			window.removeEventListener("keydown", handleKeydown);
		};
	});
</script>

<div
	class="fixed inset-0 flex items-center justify-center z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[2px]"
	on:click|preventDefault="{closeModal}"
	role="presentation"
>
	<div
		class="p-4 bg-osvauld-frameblack border border-osvauld-activeBorder rounded-3xl w-[24rem] h-[18rem]"
		on:click|stopPropagation
		role="presentation"
		aria-labelledby="export-recovery-data"
		in:fly
		out:fly
	>
		{#if errorView}
			<SuccessView status="{false}" recovery="{true}" />
		{:else if success}
			<SuccessView status="{true}" recovery="{true}" />
		{:else}
			<form
				class="flex flex-col h-full"
				on:submit|preventDefault="{handleSubmit}"
			>
				<div class="flex justify-between items-center w-full">
					<span
						id="export-recovery-data"
						class="text-[21px] font-medium text-osvauld-quarzowhite"
					>
						Confirm Master Password
					</span>
					<button
						class="cursor-pointer p-2"
						type="button"
						on:click|preventDefault="{closeModal}"
						aria-label="Close"
					>
						<ClosePanel />
					</button>
				</div>
				<div
					class="border-b border-osvauld-iconblack w-[calc(100%+2rem)] -translate-x-4"
				></div>
				<div class="grow flex justify-center items-center">
					<input
						id="password"
						type="password"
						use:autofocus
						required
						bind:value="{password}"
						class="py-1 rounded-sm items-center text-base bg-osvauld-frameblack border-osvauld-iconblack w-[95%] h-10 mx-2 focus:border-osvauld-iconblack focus:ring-0 form-input"
						autocomplete="current-password"
						aria-label="Master Password"
					/>
				</div>
				<button
					class="border border-osvauld-iconblack w-[10rem] py-3 px-6 my-4 mx-auto text-base font-medium text-osvauld-textActive rounded-md hover:bg-osvauld-carolinablue hover:border-osvauld-carolinablue hover:text-osvauld-frameblack cursor-pointer"
					type="submit"
					disabled="{!password}"
				>
					Proceed
				</button>
			</form>
		{/if}
	</div>
</div>
