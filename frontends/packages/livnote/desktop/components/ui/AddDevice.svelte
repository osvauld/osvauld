<script lang="ts">
	const DEFAULT_IDENTIFICATION_KEY = 'eyJ1c2VyX3B1YmxpY19rZXkiOiItLS0tLUJFR0lOIFBHUCBQVUJMSUMgS0VZIEJMT0NLLS0tLS1cbkNvbW1lbnQ6IDg0RTggMDU4NiBBRTZFIENBOEUgRDkzOSAgRTQ4QyAxMzg0IDk1OEEgNTQ3MSA2RjMyXG5Db21tZW50OiBqa2sxXG5cbnhqTUVhRFJIUkJZSkt3WUJCQUhhUnc4QkFRZEE1Q3R1MDRwVkI1Z3R6V3A1V2lYUEVRQi8vZTVKdHJLT09zTzBcblhtdHFLRURDd0FzRUh4WUtBSDBGZ21nMFIwUURDd2tIQ1JBVGhKV0tWSEZ2TWtjVUFBQUFBQUFlQUNCellXeDBcblFHNXZkR0YwYVc5dWN5NXpaWEYxYjJsaExYQm5jQzV2Y21mQnZxZVNYRmYvMWpuaVRWNzJjSnA0bVRNaWhrVkZcblZvM1NCRlpsMkN5b1lBTVZDZ2dDbXdFQ0hna1dJUVNFNkFXR3JtN0tqdGs1NUl3VGhKV0tWSEZ2TWdBQUJCb0JcbkFNMEdhdXB5MFJBYkhHZnpjVjBsT1lGNDZJU0hJQlk3eXVUQkt0dG1MK3JjQVFEOEdyWFNyYkFmb3p1OXNMVERcbjVFTkxOV1lKKzNpOFVPMkVBdWZFbGhJeEM4MEVhbXRyTWNMQURnUVRGZ29BZ0FXQ2FEUkhSQU1MQ1FjSkVCT0VcbmxZcFVjVzh5UnhRQUFBQUFBQjRBSUhOaGJIUkFibTkwWVhScGIyNXpMbk5sY1hWdmFXRXRjR2R3TG05eVo5VGlcbjFOeUNObzU4V0lMbnhRSWJQdGd4VDhPYTFWVzRnK2FpRWVwTXJsYlVBeFVLQ0FLWkFRS2JBUUllQ1JZaEJJVG9cbkJZYXVic3FPMlRua2pCT0VsWXBVY1c4eUFBQ1EzUUQvY08yT3VBajRNK1NUWVZ1UHlsOFpsZXNsenA5bkczZ0lcbkIxUDA5UTBpaEhzQS8xMGpDaDJBNlZlVGZjd0kwMGtmZ0VkUURKQmhHdUFyNkJUUG1UTEh4eVlNempNRWFEUkhcblJCWUpLd1lCQkFIYVJ3OEJBUWRBUUNpcHZFcHk5b3ZHNGpoQSs3M1dsR1RyWlFNYU9sSkJqSjBQK0xWRFJsYkNcbndMOEVHQllLQVRFRmdtZzBSMFFKRUJPRWxZcFVjVzh5UnhRQUFBQUFBQjRBSUhOaGJIUkFibTkwWVhScGIyNXpMbk5sY1hWdmFXRXRjR2R3TG05eVo1bVZcbmtyM0FDM2pZNE1MY0s2cXZ2VWdVcDNHeVBtalVaRmtjNi8zRXQvNTlBeFVLQ0FLWkFRS2JBUUllQ1JZaEJGUzVcbng4MENHYU1EYUFMS1BYeTFqdjlBRTRtc0FBQTdCZ0VBdXZRaStGVUQrb3JOMG1hSzlCWXhZckV6UHRaSkJGY2RcbmJuelJPMzJlRmY0QkFOWTZZQ29MaTBiVjRlMXdyMTNIN0RUcEl5QmIza2laajJnNmlHSVBYWG9QempNRWFEUkhcblJCWUpLd1lCQkFIYVJ3OEJBUWRBSCtwM3orR3NnNFRHNHlVWFBQamo0OEhydHVvSHBMVkJOa28wZnJETDZoakNcbndMOEVHQllLQVRFRmdtZzBSMFFKRUh5MWp2OUFFNG1zUnhRQUFBQUFBQjRBSUhOaGJIUkFibTkwWVhScGIyNXpcbkxuTmxjWFZ2YVdFdGNHZHdMbTl5Wnk2d1FrZGp4bHU1SzZ5NDQ0ZUNCcDkzWFk4SklMZnpaNS96VnY0c0dyVDBcbkFwc0N2cUFFR1JZS0FHOEZnbWcwUjBRSkVIeTFqdjlBRTRtc1J4UUFBQUFBQUI0QUlITmhiSFJBYm05MFlYUnBcbmIyNXpMbk5sY1hWdmFXRXRjR2R3TG05eVo2NG4wQ3M2OHJDZTU5cmVOWHpNVW5Wb2hwVmJNaGd2aVlyaFBEYUVcbkZYUnFGaUVFdkVwZlU5VkE5UlI1UXpTUWpaOWhCbEE1Z3VZQUFQN0RBUDQ1KzQrY2dPRlA4TWJhTU1GTzNPNDFcblN5MlZONk8vM2lvcmN5dEUranp2NUFEL1l2a3hvYnVLWmRHa0wvdldEaSt6d2U3VUhKQTIycGIrSkFaQlRNYnhcbklna1dJUVJVdWNmTkFobWpBMmdDeWoxOHRZNy9RQk9KckFBQU9xVUEvM3FjVUd6Lzd6OGF3M0o3MEN6cmoydERcbldDd0xlZDROOWdaOVFyVDRRNEw5QVFDSTlzR3Z4TjRXcmMvUDJxY0VxektQY0M5WU03SHFlTWZudTVRR3R1ckNcbkNjNDRCR2cwUjBRU0Npc0dBUVFCbDFVQkJRRUJCMEE4TFNUS0IycS9ObUprNkdXSHNRUWc2Q0ZpWmFGNjFwVTFcbmVKY2IwVG9VWXdNQkNBZkN3QUFFR0JZS0FISUZnbWcwUjBRSkVIeTFqdjlBRTRtc1J4UUFBQUFBQUI0QUlITmhiSFJBYm05MFlYUnBcbmIyNXpMbk5sY1hWdmFXRXRjR2R3TG05eVorMjVxTytuL3hCbFc0dm5TcVI2UVRPOCtwUHdcbmZyR1phWVF1Ym84OG1ZY3BBcHNJRmlFRVZMbkh6UUlab3dOb0FzbzlmTFdPLzBBVGlhd0FBT0RMQVA5Q1A1RVJcblZEMjZHenB6aDBhdXJGSHRVRGg5ZjBQK01nQ0pSVmV2MnA0YlFnRUE3SzJMb29TL09WVmdTaklwbzJINXV0TDJcbjU4M2paRXRqUFJteUJaaWx2QWc9XG49TjBOVVxuLS'; // Default key

	let { identificationKey = DEFAULT_IDENTIFICATION_KEY } = $props<{ identificationKey?: string }>();
	let copied = $state(false);
	let isKeyRevealed = $state(false);

	function revealKey() {
		isKeyRevealed = true;
	}

	async function handleCopy() {
		// Here you would typically trigger your password modal
		// For demonstration, we'll assume password validation is successful
		// and then proceed to copy.

		// const passwordIsValid = await showPasswordModal(); // Your actual password modal logic
		// if (!passwordIsValid) {
		// return;
		// }

		try {
			await navigator.clipboard.writeText(identificationKey);
			copied = true;
			setTimeout(() => {
				copied = false;
			}, 2000); // Reset message after 2 seconds
		} catch (err) {
			console.error('Failed to copy: ', err);
			// Handle error (e.g., show an error message to the user)
		}
	}
