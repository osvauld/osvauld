<script lang="ts">
	import QRCode from "@castlenine/svelte-qrcode";
	import { invoke } from "@tauri-apps/api/core";
	import { listen } from "@tauri-apps/api/event";
	import { onMount, onDestroy } from "svelte";

	let connectionTicket = "";
	let status = "Ready to connect";
	let error = "";
	let unlistenHandlers: (() => void)[] = [];

	async function setupEventListeners() {
		const unlisten1 = await listen("peer-connected", () => {
			status = "Mobile device trying to connect...";
		});

		const unlisten2 = await listen("sync-complete", () => {
			status = "Mobile device connected successfully";
		});

		const unlisten3 = await listen("peer-down", () => {
			status = "Mobile device disconnected";
		});

		unlistenHandlers = [unlisten1, unlisten2, unlisten3];
	}

	onMount(async () => {
		try {
			await invoke("start_p2p_listener");
			await setupEventListeners();
			connectionTicket = await invoke("get_ticket");
			status = "Ready to connect. Share the ticket with mobile device.";
		} catch (err) {
			error = err.toString();
			status = "Failed to initialize";
		}
	});

	onDestroy(() => {
		unlistenHandlers.forEach((unlisten) => unlisten());
	});

	async function copyTicket() {
		try {
			await navigator.clipboard.writeText(connectionTicket);
			const originalStatus = status;
			status = "Ticket copied!";
			setTimeout(() => {
				status = originalStatus;
			}, 2000);
		} catch (err) {
			error = "Failed to copy ticket";
		}
	}
</script>

<div class="p-4 flex flex-col gap-4">
	<div class="bg-mobile-bgSeconary rounded-lg p-4">
		<h2 class="text-xl mb-2 text-mobile-textPrimary">Receive Connection</h2>
		<p class="text-mobile-textSecondary mb-4">
			Share this ticket with your mobile device to establish connection
		</p>

		{#if error}
			<div class="bg-red-500/10 text-red-500 p-3 rounded-lg mb-4">
				{error}
			</div>
		{/if}

		<div class="flex flex-col gap-3">
			{#if connectionTicket}
				<div class="mx-auto">
					<QRCode data="{JSON.stringify(connectionTicket)}" />
				</div>

				<button
					on:click="{copyTicket}"
					class="w-full bg-osvauld-carolinablue text-mobile-bgPrimary rounded-lg py-3 font-medium">
					Copy Ticket
				</button>
			{:else}
				<div class="text-mobile-textSecondary text-center py-4">
					Generating connection ticket...
				</div>
			{/if}
		</div>
	</div>

	<div class="bg-mobile-bgSeconary rounded-lg p-4">
		<div class="flex items-center gap-2">
			<div class="w-2 h-2 rounded-full bg-mobile-textSecondary"></div>
			<span class="text-mobile-textSecondary">Status: {status}</span>
		</div>
	</div>
</div>
