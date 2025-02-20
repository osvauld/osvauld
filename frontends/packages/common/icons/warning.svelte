<!-- Warning.svelte -->
<script>
	export let copiedToClipboard = false;
</script>

<style>
	@keyframes blink {
		0% {
			opacity: 1;
		}
		50% {
			opacity: 0;
		}
		100% {
			opacity: 1;
		}
	}

	.animate-blink {
		animation: blink 1s linear infinite;
	}

	.text-bright-red {
		color: #ff0000;
	}
</style>

<svg
	class="h-6 w-6 {copiedToClipboard
		? 'animate-blink text-bright-red'
		: 'text-osvauld-dangerRed'}"
	fill="#FF6A6A"
	viewBox="0 0 28 29"
	stroke="currentColor"
>
	<path
		fill-rule="evenodd"
		clip-rule="evenodd"
		d="M3.33797 25.0004C2.81297 25.0004 2.34047 24.7262 2.07797 24.2712C1.81547 23.8162 1.80964 23.2737 2.07797 22.8129L12.7355 4.16371C12.9921 3.70871 13.4821 3.42871 14.0013 3.42871C14.5205 3.42871 15.0105 3.70871 15.2671 4.16371L25.9305 22.8187C26.193 23.2737 26.1871 23.822 25.9305 24.277C25.668 24.732 25.1955 25.0062 24.6705 25.0062L24.6646 25.0004H3.33797ZM3.84547 23.2504H24.163L14.0013 5.47038L3.83964 23.2504H3.84547ZM14.8763 17.417V11.0004H13.1263V17.417H14.8763ZM15.168 20.3337C15.168 20.978 14.6456 21.5004 14.0013 21.5004C13.357 21.5004 12.8346 20.978 12.8346 20.3337C12.8346 19.6894 13.357 19.167 14.0013 19.167C14.6456 19.167 15.168 19.6894 15.168 20.3337Z"
	></path>
</svg>
