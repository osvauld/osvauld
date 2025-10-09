<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import * as Y from "yjs";

	let editorContainer = $state<HTMLDivElement>();
	let blocks = $state<any[]>([]);
	let doc: Y.Doc;
	let yBlocks: Y.Array<any>;

	onMount(async () => {
		try {
			console.log("🚀 Creating simple block system with Yjs...");

			// Create a Yjs document
			doc = new Y.Doc();

			// Create a Y.Array for blocks
			yBlocks = doc.getArray("blocks");

			// Add some initial blocks
			yBlocks.push([
				{
					id: "block-1",
					type: "heading",
					content: "My First Page",
				},
			]);

			yBlocks.push([
				{
					id: "block-2",
					type: "paragraph",
					content: "This is a simple block-based editor built on Yjs.",
				},
			]);

			yBlocks.push([
				{
					id: "block-3",
					type: "paragraph",
					content:
						"Click any block to edit it (not implemented yet, but the data model is ready!).",
				},
			]);

			// Subscribe to changes
			yBlocks.observe(() => {
				blocks = yBlocks.toArray();
				console.log("📦 Blocks updated:", blocks);
			});

			// Initial render
			blocks = yBlocks.toArray();

			// For debugging
			if (typeof window !== "undefined") {
				(window as any).doc = doc;
				(window as any).yBlocks = yBlocks;
				(window as any).addBlock = (type: string, content: string) => {
					yBlocks.push([
						{
							id: `block-${Date.now()}`,
							type,
							content,
						},
					]);
				};
			}

			console.log("✅ Block system initialized!");
			console.log("💡 Try: window.addBlock('paragraph', 'New block!')");
		} catch (error) {
			console.error("❌ Failed:", error);
		}
	});

	onDestroy(() => {
		if (doc) {
			doc.destroy();
		}
	});

	function handleBlockClick(blockId: string) {
		console.log("Clicked block:", blockId);
	}

	function handleBlockInput(event: Event, index: number) {
		const target = event.target as HTMLDivElement;
		const newContent = target.textContent || "";

		// Update the block in Yjs
		const block = yBlocks.get(index);
		yBlocks.delete(index, 1);
		yBlocks.insert(index, [{ ...block, content: newContent }]);
	}

	function handleKeyDown(event: KeyboardEvent, index: number) {
		if (event.key === "Enter") {
			event.preventDefault();

			// Create new block after current one
			const newBlock = {
				id: `block-${Date.now()}`,
				type: "paragraph",
				content: "",
			};

			yBlocks.insert(index + 1, [newBlock]);

			// Focus the new block
			setTimeout(() => {
				const blocks = editorContainer?.querySelectorAll('.block');
				const nextBlock = blocks?.[index + 1] as HTMLElement;
				nextBlock?.focus();
			}, 0);
		}
	}
</script>

<style>
	.editor-wrapper {
		width: 100%;
		height: 100%;
		padding: 40px;
		background: #fff;
		font-family:
			system-ui,
			-apple-system,
			sans-serif;
	}

	.block {
		margin-bottom: 16px;
		padding: 8px 12px;
		border-radius: 4px;
		cursor: pointer;
		transition: background 0.2s;
	}

	.block:hover {
		background: #f5f5f5;
	}

	.block:focus {
		outline: 2px solid #667eea;
		outline-offset: -2px;
		background: #f8f9ff;
	}

	.block-heading {
		font-size: 32px;
		font-weight: 700;
		margin-bottom: 24px;
	}

	.block-paragraph {
		font-size: 16px;
		line-height: 1.6;
		color: #333;
	}

	.info {
		margin-top: 40px;
		padding: 20px;
		background: #e3f2fd;
		border-radius: 8px;
		font-size: 14px;
	}

	.info code {
		background: #fff;
		padding: 2px 6px;
		border-radius: 3px;
		font-family: monospace;
	}
</style>

<div class="editor-wrapper" bind:this={editorContainer}>
	{#each blocks as block, index}
		<div
			class="block block-{block.type}"
			contenteditable="true"
			role="textbox"
			aria-multiline="false"
			oninput={(e) => handleBlockInput(e, index)}
			onkeydown={(e) => handleKeyDown(e, index)}
		>
			{block.content}
		</div>
	{/each}

	<div class="info">
		<strong>✨ Editable Block System Ready!</strong><br />
		• Click any block to start typing<br />
		• Press <code>Enter</code> to create a new block<br />
		• Try: <code>window.addBlock('paragraph', 'Hello!')</code><br />
		• All blocks are stored in Yjs for collaboration support
	</div>
</div>
