<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { onMount, onDestroy } from "svelte";
	import { listen } from "@tauri-apps/api/event";
	import { wsConnector } from "../../store/desktop.ui.store";

	let ticket = "";
	let status = "Ready to connect";
	let error = "";
	let connecting = false;
	let scanning = false;
	let unlistenHandlers: (() => void)[] = [];

	async function setupEventListeners() {
		const unlisten1 = await listen("peer-connected", () => {
			status = "Establishing secure connection...";
		});
		const unlisten2 = await listen("sync-complete", () => {
			status = "Connected successfully";
			connecting = false;
		});
		const unlisten3 = await listen("peer-down", () => {
			status = "Disconnected from desktop";
		});
		unlistenHandlers = [unlisten1, unlisten2, unlisten3];
	}

	onMount(setupEventListeners);
	onDestroy(() => {
		unlistenHandlers.forEach((unlisten) => unlisten());
	});

	async function connect() {
		console.log(ticket);
		if (!ticket.trim()) {
			error = "Please enter a connection ticket";
			return;
		}
		try {
			status = "Initiating connection...";
			error = "";
			connecting = true;
			//TODO: handle live and sync connection
			const response = await $wsConnector.sendConnectionStringRequest(ticket);
			console.log(response);
			await invoke("connect_with_device", { ticket: response.trim() });
		} catch (err) {
			error = err.toString();
			status = "Connection failed";
			connecting = false;
		}
	}

	async function pasteTicket() {
		try {
			const text = await navigator.clipboard.readText();
			ticket = text;
			error = "";
		} catch (err) {
			error = "Failed to paste from clipboard";
		}
	}
</script>

<div class="items-center flex flex-col justify-center">
	<div class="p-4 flex flex-col gap-4">
		<div class="bg-mobile-bgSeconary rounded-lg p-4">
			<h2 class="text-xl mb-2 text-mobile-textPrimary">
				Connect to your device
			</h2>
			<p class="text-mobile-textSecondary mb-4">
				Enter the connection ticket from your device or scan QR code to
				establish connection
			</p>
			{#if error}
				<div class="bg-red-500/10 text-red-500 p-3 rounded-lg mb-4">
					{error}
				</div>
			{/if}
			<div class="flex flex-col gap-3">
				<input
					type="text"
					bind:value={ticket}
					placeholder="Enter connection ticket"
					class="w-full bg-mobile-bgPrimary border rounded-lg text-mobile-textPrimary border-mobile-bgHighlight p-3 focus:border-mobile-borderActive focus:ring-0" />
				<div class="flex gap-2">
					<button
						on:click={pasteTicket}
						class="px-4 py-2.5 bg-mobile-bgHighlight text-mobile-textPrimary rounded-lg font-medium"
						disabled={connecting || scanning}>
						Paste
					</button>
					<button
						on:click={connect}
						class="flex-1 px-4 py-2.5 bg-osvauld-carolinablue text-mobile-bgPrimary rounded-lg font-medium"
						disabled={connecting || scanning || !ticket.trim()}>
						{connecting ? "Connecting..." : "Connect"}
					</button>
				</div>
			</div>
		</div>
		<div class="bg-mobile-bgSeconary rounded-lg p-4">
			<div class="flex items-center gap-2">
				<div class="w-2 h-2 rounded-full bg-mobile-textSecondary"></div>
				<span class="text-mobile-textSecondary">Status: {status}</span>
			</div>
		</div>
	</div>
</div>
