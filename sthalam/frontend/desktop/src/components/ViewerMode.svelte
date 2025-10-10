<script lang="ts">
	import AddWebsiteConnectionModal from "./AddWebsiteConnectionModal.svelte";

	let showAddWebsiteModal = $state(false);

	function openAddWebsiteModal() {
		showAddWebsiteModal = true;
	}

	function closeAddWebsiteModal() {
		showAddWebsiteModal = false;
	}
</script>

<style>
	.viewer-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		width: 100%;
		padding: 2rem;
		background: #010409;
	}

	.placeholder {
		text-align: center;
		max-width: 500px;
	}

	h2 {
		font-size: 1.5rem;
		margin-bottom: 1rem;
		color: #c9d1d9;
	}

	p {
		color: #8b949e;
		margin-bottom: 1.5rem;
		line-height: 1.6;
	}

	.empty-state {
		font-size: 3rem;
		margin-bottom: 1rem;
		opacity: 0.3;
	}

	.add-website-btn {
		padding: 0.75rem 1.5rem;
		background: #16171f;
		color: #8A86E5;
		border: 1px solid #8A86E5;
		border-radius: 8px;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	.add-website-btn:hover {
		background: #8A86E5;
		color: #0d0e13;
	}
</style>

<div class="viewer-container">
	<div class="placeholder">
		<div class="empty-state">👀</div>
		<h2>Viewer Mode</h2>
		<p>
			Connect to published websites using their connection strings.
			View and sync websites shared with you.
		</p>
		<button class="add-website-btn" onclick={openAddWebsiteModal}>
			+ Add Website
		</button>
	</div>
</div>

{#if showAddWebsiteModal}
	<AddWebsiteConnectionModal onClose={closeAddWebsiteModal} />
{/if}
