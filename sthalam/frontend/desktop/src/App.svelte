<script lang="ts">
	import { onMount } from "svelte";
	import WebsiteBuilder from "./lib/WebsiteBuilder.svelte";
	import { editorStore } from "./store.svelte";

	let showDebugInfo = $state(false);

	const storeStatus = $derived(editorStore.doc ? "✅ Ready" : "❌ Not ready");

	onMount(() => {
		console.log("App mounted");
	});

	function toggleDebugInfo() {
		showDebugInfo = !showDebugInfo;
	}

	function clearDocument() {
		if (confirm("Are you sure you want to clear the document?")) {
			editorStore.clear();
		}
	}
</script>

<style>
	main {
		display: flex;
		flex-direction: column;
		height: 100vh;
		width: 100%;
	}

	header {
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
		padding: 1rem 2rem;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
	}

	.header-content {
		display: flex;
		justify-content: space-between;
		align-items: center;
		max-width: 1400px;
		margin: 0 auto;
	}

	h1 {
		margin: 0;
		font-size: 1.75rem;
		font-weight: 600;
	}

	.actions {
		display: flex;
		gap: 0.5rem;
	}

	button {
		padding: 0.5rem 1rem;
		border: none;
		border-radius: 6px;
		background: rgba(255, 255, 255, 0.2);
		color: white;
		cursor: pointer;
		font-size: 0.875rem;
		font-weight: 500;
		transition: background 0.2s;
	}

	button:hover {
		background: rgba(255, 255, 255, 0.3);
	}

	.content {
		flex: 1;
		display: flex;
		overflow: hidden;
		position: relative;
	}

	.debug-panel {
		position: absolute;
		top: 1rem;
		right: 1rem;
		background: white;
		border: 1px solid #e0e0e0;
		border-radius: 8px;
		padding: 1rem;
		width: 250px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
		z-index: 1000;
	}

	.debug-panel h3 {
		margin: 0 0 0.75rem 0;
		font-size: 1rem;
		color: #333;
	}

	.debug-content p {
		margin: 0.5rem 0;
		font-size: 0.875rem;
		color: #666;
	}
</style>

<main>
	<header>
		<div class="header-content">
			<h1>🎨 Website Builder</h1>
			<div class="actions">
				<button onclick={toggleDebugInfo}>
					{showDebugInfo ? "Hide" : "Show"} Debug
				</button>
				<button onclick={clearDocument}>Clear Canvas</button>
			</div>
		</div>
	</header>

	<div class="content">
		<WebsiteBuilder />

		{#if showDebugInfo}
			<aside class="debug-panel">
				<h3>Debug Info</h3>
				<div class="debug-content">
					<p><strong>Store:</strong> {storeStatus}</p>
				</div>
			</aside>
		{/if}
	</div>
</main>
