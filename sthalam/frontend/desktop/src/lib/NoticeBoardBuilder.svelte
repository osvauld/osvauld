<script lang="ts">
	import { onMount, onDestroy, untrack } from "svelte";
	import { dataState } from "../state";
	import NavigationPanel from "../components/NavigationPanel.svelte";
	import ThreadPost from "./ThreadPost.svelte";
	import CommentList from "./CommentList.svelte";
	import type { YjsDocuments } from "./yjsManager";

	let yDocs: YjsDocuments | null = null;
	let blocks = $state<Map<string, any>>(new Map());
	let autoSaveInterval: number | null = null;

	// Track if we have a resource selected
	const hasResource = $derived(!!dataState.currentResourceId);

	// Get the thread post block (type: 'thread-post')
	const threadPost = $derived(() => {
		for (const [id, block] of blocks) {
			if (block.type === 'thread-post') {
				return block;
			}
		}
		return null;
	});

	// Get all comment blocks (type: 'comment')
	const comments = $derived(() => {
		const commentBlocks: any[] = [];
		for (const [id, block] of blocks) {
			if (block.type === 'comment') {
				commentBlocks.push(block);
			}
		}
		// Sort by order/timestamp
		return commentBlocks.sort((a, b) => (a.order || 0) - (b.order || 0));
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

			console.log("📦 Yjs documents received for thread_doc");

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

			docs.blocks.observe(blocksObserver);

			// Initial load
			blocksObserver();

			// If no thread post exists, create one
			if (!Array.from(blocks.values()).some(b => b.type === 'thread-post')) {
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
		yDocs.blocks.set(id, newBlock);
	}

	function updateThreadPost(updates: any) {
		if (!yDocs || !threadPost()) return;
		const post = threadPost();
		if (post) {
			yDocs.blocks.set(post.id, { ...post, ...updates });
		}
	}

	function addComment(content: string, mode: string, css: string, parentId?: string) {
		if (!yDocs) return;
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
			order: blocks.size
		};
		yDocs.blocks.set(id, newBlock);
	}

	function updateComment(commentId: string, updates: any) {
		if (!yDocs) return;
		const comment = blocks.get(commentId);
		if (comment) {
			yDocs.blocks.set(commentId, { ...comment, ...updates });
		}
	}

	function deleteComment(commentId: string) {
		if (!yDocs) return;
		yDocs.blocks.delete(commentId);
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
