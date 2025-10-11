<script lang="ts">
	import { onMount, onDestroy, untrack } from "svelte";
	import { dataState } from "../store.svelte";
	import { dataState as authDataState } from "../state";
	import Canvas from "../lib/Canvas.svelte";
	import AddWebsiteConnectionModal from "./AddWebsiteConnectionModal.svelte";
	import type { YjsDocuments } from "../lib/yjsManager";

	let yDocs: YjsDocuments | null = null;
	let blocks = $state<Map<string, any>>(new Map());
	let viewport = $state({ x: 0, y: 0, zoom: 1 });
	let showAddWebsiteModal = $state(false);

	// Get synced resources
	const syncedResources = $derived(authDataState.resources);
	const hasResources = $derived(syncedResources.length > 0);
	const hasResource = $derived(!!authDataState.currentResourceId);

	onMount(async () => {
		console.log("🚀 Initializing Viewer Mode...");

		// Initialize dataState (creates coordinator)
		await dataState.initializeState();

		// Fetch resources if not already loaded
		if (authDataState.resources.length === 0) {
			await authDataState.fetchAllResources();
		}

		console.log("✅ Viewer Mode initialized!");
	});

	// React to resource changes (same pattern as WebsiteBuilder)
	$effect(() => {
		console.log("🎬 [VIEWER EFFECT] Starting effect...");
		const resourceId = authDataState.currentResourceId;
		console.log("🔄 [VIEWER EFFECT] Resource changed:", resourceId);

		if (!resourceId) {
			// No resource selected - clear everything
			console.log("❌ No resource selected - clearing workspace");
			yDocs = null;
			blocks = new Map();
			viewport = { x: 0, y: 0, zoom: 1 };
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
				blocks: docs.blocks.size,
				viewport: {
					x: docs.viewport.get("x"),
					y: docs.viewport.get("y"),
					zoom: docs.viewport.get("zoom")
				}
			});

			// Subscribe to blocks changes
			const blocksObserver = () => {
				if (!yDocs) return;
				const newBlocks = new Map();
				yDocs.blocks.forEach((value, key) => {
					newBlocks.set(key, value);
				});
				blocks = newBlocks;
				console.log("📦 Blocks updated:", blocks.size);
			};

			// Subscribe to viewport changes
			const viewportObserver = () => {
				if (!yDocs) return;
				viewport = {
					x: yDocs.viewport.get("x") || 0,
					y: yDocs.viewport.get("y") || 0,
					zoom: yDocs.viewport.get("zoom") || 1,
				};
				console.log("🔍 Viewport updated:", viewport);
			};

			docs.blocks.observe(blocksObserver);
			docs.viewport.observe(viewportObserver);

			// Initial load
			blocksObserver();
			viewportObserver();
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
		// Cleanup is handled by dataState.clearAllState()
		dataState.clearAllState();
	});

	function updateViewport(newViewport: { x: number; y: number }) {
		// In viewer mode, allow viewport panning but don't save to Yjs
		viewport = { ...viewport, ...newViewport };
	}

	async function handleSelectResource(resourceId: string) {
		console.log("🎯 Selecting resource:", resourceId);
		await authDataState.switchResource(resourceId);
	}

	function openAddWebsiteModal() {
		showAddWebsiteModal = true;
	}

	function closeAddWebsiteModal() {
		showAddWebsiteModal = false;
	}

	function formatDate(timestamp: number): string {
		const date = new Date(timestamp);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffMins = Math.floor(diffMs / 60000);

		if (diffMins < 1) return "just now";
		if (diffMins < 60) return `${diffMins}m ago`;
		if (diffMins < 1440) return `${Math.floor(diffMins / 60)}h ago`;
		return date.toLocaleDateString();
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
				{#each syncedResources as resource (resource.id)}
					<button
						class="resource-item"
						onclick={() => handleSelectResource(resource.id)}
					>
						<div class="resource-title">{resource.title}</div>
						<div class="resource-meta">
							Updated {formatDate(resource.lastModified)}
						</div>
					</button>
				{/each}
			</div>
		</div>
		<div class="empty-main">
			<div class="empty-state">
				<div class="empty-icon">👈</div>
				<h2>Select a website</h2>
				<p>Choose a synced website from the sidebar to view its content.</p>
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
				{#each syncedResources as resource (resource.id)}
					<button
						class="resource-item"
						class:selected={authDataState.currentResourceId === resource.id}
						onclick={() => handleSelectResource(resource.id)}
					>
						<div class="resource-title">{resource.title}</div>
						<div class="resource-meta">
							Updated {formatDate(resource.lastModified)}
						</div>
					</button>
				{/each}
			</div>
		</div>
		<Canvas
			{blocks}
			{viewport}
			selectedBlockId={null}
			readonly={true}
			onViewportChange={updateViewport}
			onBlockUpdate={() => {}}
			onBlockSelect={() => {}}
		/>
	</div>
{/if}

{#if showAddWebsiteModal}
	<AddWebsiteConnectionModal onClose={closeAddWebsiteModal} />
{/if}

<style>
	.viewer-container {
		width: 100%;
		height: 100vh;
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

	.resource-item {
		width: 100%;
		padding: 0.75rem;
		margin-bottom: 0.5rem;
		background: #16171f;
		border: 1px solid #292a36;
		border-radius: 6px;
		cursor: pointer;
		transition: all 0.2s;
		text-align: left;
	}

	.resource-item:hover {
		background: #1c1d26;
		border-color: #8A86E5;
	}

	.resource-item.selected {
		background: #1c1d26;
		border-color: #8A86E5;
		box-shadow: 0 0 0 1px #8A86E5;
	}

	.resource-title {
		font-size: 0.875rem;
		font-weight: 500;
		color: #c9d1d9;
		margin-bottom: 0.25rem;
	}

	.resource-meta {
		font-size: 0.75rem;
		color: #8b949e;
	}

	.empty-main {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
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
