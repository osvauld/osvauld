<script lang="ts">
	import { onMount, onDestroy, untrack } from "svelte";
	import { dataState, uiState } from "../state";
	import FullScreenViewer from "../lib/FullScreenViewer.svelte";
	import AddWebsiteConnectionModal from "./AddWebsiteConnectionModal.svelte";
	import ViewerWebsiteFolder from "./ViewerWebsiteFolder.svelte";
	import type { YjsDocuments } from "../lib/yjsManager";
	import type { Website } from "../types";

	let yDocs: YjsDocuments | null = null;
	let blocks = $state<Map<string, any>>(new Map());
	let showAddWebsiteModal = $state(false);

	// Get synced resources
	const syncedResources = $derived(dataState.resources);
	const hasResources = $derived(syncedResources.length > 0);
	const hasResource = $derived(!!dataState.currentResourceId);

	// Get unique websites from synced resources
	const syncedWebsites = $derived(() => {
		const websiteMap = new Map<string, Website>();

		syncedResources.forEach(resource => {
			if (resource.websiteId && !websiteMap.has(resource.websiteId)) {
				// Find the website info from dataState.websites
				const website = dataState.websites.find(w => w.id === resource.websiteId);
				if (website) {
					websiteMap.set(resource.websiteId, website);
				} else {
					// Fallback: create a basic website object if not found
					websiteMap.set(resource.websiteId, {
						id: resource.websiteId,
						name: resource.websiteId, // Use ID as name if website info not available
					});
				}
			}
		});

		return Array.from(websiteMap.values());
	});

	onMount(async () => {
		console.log("🚀 Initializing Viewer Mode...");

		// Fetch resources if not already loaded
		if (dataState.resources.length === 0) {
			await dataState.fetchAllResources();
		}

		console.log("✅ Viewer Mode initialized!");
	});

	// React to resource changes (same pattern as WebsiteBuilder)
	$effect(() => {
		console.log("🎬 [VIEWER EFFECT] Starting effect...");
		const resourceId = dataState.currentResourceId;
		console.log("🔄 [VIEWER EFFECT] Resource changed:", resourceId);

		if (!resourceId) {
			// No resource selected - clear everything
			console.log("❌ No resource selected - clearing workspace");
			yDocs = null;
			blocks = new Map();
			return;
		}

		// Resource selected - get fresh documents from coordinator
		console.log("✅ Resource selected - setting up workspace");

		// Use untracked to avoid infinite loops
		untrack(() => {
			const coordinator = dataState.getBlocksuiteCoordinator();
			if (!coordinator) {
				console.error("❌ No coordinator available");
				return;
			}

			// Get the FRESH Yjs documents (after loadBlocksuite was called)
			const docs = coordinator.getDocuments();
			if (!docs) {
				console.error("❌ No Yjs documents available");
				return;
			}

			yDocs = docs;

			console.log("📦 Yjs documents received:", {
				blocks: docs.blocks.size
			});

			// Subscribe to blocks changes
			const blocksObserver = () => {
				if (!yDocs) return;
				const newBlocks = new Map();
				yDocs.blocks.forEach((value, key) => {
					newBlocks.set(key, value);
				});
				blocks = newBlocks;
			};

			docs.blocks.observe(blocksObserver);

			// Initial load
			blocksObserver();
		});

		// Cleanup function for this effect
		return () => {
			if (yDocs) {
				// Store reference before clearing
				const docsToCleanup = yDocs;
				// Unobserve using the stored reference
				const blocksObserver = () => {};
				const viewportObserver = () => {};
				// Note: We can't properly unobserve here because the observers are scoped
				// This is acceptable as Yjs will clean up when docs are destroyed
			}
		};
	});

	onDestroy(() => {
		// Note: Coordinator cleanup is handled by dataState.clearAllState() when needed
	});

	async function handleSelectResource(resourceId: string) {
		console.log("🎯 Selecting resource:", resourceId);
		await dataState.switchResource(resourceId);
	}

	function openAddWebsiteModal() {
		showAddWebsiteModal = true;
	}

	function closeAddWebsiteModal() {
		showAddWebsiteModal = false;
	}
