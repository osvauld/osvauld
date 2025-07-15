<script lang="ts">
	// import QRCode from "@castlenine/svelte-qrcode";
	import { invoke } from "@tauri-apps/api/core";
	import { listen } from "@tauri-apps/api/event";
	import { onMount, onDestroy } from "svelte";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	let connectionTicket = "";
	let status = $state("Ready to connect");
	let error = $state("");
	let certificate = "";
	let recoveryString = $state("");
	let unlistenHandlers: (() => void)[] = [];
	let textareaElement = $state();
	let passwordCollected = $state("");
	let isPassphraseSubmitted = $state(false);
	let isLoading = $state(false);

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

	async function initializeConnection() {
		try {
			isLoading = true;
			await invoke("start_p2p_listener");
			console.log("acceptor mounted");
			await setupEventListeners();
			connectionTicket = await sendMessage("getTicket");
			status = "Ready to connect. Share the ticket with mobile device.";
			isLoading = false;
		} catch (err) {
			error = err.toString();
			status = "Failed to initialize";
			isLoading = false;
		}
	}

	async function handlePassphraseSubmit() {
		if (!passwordCollected.trim()) {
			error = "Please enter your passphrase";
			return;
		}

		try {
			isLoading = true;
			error = ""; // Clear any previous errors

			certificate = await sendMessage("exportCertificate", {
				passphrase: passwordCollected,
			});

			recoveryString = JSON.stringify({
				ticket: connectionTicket,
				certificate: certificate,
			});

			isPassphraseSubmitted = true;
			status = "Ready to connect. Share the ticket with mobile device.";

			// Use setTimeout to ensure the textarea is rendered before focusing
			setTimeout(() => {
				if (textareaElement) {
					textareaElement.focus();
				}
			}, 100);

			isLoading = false;
		} catch (err) {
			error = `Failed to export certificate: ${err.toString()}`;
			isLoading = false;
		}
	}

	onMount(async () => {
		await initializeConnection();
	});

	onDestroy(() => {
		unlistenHandlers.forEach((unlisten) => unlisten());
	});

	async function copyTicket() {
		try {
			await navigator.clipboard.writeText(recoveryString);
			const originalStatus = status;
			status = "Connection Ticket copied!";
			setTimeout(() => {
				status = originalStatus;
			}, 2000);
		} catch (err) {
			error = "Failed to copy connection ticket";
		}
	}
</script>

<div class="flex flex-col justify-center">
	<div class="bg-mobile-bgSeconary rounded-lg p-4">
		<h2 class="text-xl mb-2 text-mobile-textPrimary">Receive Connection</h2>
		<p class="text-mobile-textSecondary mb-4">
			{#if !isPassphraseSubmitted}
				Enter your passphrase to generate a connection ticket for your mobile
				device
			{:else}
				Share this ticket with your mobile device to establish connection
			{/if}
		</p>

		{#if error}
			<div class="bg-red-500/10 text-red-500 p-3 rounded-lg mb-4">
				{error}
			</div>
		{/if}

		<div class="flex flex-col gap-3">
			{#if !isPassphraseSubmitted}
				<!-- Passphrase input form -->
				<div class="w-full">
					<label for="passphrase" class="block text-mobile-textSecondary mb-1"
						>Passphrase</label>
					<input
						type="password"
						id="passphrase"
						placeholder="Enter your passphrase"
						bind:value={passwordCollected}
						class="w-full bg-mobile-bgPrimary border border-mobile-borderColor rounded-lg p-2 text-mobile-textPrimary" />
				</div>

				<button
					on:click={handlePassphraseSubmit}
					disabled={isLoading}
					class="w-full bg-osvauld-carolinablue text-mobile-bgPrimary rounded-lg py-3 font-medium mt-2">
					{isLoading ? "Processing..." : "Submit Passphrase"}
				</button>
			{:else if recoveryString}
				<div class="mx-auto">
					<!-- <QRCode data="{recoveryString}" /> -->
				</div>
				<textarea
					name="text"
					class="font-light text-xs text-white w-full h-32 p-2 mt-4 overflow-auto break-all"
					bind:this={textareaElement}>
					{recoveryString}
				</textarea>

				<button
					on:click={copyTicket}
					class="w-full bg-osvauld-carolinablue text-mobile-bgPrimary rounded-lg py-3 font-medium">
					Copy Ticket
				</button>
			{:else}
				<div class="text-mobile-textSecondary text-center py-4">
					{isLoading
						? "Generating connection ticket..."
						: "Failed to generate ticket. Please try again."}
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
