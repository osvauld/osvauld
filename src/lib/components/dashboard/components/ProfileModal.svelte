<script lang="ts">
	import browser from "webextension-polyfill";
	import { clickOutside, getTokenAndBaseUrl, sendMessage } from "../helper";
	import { onMount, onDestroy } from "svelte";
	import { CopyIcon, Tick } from "../icons";
	import { createEventDispatcher } from "svelte";
	import { scale } from "svelte/transition";
	import { promptPassword } from "../store";

	const dispatch = createEventDispatcher();

	export let top: number;
	export let left: number;
	export let username: string;
	let isUsernameHovered = false;
	let isRecoveryHovered = false;
	let copied = false;

	function closeModal() {
		dispatch("close", true);
	}

	const handleKeyDown = (event: KeyboardEvent) => {
		if (event.key === "Escape") {
			closeModal();
		}
	};

	const handleClickOutside = (e: any) => {
		e.stopPropagation();
		e.preventDefault();
		closeModal();
	};

	const delayFunction = () => {
		copied = true;
		setTimeout(() => {
			copied = false;
		}, 1500);
	};

	const copyUsername = async () => {
		await navigator.clipboard.writeText(username);
		delayFunction();
	};

	const passwordprompter = async () => {
		promptPassword.set(true);
	};

	const initiateRecovery = async () => {
		const { baseUrl } = await getTokenAndBaseUrl();
		// Here a modal needs to be initiated to confirm master password
		// Once submit, we can call this function then instead of directly calling it
		const certificate = await sendMessage("exportCertificate", {
			passphrase: "test",
		});

		// Now the certificate string is here
		const encryptionPvtKeyObj =
			await browser.storage.local.get("encryptionPvtKey");
		const signPvtKeyObj = await browser.storage.local.get("signPvtKey");
		const encryptionKey = encryptionPvtKeyObj.encryptionPvtKey;
		const signKey = signPvtKeyObj.signPvtKey;
		const exporter = JSON.stringify({ encryptionKey, signKey, baseUrl });
		await navigator.clipboard.writeText(exporter);
		delayFunction();
	};

	onMount(() => {
		window.addEventListener("keydown", handleKeyDown);
	});

	onDestroy(() => {
		window.removeEventListener("keydown", handleKeyDown);
	});
</script>

<div
	class="absolute z-50 bg-osvauld-frameblack border border-osvauld-iconblack w-[14rem] rounded-2xl"
	style="top: {top + 10}px; left: {left - 100}px;"
	use:clickOutside
	on:clickOutside="{handleClickOutside}"
>
	<div class="flex flex-col items-start p-2 gap-2 w-full h-full">
		<button
			class="flex items-center p-2 gap-2 w-full h-12 text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-modalFieldActive rounded-lg"
			on:mouseenter="{() => (isUsernameHovered = true)}"
			on:mouseleave="{() => (isUsernameHovered = false)}"
			on:click|preventDefault="{copyUsername}"
		>
			<div class="w-6 h-6 flex items-center justify-center">
				{#if copied && isUsernameHovered}
					<span in:scale>
						<Tick />
					</span>
				{:else}
					<CopyIcon color="{isUsernameHovered ? '#F2F2F0' : '#85889C'}" />
				{/if}
			</div>
			<div class="font-inter text-base whitespace-nowrap">Copy username</div>
		</button>

		<button
			class="flex items-center p-2 gap-2 w-full h-12 text-osvauld-fieldText hover:text-osvauld-dangerRed hover:bg-osvauld-modalFieldActive rounded-lg"
			on:mouseenter="{() => (isRecoveryHovered = true)}"
			on:mouseleave="{() => (isRecoveryHovered = false)}"
			on:click|preventDefault="{passwordprompter}"
		>
			<div class="w-6 h-6 flex items-center justify-center">
				{#if copied && isRecoveryHovered}
					<span in:scale>
						<Tick />
					</span>
				{:else}
					<CopyIcon color="{isRecoveryHovered ? '#FF6A6A' : '#85889C'}" />
				{/if}
			</div>
			<div class="font-inter text-base whitespace-nowrap">
				Copy recovery data
			</div>
		</button>
	</div>
</div>
