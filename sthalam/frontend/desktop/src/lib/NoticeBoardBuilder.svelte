<script lang="ts">
	import { onMount, onDestroy, untrack } from "svelte";
	import { dataState } from "../state";
	import NavigationPanel from "../components/NavigationPanel.svelte";
	import ThreadEditor from "./ThreadEditor.svelte";
	import CommentSection from "./CommentSection.svelte";
	import type { YjsDocuments } from "./yjsManager";

	let yDocs: YjsDocuments | null = null;
	let threadContent = $state<string>("");
	let comments = $state<any[]>([]);
	let autoSaveInterval: number | null = null;

	// Track if we have a resource selected
	const hasResource = $derived(!!dataState.currentResourceId);

	onMount(async () => {
		console.log("🚀 Initializing NoticeBoard Builder...");

		// Set up auto-save every 10 seconds
		autoSaveInterval = window.setInterval(async () => {
			const currentResourceId = dataState.currentResourceId;
			if (currentResourceId) {
				try {
					console.log("💾 Auto-saving notice board:", currentResourceId);
					await dataState.saveCurrentResource(currentResourceId);
					console.log("✅ Auto-save completed");
				} catch (error) {
					console.error("❌ Auto-save failed:", error);
				}
			}
		}, 10000); // 10 seconds

		console.log("✅ NoticeBoard Builder initialized with auto-save!");
	});

	// React to resource changes
	$effect(() => {
		const resourceId = dataState.currentResourceId;
		console.log("🔄 Resource changed:", resourceId);

		if (!resourceId) {
			// No resource selected - clear everything
			console.log("❌ No resource selected - clearing workspace");
			yDocs = null;
			threadContent = "";
			comments = [];
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

			console.log("📦 Yjs documents received for thread_doc");

			// Subscribe to thread content changes
			const threadObserver = () => {
				if (!yDocs) return;
				const content = yDocs.blocks.get("thread_content");
				threadContent = content || "";
				console.log("📝 Thread content updated");
			};

			// Subscribe to comments changes
			const commentsObserver = () => {
				if (!yDocs) return;
				const commentsData = yDocs.blocks.get("comments");
				if (Array.isArray(commentsData)) {
					comments = commentsData;
				} else {
					comments = [];
				}
				console.log("💬 Comments updated:", comments.length);
			};

			docs.blocks.observe(threadObserver);
			docs.blocks.observe(commentsObserver);

			// Initial load
			threadObserver();
			commentsObserver();
		});

		// Cleanup function for this effect
		return () => {
			if (yDocs) {
				// Note: Yjs will clean up when docs are destroyed
			}
		};
	});

	onDestroy(() => {
		// Clear auto-save interval
		if (autoSaveInterval !== null) {
			clearInterval(autoSaveInterval);
			autoSaveInterval = null;
		}
	});

	function updateThreadContent(content: string) {
		if (!yDocs) return;
		yDocs.blocks.set("thread_content", content);
	}

	function addComment(comment: any) {
		if (!yDocs) return;
		const currentComments = yDocs.blocks.get("comments") || [];
		const newComments = Array.isArray(currentComments) ? [...currentComments, comment] : [comment];
		yDocs.blocks.set("comments", newComments);
	}

	function updateComment(commentId: string, updates: any) {
		if (!yDocs) return;
		const currentComments = yDocs.blocks.get("comments") || [];
		if (!Array.isArray(currentComments)) return;

		const updatedComments = currentComments.map((c: any) =>
			c.id === commentId ? { ...c, ...updates } : c
		);
		yDocs.blocks.set("comments", updatedComments);
	}

	function deleteComment(commentId: string) {
		if (!yDocs) return;
		const currentComments = yDocs.blocks.get("comments") || [];
		if (!Array.isArray(currentComments)) return;

		const filteredComments = currentComments.filter((c: any) => c.id !== commentId);
		yDocs.blocks.set("comments", filteredComments);
	}
</script>

{#if !hasResource}
	<!-- No resource selected - show empty state -->
	<div class="empty-state">
		<NavigationPanel />
		<div class="empty-message">
			<h2>No notice board selected</h2>
			<p>Select a notice board from the sidebar or create a new one to get started.</p>
		</div>
	</div>
{:else}
	<!-- Resource selected - show builder -->
	<div class="builder-container">
		<NavigationPanel />
		<div class="main-content">
			<div class="thread-container">
				<ThreadEditor
					content={threadContent}
					onUpdate={updateThreadContent}
				/>
				<CommentSection
					{comments}
					onAddComment={addComment}
					onUpdateComment={updateComment}
					onDeleteComment={deleteComment}
				/>
			</div>
		</div>
	</div>
{/if}

<style>
	.builder-container {
		width: 100%;
		height: 100vh;
		overflow: hidden;
		background: #010409;
		display: flex;
	}

	.empty-state {
		width: 100%;
		height: 100vh;
		display: flex;
		background: #010409;
	}

	.empty-message {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		color: #8b949e;
	}

	.empty-message h2 {
		font-size: 1.5rem;
		margin-bottom: 0.5rem;
		color: #c9d1d9;
	}

	.empty-message p {
		font-size: 1rem;
	}

	.main-content {
		flex: 1;
		display: flex;
		justify-content: center;
		overflow-y: auto;
		background: #0d1117;
	}

	.thread-container {
		width: 100%;
		max-width: 900px;
		padding: 2rem;
	}
</style>
