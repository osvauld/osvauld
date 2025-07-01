<script lang="ts">
	import { onMount } from "svelte";
	import { sendMessage } from "@osvauld/password-manager-common";

	let connectionTicket = "";
	let copied = $state(false);
	let isKeyRevealed = $state(false);
	let isLoading = $state(false);
	let certificate = $state("");
	let recoveryString = $state("");
	let showPasswordInputForReveal = $state(false);
	let revealPasswordValue = $state("");
	let revealError = $state("");

	const SAMPLE_IDENTIFICATION_KEY =
		"eyJ1c2VyX3B1YmxpY19rZXkiOiItLS0tLUJFR0lOIFBHUCBQVUJMSUMgS0VZIEJMT0NLLS0tLS1cbkNvbW1lbnQ6IDg0RTggMDU4NiBBRTZFIENBOEUgRDkzOSAgRTQ4QyAxMzg0IDk1OEEgNTQ3MSA2RjMyXG5Db21tZW50OiBqa2sxXG5cbnhqTUVhRFJIUkJZSkt3WUJCQUhhUnc4QkFRZEE1Q3R1MDRwVkI1Z3R6V3A1V2lYUEVRQi8vZTVKdHJLT09zTzBcblhtdHFLRURDd0FzRUh4WUtBSDBGZ21nMFIwUURDd2tIQ1JBVGhKV0tWSEZ2TWtjVUFBQUFBQUFlQUNCellXeDBcblFHNXZkR0YwYVc5dWN5NXpaWEYxYjJsaExYQm5jQzV2Y21mQnZxZVNYRmYvMWpuaVRWNzJjSnA0bVRNaWhrVkZcblZvM1NCRlpsMkN5b1lBTVZDZ2dDbXdFQ0hna1dJUVNFNkFXR3JtN0tqdGs1NUl3VGhKV0tWSEZ2TWdBQUJCb0JcbkFNMEdhdXB5MFJBYkhHZnpjVjBsT1lGNDZJU0hJQlk3eXVUQkt0dG1MK3JjQVFEOEdyWFNyYkFmb3p1OXNMVERcbjVFTkxOV1lKKzNpOFVPMkVBdWZFbGhJeEM4MEVhbXRyTWNMQURnUVRGZ29BZ0FXQ2FEUkhSQU1MQ1FjSkVCT0VcbmxZcFVjVzh5UnhRQUFBQUFBQjRBSUhOaGJIUkFibTkwWVhScGIyNXpMbk5sY1hWdmFXRXRjR2R3TG05eVo5VGlcbjFOeUNObzU4V0lMbnhRSWJQdGd4VDhPYTFWVzRnK2FpRWVwTXJsYlVBeFVLQ0FLWkFRS2JBUUllQ1JZaEJJVG9cbkJZYXVic3FPMlRua2pCT0VsWXBVY1c4eUFBQ1EzUUQvY08yT3VBajRNK1NUWVZ1UHlsOFpsZXNsenA5bkczZ0lcbkIxUDA5UTBpaEhzQS8xMGpDaDJBNlZlVGZjd0kwMGtmZ0VkUURKQmhHdUFyNkJUUG1UTEh4eVlNempNRWFEUkhcblJCWUpLd1lCQkFIYVJ3OEJBUWRBUUNpcHZFcHk5b3ZHNGpoQSs3M1dsR1RyWlFNYU9sSkJqSjBQK0xWRFJsYkNcbndMOEVHQllLQVRFRmdtZzBSMFFKRUJPRWxZcFVjVzh5UnhRQUFBQUFBQjRBSUhOaGJIUkFibTkwWVhScGIyNXpMbk5sY1hWdmFXRXRjR2R3TG05eVo1bVZcbmtyM0FDM2pZNE1MY0s2cXZ2VWdVcDNHeVBtalVaRmtjNi8zRXQvNTlBeFVLQ0FLWkFRS2JBUUllQ1JZaEJGUzVcbng4MENHYU1EYUFMS1BYeTFqdjlBRTRtc0FBQTdCZ0VBdXZRaStGVUQrb3JOMG1hSzlCWXhZckV6UHRaSkJGY2RcbmJuelJPMzJlRmY0QkFOWTZZQ29MaTBiVjRlMXdyMTNIN0RUcEl5QmIza2laajJnNmlHSVBYWG9QempNRWFEUkhcblJCWUpLd1lCQkFIYVJ3OEJBUWRBSCtwM3orR3NnNFRHNHlVWFBQamo0OEhydHVvSHBMVkJOa28wZnJETDZoakNcbndMOEVHQllLQVRFRmdtZzBSMFFKRUh5MWp2OUFFNG1zUnhRQUFBQUFBQjRBSUhOaGJIUkFibTkwWVhScGIyNXpcbkxuTmxjWFZ2YVdFdGNHZHdMbTl5Wnk2d1FrZGp4bHU1SzZ5NDQ0ZUNCcDkzWFk4SklMZnpaNS96VnY0c0dyVDBcbkFwc0N2cUFFR1JZS0FHOEZnbWcwUjBRSkVIeTFqJnNmlHSVBYWG9QempNRWFEUkhcblJCWUpLd1lCQkFIYVJ3OEJBUWRBSCtwM3orR3NnNFRHNHlVWFBQamo0OEhydHVvSHBMVkJOa28wZnJETDZoakNcbndMOEVHQllLQVRFRmdtZzBSMFFKRUh5MWp2OUFFNG1zUnhRQUFBQUFBQjRBSUhOaGJIUkFibTkwWVhScGIyNXpcbkxuTmxjWFZ2YVdFdGNHZHdMbTl5Wnk2d1FrZGp4bHU1SzZ5NDQ0ZUNCcDkzWFk4SklMZnpaNS96VnY0c0dyVDBcbkFwc0N2cUFFR1JZS0FHOEZnbWcwUjBRSkVIeTFqh5UnhRQUFBQUFBQjRBSUhOaGJIUkFibTkwWVhSSs3M1dsR1RyWlFNYU9sSkJqSjBQK0xWRFJsYkNcbndMOEVHQl"; // Sample

	let { identificationKey = SAMPLE_IDENTIFICATION_KEY } = $props<{
		identificationKey?: string;
	}>();

	function displayPasswordPrompt() {
		showPasswordInputForReveal = true;
	}

	async function handleSubmitRevealPassword() {
		if (!revealPasswordValue.trim()) {
			revealError = "Password cannot be empty.";
			return;
		}

		try {
			isLoading = true;

			certificate = await sendMessage("exportCertificate", {
				passphrase: revealPasswordValue,
			});
			await sendMessage("startListening");

			if (!certificate) {
				throw new Error("No certificate received");
			}

			recoveryString = JSON.stringify({
				ticket: connectionTicket,
				certificate: certificate,
			});

			identificationKey = recoveryString;
			isKeyRevealed = true;
			showPasswordInputForReveal = false;
			revealError = "";
		} catch (err: any) {
			revealError =
				err.message ||
				"Failed to export certificate. Please check your password and try again.";
		} finally {
			isLoading = false;
			revealPasswordValue = "";
		}
	}

	async function handleCopy() {
		try {
			await navigator.clipboard.writeText(identificationKey);
			copied = true;
			setTimeout(() => {
				copied = false;
			}, 2000); // Reset message after 2 seconds
		} catch (err) {
			console.error("Failed to copy: ", err);
		}
	}

	onMount(async () => {
		try {
			connectionTicket = await sendMessage("getTicket");
		} catch (err) {
			console.error("Failed to get connection ticket:", err);
			revealError =
				"Failed to initialize device connection. Please refresh and try again.";
		}
	});
