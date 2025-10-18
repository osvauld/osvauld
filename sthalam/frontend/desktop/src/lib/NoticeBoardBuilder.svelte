<script lang="ts">
	import { onMount, onDestroy, untrack } from "svelte";
	import { dataState } from "../state";
	import { sendMessage } from "../utils/helper";
	import NavigationPanel from "../components/NavigationPanel.svelte";
	import ThreadPost from "./ThreadPost.svelte";
	import CommentList from "./CommentList.svelte";
	import type { YjsDocuments } from "./yjsManager";

	let yDocs: YjsDocuments | null = null;

	// NEW: Separate state for thread post and comments
	let threadPostBlocks = $state<Map<string, any>>(new Map()); // From mainDoc
	let commentBlocks = $state<Map<string, any>>(new Map());    // From secondaryDoc

	let autoSaveInterval: number | null = null;

	// Track if we have a resource selected
	const hasResource = $derived(!!dataState.currentResourceId);

	// Get the thread post block (type: 'thread-post') from threadPostBlocks
	const threadPost = $derived(() => {
		for (const [id, block] of threadPostBlocks) {
			if (block.type === 'thread-post') {
				return block;
			}
		}
		return null;
	});

	// Get all comment blocks (type: 'comment') from commentBlocks
	const comments = $derived(() => {
		const commentList: any[] = [];
		for (const [id, block] of commentBlocks) {
			if (block.type === 'comment') {
				commentList.push(block);
			}
		}
		// Sort by order/timestamp
		return commentList.sort((a, b) => (a.order || 0) - (b.order || 0));
	});

	onMount(async () => {
		console.log("🚀 Initializing Thread Builder...");

		// Set up auto-save every 10 seconds
		autoSaveInterval = window.setInterval(async () => {
			const currentResourceId = dataState.currentResourceId;
			if (currentResourceId) {
				try {
					console.log("💾 Auto-saving thread:", currentResourceId);
					await dataState.saveCurrentResource(currentResourceId);
					console.log("✅ Auto-save completed");
				} catch (error) {
					console.error("❌ Auto-save failed:", error);
				}
			}
		}, 10000); // 10 seconds

		console.log("✅ Thread Builder initialized with auto-save!");
	});

	// React to resource changes
	$effect(() => {
		const resourceId = dataState.currentResourceId;
		console.log("🔄 Resource changed:", resourceId);

		if (!resourceId) {
			// No resource selected - clear everything
			console.log("❌ No resource selected - clearing workspace");
			yDocs = null;
			threadPostBlocks = new Map();
			commentBlocks = new Map();
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
				hasMainDoc: !!docs.mainDoc,
				hasSecondaryDoc: !!docs.secondaryDoc
			});

			// NEW: Subscribe to mainDoc blocks (thread post)
			const mainBlocksObserver = () => {
				if (!yDocs) return;
				const newBlocks = new Map();
				yDocs.blocks.forEach((value, key) => {
					newBlocks.set(key, value);
				});
				threadPostBlocks = newBlocks;
				console.log("📦 Thread post blocks updated:", threadPostBlocks.size);
			};

			docs.blocks.observe(mainBlocksObserver);

			// NEW: Subscribe to secondaryDoc blocks (comments) if it exists
			const secondaryBlocksObserver = () => {
				if (!yDocs || !yDocs.secondaryBlocks) return;
				const newBlocks = new Map();
				yDocs.secondaryBlocks.forEach((value, key) => {
					newBlocks.set(key, value);
				});
				commentBlocks = newBlocks;
				console.log("📦 Comment blocks updated:", commentBlocks.size);
			};

			if (docs.secondaryBlocks) {
				docs.secondaryBlocks.observe(secondaryBlocksObserver);
			}

			// Initial load
			mainBlocksObserver();
			secondaryBlocksObserver();

			// If no thread post exists, create one
			if (!Array.from(threadPostBlocks.values()).some(b => b.type === 'thread-post')) {
				console.log("📝 Creating initial thread post block");
				createThreadPost();
			}
		});

		// Cleanup function for this effect
		return () => {
			if (yDocs) {
				// Yjs will clean up when docs are destroyed
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

	function createThreadPost() {
		if (!yDocs) return;
		const id = `thread-post-${Date.now()}`;
		const newBlock = {
			id,
			type: 'thread-post',
			content: '',
			mode: 'markdown',
			css: '',
			author: 'Owner',
			timestamp: new Date().toISOString(),
			order: 0
		};
		// Add to mainDoc (thread post)
		yDocs.blocks.set(id, newBlock);
	}

	function updateThreadPost(updates: any) {
		if (!yDocs || !threadPost()) return;
		const post = threadPost();
		if (post) {
			// Update in mainDoc (thread post)
			yDocs.blocks.set(post.id, { ...post, ...updates });
		}
	}

	async function addComment(content: string, mode: string, css: string, parentId?: string) {
		if (!yDocs || !yDocs.secondaryBlocks) return;
		const id = `comment-${Date.now()}`;
		const newBlock = {
			id,
			type: 'comment',
			content,
			mode,
			css: css || '',
			author: 'Owner', // Will be replaced with actual user later
			timestamp: new Date().toISOString(),
			parentId: parentId || null,
			order: commentBlocks.size
		};
		// NEW: Add to secondaryDoc (comments)
		yDocs.secondaryBlocks.set(id, newBlock);

		// Trigger immediate save and sync after adding comment
		const currentResourceId = dataState.currentResourceId;
		if (currentResourceId) {
			try {
				// IMPORTANT: Wait for save to complete before syncing
				await dataState.saveCurrentResource(currentResourceId);
				console.log('✅ Comment saved to database');

				// Sync resource to P2P network after saving completes
				await sendMessage('syncResource', { resourceId: currentResourceId });
				console.log('✅ Comment synced to P2P network');
			} catch (error) {
				console.error('Failed to save/sync comment:', error);
				// Don't fail the comment addition if save/sync fails
			}
		}
	}

	function updateComment(commentId: string, updates: any) {
		if (!yDocs || !yDocs.secondaryBlocks) return;
		const comment = commentBlocks.get(commentId);
		if (comment) {
			// NEW: Update in secondaryDoc (comments)
			yDocs.secondaryBlocks.set(commentId, { ...comment, ...updates });
		}
	}

	function deleteComment(commentId: string) {
		if (!yDocs || !yDocs.secondaryBlocks) return;
		// NEW: Delete from secondaryDoc (comments)
		yDocs.secondaryBlocks.delete(commentId);
	}
</script>

{#if !hasResource}
	<!-- No resource selected - show empty state -->
	<div class="empty-state">
		<NavigationPanel />
		<div class="empty-message">
			<h2>No thread selected</h2>
			<p>Select a thread from the sidebar or create a new one to get started.</p>
		</div>
	</div>
{:else}
	<!-- Resource selected - show builder -->
	<div class="builder-container">
		<NavigationPanel />
		<div class="main-content">
			<div class="thread-container">
				{#if threadPost()}
					<ThreadPost
						post={threadPost()}
						onUpdate={updateThreadPost}
					/>
				{/if}

				<CommentList
					comments={comments()}
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
		height: 100%;
		overflow: hidden;
		background: #010409;
		display: flex;
	}

	.empty-state {
		width: 100%;
		height: 100%;
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
