<script>
	export let size = 24;
	export let color = "#000";
	export let duration = 1;
</script>

<style>
	.loader {
		width: 100%;
		height: 100%;
		display: flex;
		justify-content: center;
		align-items: center;
	}

	.spinner {
		width: 100%;
		height: 100%;
		border-radius: 50%;
		border: 2px solid transparent;
		border-top-color: var(--color);
		animation: spin var(--duration) linear infinite;
	}

	@keyframes spin {
		0% {
			transform: rotate(0deg);
		}
		100% {
			transform: rotate(360deg);
		}
	}
</style>

<div
	class="loader"
	style="--color: {color}; --duration: {duration}s; width: {size}px; height: {size}px;"
>
	<div class="spinner"></div>
</div>