</script>

<div class="h-full flex flex-col text-base">
	<!-- Header Section -->
	<div class="border-b border-osvauld-borderColor pb-6 mb-8">
		<h1 class="text-2xl font-semibold text-white mb-2">Add Device</h1>
		<p class="text-osvauld-fieldText text-sm">
			Copy the identification key below and enter it on your new device to
			connect it to your workspace.
		</p>
	</div>

	<!-- Form Section -->
	<div class="flex-1 flex flex-col items-center justify-center">
		<div class="w-full grow flex flex-col items-center justify-center">
			<p class="text-sm text-osvauld-fieldText mb-2">
				Your cryptographically secure unique identification key:
			</p>
			<!-- Key Display Area Container -->
			<div
				class="w-full bg-osvauld-bgDarker border border-osvauld-borderColor rounded-md font-mono break-words mb-4 overflow-y-auto grow relative text-white max-h-[25rem] overflow-y-auto"
				style="min-height: 120px;">
				{#if isKeyRevealed}
					<!-- Revealed Key -->
					<div
						class="p-4 select-all whitespace-pre-wrap"
						role="textbox"
						aria-label="Identification key">
						{identificationKey}
					</div>
				{:else}
					<!-- Hidden State: Blurred Key as background -->
					<div
						class="absolute inset-0 p-4 filter blur-[2px] select-none opacity-60 whitespace-pre-wrap overflow-y-auto">
						{identificationKey}
					</div>

					<!-- Translucent Overlay with Text or Password Input -->
					{#if showPasswordInputForReveal}
						<div
							class="absolute inset-0 flex flex-col items-center justify-center z-10 bg-black/75 p-4">
							{#if isLoading}
								<div
									class="flex flex-col items-center justify-center"
									role="status"
									aria-label="Loading">
									<div
										class="animate-spin rounded-full h-12 w-12 border-b-2 border-osvauld-carolinablue mb-4"
										aria-hidden="true">
									</div>
									<p class="text-osvauld-fieldText">Exporting certificate...</p>
								</div>
							{:else}
								<form
									onsubmit={(e) => {
										e.preventDefault();
										handleSubmitRevealPassword();
									}}
									class="flex flex-col items-center w-full max-w-xs">
									<input
										type="password"
										bind:value={revealPasswordValue}
										autofocus
										placeholder="Enter password to reveal key"
										class="bg-osvauld-bgDarker border border-osvauld-borderColor text-white placeholder:text-osvauld-fieldText/70 rounded-md p-3 mb-3 w-full text-sm"
										aria-label="Password for revealing identification key"
										aria-invalid={!!revealError}
										aria-describedby={revealError
											? "password-error"
											: undefined} />
									{#if revealError}
										<p
											id="password-error"
											class="text-red-500 text-xs mb-2"
											role="alert">
											{revealError}
										</p>
									{/if}
									<div class="flex flex-col w-full gap-2">
										<button
											type="submit"
											class="bg-osvauld-carolinablue text-osvauld-frameblack font-semibold py-2 px-4 rounded-md cursor-pointer focus:outline-none focus:ring-2 focus:ring-osvauld-carolinablue focus:ring-opacity-50 transition-colors text-sm">
											Submit & Reveal
										</button>
										<button
											type="button"
											onclick={() => {
												showPasswordInputForReveal = false;
												revealPasswordValue = "";
												revealError = "";
											}}
											class="text-osvauld-fieldText hover:text-white text-xs underline">
											Cancel
										</button>
									</div>
								</form>
							{/if}
						</div>
					{:else}
						<div
							class="absolute inset-0 flex flex-col items-center justify-center z-10 cursor-pointer bg-black/55 transition-colors hover:bg-black/45"
							onclick={displayPasswordPrompt}
							role="button"
							tabindex="0"
							onkeydown={(e) => e.key === "Enter" && displayPasswordPrompt()}
							aria-label="Click to unlock identification key">
							<svg
								xmlns="http://www.w3.org/2000/svg"
								fill="none"
								viewBox="0 0 24 24"
								stroke-width="1.5"
								stroke="currentColor"
								class="w-10 h-10 text-osvauld-fieldText mb-3"
								aria-hidden="true">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M16.5 10.5V6.75a4.5 4.5 0 1 0-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 0 0 2.25-2.25v-6.75a2.25 2.25 0 0 0-2.25-2.25H6.75a2.25 2.25 0 0 0-2.25 2.25v6.75a2.25 2.25 0 0 0 2.25 2.25Z"
								></path>
							</svg>
							<span
								class="text-osvauld-fieldText italic text-base px-4 text-center"
								>Click to Unlock identification key</span>
						</div>
					{/if}
				{/if}
			</div>

			<button
				type="button"
				onclick={handleCopy}
				disabled={!isKeyRevealed || copied}
				class="w-full max-w-[20rem] bg-osvauld-carolinablue text-osvauld-frameblack disabled:text-white font-semibold mt-6 py-4 px-6 rounded-md cursor-pointer focus:outline-none focus:ring-2 focus:ring-osvauld-carolinablue focus:ring-opacity-50 transition-colors disabled:bg-osvauld-fieldActive disabled:cursor-not-allowed"
				aria-label={copied ? "Copied to clipboard" : "Copy identification key"}>
				{#if copied}
					Copied to clipboard!
				{:else}
					Copy Key
				{/if}
			</button>
		</div>

		<div class="text-xs text-osvauld-fieldText text-center max-w-md mt-4">
			<p>
				Click the unlock icon to reveal your identification key. After entering
				your password and successful verification, you can copy the key to your
				clipboard
			</p>
			<p class="mt-2">
				You can then paste this key into the corresponding field on your new
				device to link it.
			</p>
		</div>
	</div>
</div>