</script>

<div class="h-full flex flex-col text-base">
	<!-- Header Section -->
	<div class="border-b border-osvauld-borderColor pb-6 mb-8">
		<h1 class="text-2xl font-semibold text-white mb-2">Add Device</h1>
		<p class="text-osvauld-fieldText text-sm">
			Copy the identification key below and enter it on your new device to connect it to your
			workspace.
		</p>
	</div>

	<!-- Form Section -->
	<div class="flex-1 flex flex-col items-center justify-center">
		<div class="w-full grow flex flex-col items-center justify-center">
			<p class="text-sm text-osvauld-fieldText mb-2">Your unique identification key:</p>
			<!-- Key Display Area Container -->
			<div
				class="w-full bg-osvauld-bgDarker border border-osvauld-borderColor rounded-md font-mono break-words mb-4 overflow-y-auto grow relative text-white max-h-[25rem] overflow-y-auto"
				style="min-height: 120px;"
			>
				{#if isKeyRevealed}
					<!-- Revealed Key -->
					<div class="p-4 select-all whitespace-pre-wrap">
						{identificationKey}
					</div>
				{:else}
					<!-- Hidden State: Blurred Key as background -->
					<div
						class="absolute inset-0 p-4 filter blur-[2px] select-none opacity-60 whitespace-pre-wrap overflow-y-auto"
					>
						{identificationKey}
					</div>

					<!-- Translucent Overlay with Text -->
					<div
						class="absolute inset-0  flex flex-col items-center justify-center z-10 cursor-pointer bg-black/55 transition-colors"
						onclick={revealKey}
						role="button"
						tabindex="0"
						onkeydown={(e) => e.key === 'Enter' && revealKey()}
					>
						<!-- Lock Icon -->
						<svg
							xmlns="http://www.w3.org/2000/svg"
							fill="none"
							viewBox="0 0 24 24"
							stroke-width="1.5"
							stroke="currentColor"
							class="w-10 h-10 text-osvauld-fieldText mb-3"
						>
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								d="M16.5 10.5V6.75a4.5 4.5 0 1 0-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 0 0 2.25-2.25v-6.75a2.25 2.25 0 0 0-2.25-2.25H6.75a2.25 2.25 0 0 0-2.25 2.25v6.75a2.25 2.25 0 0 0 2.25 2.25Z"
							/>
						</svg>
						<span class="text-osvauld-fieldText italic text-base px-4 text-center"
							>Click to Unlock identification key</span
						>
					
					</div>
				{/if}
			</div>

			<button
				type="button"
				onclick={handleCopy}
				disabled={!isKeyRevealed || copied}
				class="w-full max-w-[20rem] bg-osvauld-carolinablue text-osvauld-frameblack disabled:text-white font-semibold mt-6 py-4 px-6 rounded-md  cursor-pointer focus:outline-none focus:ring-2 focus:ring-osvauld-carolinablue focus:ring-opacity-50 transition-colors disabled:bg-osvauld-fieldActive disabled:cursor-not-allowed"
			>
				{#if copied}
						Copied to clipboard!
				{:else}
					  Copy Key
				{/if}
			</button>
		
		</div>

		<div class="text-xs text-osvauld-fieldText text-center max-w-md mt-4">
			<p>
				Clicking "Copy Key" will prompt for your password. Upon successful verification, the key
				will be copied to your clipboard.
			</p>
			<p class="mt-2">
				You can then paste this key into the corresponding field on your new device to link it.
			</p>
		</div>
	</div>
</div>