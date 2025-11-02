<script lang="ts">
	import { uiState } from "../state/ui.svelte";

	function switchToBuilder() {
		uiState.setMode('builder');
	}

	function switchToPublisher() {
		uiState.setMode('publisher');
	}

	function switchToViewer() {
		uiState.setMode('viewer');
	}
</script>

<style>
	.mode-switcher {
		display: flex;
		gap: 0.25rem;
		align-items: center;
		background: #0d0e13;
		padding: 0.25rem;
		border-radius: 8px;
		border: 1px solid #2f303e;
	}

	button {
		padding: 0.5rem 1rem;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: #85889c;
		cursor: pointer;
		font-size: 0.875rem;
		font-weight: 500;
		transition: all 0.2s;
	}

	button.active {
		background: #16171f;
		color: #f2f2f0;
		border: 1px solid #4d4f60;
	}

	button:hover:not(.active) {
		color: #bfc0cc;
		background: #12161c;
	}
</style>

<div class="mode-switcher">
	<button
		class:active={uiState.mode === 'builder'}
		onclick={switchToBuilder}
	>
		Builder
	</button>
	<button
		class:active={uiState.mode === 'publisher'}
		onclick={switchToPublisher}
	>
		Publisher
	</button>
	<button
		class:active={uiState.mode === 'viewer'}
		onclick={switchToViewer}
	>
		Viewer
	</button>
</div>