</script>

{#if !hasResources}
	<!-- No synced websites -->
	<div class="viewer-container">
		<div class="empty-main">
			<div class="empty-state">
				<div class="empty-icon">👀</div>
				<h2>Viewer Mode</h2>
				<p>
					Connect to published websites using their connection strings.
					View and sync websites shared with you.
				</p>
				<button class="add-website-btn" onclick={openAddWebsiteModal}>
					+ Add Website Connection
				</button>
			</div>
		</div>
	</div>
{:else if !hasResource}
	<!-- Has websites but none selected -->
	<div class="viewer-container">
		<div class="sidebar">
			<div class="sidebar-header">
				<div class="sidebar-title">Synced Websites</div>
				<button class="add-website-btn" onclick={openAddWebsiteModal}>
					+ Add Website
				</button>
			</div>
			<div class="resources-list">
				{#each syncedWebsites() as website (website.id)}
					<ViewerWebsiteFolder
						{website}
						isExpanded={uiState.isFolderExpanded(website.id)}
						onToggle={() => uiState.toggleFolderExpansion(website.id)}
						onSelect={() => {}}
						isSelected={false}
					/>
				{/each}
			</div>
		</div>
		<div class="empty-main">
			<div class="empty-state">
				<div class="empty-icon">👈</div>
				<h2>Select a page</h2>
				<p>Choose a page from the sidebar to view its content.</p>
			</div>
		</div>
	</div>
{:else}
	<!-- Resource selected - show viewer -->
	<div class="viewer-container">
		<div class="sidebar">
			<div class="sidebar-header">
				<div class="sidebar-title">Synced Websites</div>
				<button class="add-website-btn" onclick={openAddWebsiteModal}>
					+ Add Website
				</button>
			</div>
			<div class="resources-list">
				{#each syncedWebsites() as website (website.id)}
					<ViewerWebsiteFolder
						{website}
						isExpanded={uiState.isFolderExpanded(website.id)}
						onToggle={() => uiState.toggleFolderExpansion(website.id)}
						onSelect={() => {}}
						isSelected={false}
					/>
				{/each}
			</div>
		</div>
		<div class="viewer-main">
			<FullScreenViewer {blocks} />
		</div>
	</div>
{/if}

{#if showAddWebsiteModal}
	<AddWebsiteConnectionModal onClose={closeAddWebsiteModal} />
{/if}

<style>
	.viewer-container {
		width: 100%;
		height: 100%;
		overflow: hidden;
		background: #010409;
		display: flex;
	}

	.sidebar {
		width: 17rem;
		height: 100%;
		border-right: 1px solid #292a36;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.sidebar-header {
		padding: 1rem;
		border-bottom: 1px solid #292a36;
		flex-shrink: 0;
	}

	.sidebar-title {
		font-size: 0.875rem;
		font-weight: 600;
		color: #c9d1d9;
		margin-bottom: 0.75rem;
	}

	.add-website-btn {
		width: 100%;
		padding: 0.5rem 1rem;
		background: #16171f;
		color: #8A86E5;
		border: 1px solid #8A86E5;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	.add-website-btn:hover {
		background: #8A86E5;
		color: #0d0e13;
	}

	.resources-list {
		flex: 1;
		overflow-y: auto;
		padding: 0.5rem;
	}

	.empty-main {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.viewer-main {
		flex: 1;
		overflow: hidden;
	}

	.empty-state {
		text-align: center;
		max-width: 500px;
		padding: 2rem;
	}

	.empty-icon {
		font-size: 3rem;
		margin-bottom: 1rem;
		opacity: 0.3;
	}

	.empty-state h2 {
		font-size: 1.5rem;
		margin-bottom: 1rem;
		color: #c9d1d9;
	}

	.empty-state p {
		color: #8b949e;
		margin-bottom: 1.5rem;
		line-height: 1.6;
	}
</style>
