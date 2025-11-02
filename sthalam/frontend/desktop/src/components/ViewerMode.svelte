<script lang="ts">
	import { onMount, onDestroy, untrack } from "svelte";
	import { dataState, uiState } from "../state";
	import FullScreenViewer from "../lib/FullScreenViewer.svelte";
	import SubmissionsViewer from "./SubmissionsViewer.svelte";
	import AddWebsiteConnectionModal from "./AddWebsiteConnectionModal.svelte";
	import ViewerWebsiteFolder from "./ViewerWebsiteFolder.svelte";
	import ModeSwitcher from "./ModeSwitcher.svelte";
	import NavigationToggle from "./NavigationToggle.svelte";
	import type { YjsDocuments } from "../lib/yjsManager";
	import type { Website } from "../types";
	import { sendMessage } from "../utils/helper";
	import type { TemplateStructureStore } from "../lib/templateStructureStore";

	let yDocs: YjsDocuments | null = null;
	let blocks = $state<Map<string, any>>(new Map());
	let showAddWebsiteModal = $state(false);
	let isSyncing = $state(false);
	let viewMode = $state<'content' | 'submissions'>('content');
	let templateStructureStore: TemplateStructureStore | null = null;
	let blocksuiteUnsubscribe: (() => void) | null = null;

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
			viewMode = 'content'; // Reset to content view
			return;
		}

		// Resource selected - reset to content view and get fresh documents
		console.log("✅ Resource selected - setting up workspace");
		viewMode = 'content'; // Reset to content view when switching resources

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

			// Get TemplateStructureStore and subscribe to it
			const store = coordinator.getTemplateStructureStore();
			if (store) {
				// Clean up previous subscription
				if (blocksuiteUnsubscribe) {
					blocksuiteUnsubscribe();
				}

				templateStructureStore = store;

				// Subscribe to blocks changes via store
				blocksuiteUnsubscribe = templateStructureStore.subscribe(() => {
					blocks = templateStructureStore!.getAllBlocks();
				});

				// Initial load from store
				blocks = templateStructureStore.getAllBlocks();
				console.log("✅ [ViewerMode] Subscribed to TemplateStructureStore, got", blocks.size, "blocks");
			}
		});

		// Cleanup function for this effect
		return () => {
			// Unsubscribe from TemplateStructureStore
			if (blocksuiteUnsubscribe) {
				blocksuiteUnsubscribe();
				blocksuiteUnsubscribe = null;
			}
		};
	});

	onDestroy(() => {
		// Unsubscribe from TemplateStructureStore
		if (blocksuiteUnsubscribe) {
			blocksuiteUnsubscribe();
		}

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

	async function handleRefresh() {
		const resourceId = dataState.currentResourceId;
		if (!resourceId) {
			console.warn('No resource selected to refresh');
			return;
		}

		isSyncing = true;
		try {
			await sendMessage('syncResource', { resourceId });
			console.log('Successfully triggered sync for resource:', resourceId);
		} catch (error) {
			console.error('Failed to sync resource:', error);
		} finally {
			isSyncing = false;
		}
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
		{#if uiState.showNavigationPanel}
			<div class="sidebar">
				<div class="mode-switcher-wrapper">
					<div style="flex: 1;">
						<ModeSwitcher />
					</div>
					<NavigationToggle />
				</div>
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
		{/if}
		{#if !uiState.showNavigationPanel}
			<div class="floating-toggle">
				<NavigationToggle />
			</div>
		{/if}
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
		{#if uiState.showNavigationPanel}
			<div class="sidebar">
				<div class="mode-switcher-wrapper">
					<div style="flex: 1;">
						<ModeSwitcher />
					</div>
					<NavigationToggle />
				</div>
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
		{/if}
		{#if !uiState.showNavigationPanel}
			<div class="floating-toggle">
				<NavigationToggle />
			</div>
		{/if}
		<div class="viewer-main">
			<div class="viewer-controls">
				<div class="view-mode-toggle">
					<button
						class="toggle-btn {viewMode === 'content' ? 'active' : ''}"
						onclick={() => viewMode = 'content'}
					>
						<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
							<path d="M9 2a1 1 0 000 2h2a1 1 0 100-2H9z" />
							<path fill-rule="evenodd" d="M4 5a2 2 0 012-2 3 3 0 003 3h2a3 3 0 003-3 2 2 0 012 2v11a2 2 0 01-2 2H6a2 2 0 01-2-2V5zm3 4a1 1 0 000 2h.01a1 1 0 100-2H7zm3 0a1 1 0 000 2h3a1 1 0 100-2h-3zm-3 4a1 1 0 100 2h.01a1 1 0 100-2H7zm3 0a1 1 0 100 2h3a1 1 0 100-2h-3z" clip-rule="evenodd" />
						</svg>
						Content
					</button>
					<button
						class="toggle-btn {viewMode === 'submissions' ? 'active' : ''}"
						onclick={() => viewMode = 'submissions'}
					>
						<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
							<path fill-rule="evenodd" d="M6 2a2 2 0 00-2 2v12a2 2 0 002 2h8a2 2 0 002-2V7.414A2 2 0 0015.414 6L12 2.586A2 2 0 0010.586 2H6zm5 6a1 1 0 10-2 0v3.586l-1.293-1.293a1 1 0 10-1.414 1.414l3 3a1 1 0 001.414 0l3-3a1 1 0 00-1.414-1.414L11 11.586V8z" clip-rule="evenodd" />
						</svg>
						Submissions
					</button>
				</div>
				<button
					class="refresh-button"
					onclick={handleRefresh}
					disabled={isSyncing}
					title="Refresh and sync latest changes"
				>
					{#if isSyncing}
						<span class="refresh-spinner"></span>
					{:else}
						↻
					{/if}
				</button>
			</div>

			<div class="viewer-content">
				{#if viewMode === 'content'}
					<FullScreenViewer {blocks} ydoc={yDocs?.blocksuiteDoc} commentsDoc={yDocs?.commentsDoc} submissionsDoc={yDocs?.submissionsDoc} />
				{:else}
					<SubmissionsViewer />
				{/if}
			</div>
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

	.mode-switcher-wrapper {
		padding: 0.5rem;
		border-bottom: 1px solid #292a36;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.floating-toggle {
		position: absolute;
		top: 1rem;
		left: 1rem;
		z-index: 1000;
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
		position: relative;
		display: flex;
		flex-direction: column;
	}

	.viewer-controls {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 1rem;
		border-bottom: 1px solid #292a36;
		background: #010409;
		flex-shrink: 0;
	}

	.view-mode-toggle {
		display: flex;
		gap: 0.5rem;
		background: #16171f;
		border-radius: 8px;
		padding: 0.25rem;
		border: 1px solid #292a36;
	}

	.toggle-btn {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 1rem;
		background: transparent;
		color: #8b949e;
		border: none;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	.toggle-btn:hover {
		color: #c9d1d9;
		background: rgba(138, 134, 229, 0.1);
	}

	.toggle-btn.active {
		background: #8a86e5;
		color: #0d0e13;
	}

	.toggle-btn svg {
		width: 1rem;
		height: 1rem;
	}

	.refresh-button {
		width: 2.5rem;
		height: 2.5rem;
		border-radius: 50%;
		background: #16171f;
		border: 1px solid #8A86E5;
		color: #8A86E5;
		font-size: 1.5rem;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all 0.2s;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
	}

	.refresh-button:hover:not(:disabled) {
		background: #8A86E5;
		color: #16171f;
		transform: rotate(180deg) scale(1.1);
	}

	.refresh-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.viewer-content {
		flex: 1;
		overflow: hidden;
		position: relative;
	}

	.refresh-spinner {
		width: 16px;
		height: 16px;
		border: 2px solid rgba(138, 134, 229, 0.3);
		border-top-color: #8A86E5;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
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
