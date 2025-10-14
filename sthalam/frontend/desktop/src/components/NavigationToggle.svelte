<script lang="ts">
	import { uiState } from "../state";

	function toggleNavigation() {
		uiState.toggleNavigationPanel();
	}
</script>

<button class="nav-toggle" onclick={toggleNavigation} title="Toggle Navigation Panel">
	{#if uiState.showNavigationPanel}
		<svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path d="M15 5L5 15M5 5L15 15" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
		</svg>
	{:else}
		<svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path d="M3 5H17M3 10H17M3 15H17" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
		</svg>
	{/if}
</button>

<style>
	.nav-toggle {
		width: 36px;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: #0d0e13;
		border: 1px solid #2f303e;
		border-radius: 6px;
		color: #a3a4b5;
		cursor: pointer;
		transition: all 0.2s;
	}

	.nav-toggle:hover {
		background: #16171f;
		border-color: #4d4f60;
		color: #f2f2f0;
	}

	.nav-toggle:active {
		transform: scale(0.95);
	}
</style>
