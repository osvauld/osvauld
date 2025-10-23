<script lang="ts">
	import type { TreeLayout } from "./treeLayoutEngine";

	interface Props {
		treeLayouts: Map<string, Map<string, TreeLayout>>;
		blocks: Map<string, any>;
		rowHeight: number;
		indent: number;
	}

	let { treeLayouts, blocks, rowHeight, indent }: Props = $props();

	// Calculate all line segments that need to be drawn
	const lineSegments = $derived.by(() => {
		const segments: Array<{
			x: number;
			y1: number;
			y2: number;
			type: "vertical" | "horizontal" | "corner";
		}> = [];

		for (const [screenId, screenLayout] of treeLayouts.entries()) {
			const layoutArray = Array.from(screenLayout.entries());

			for (let idx = 0; idx < layoutArray.length; idx++) {
				const [blockId, layout] = layoutArray[idx];
				const block = blocks.get(blockId);

				if (!block || layout.depth === 0) continue;

				// Get parent info
				const parent = blocks.get(block.parentId);
				if (!parent) continue;

				// Check if this is the last child
				const isLastChild =
					parent.children && parent.children[parent.children.length - 1] === blockId;

				// Horizontal line from parent level to node
				const horizontalX = layout.x - indent + 10;
				const horizontalY = layout.y + rowHeight / 2;

				segments.push({
					x: horizontalX,
					y1: horizontalY,
					y2: horizontalY,
					type: "horizontal"
				});

				// Vertical line connecting to parent
				if (isLastChild) {
					// L-shaped corner - only go up to center
					const parentLayout = screenLayout.get(block.parentId);
					if (parentLayout) {
						const parentY = parentLayout.y + rowHeight / 2;
						segments.push({
							x: horizontalX,
							y1: parentY,
							y2: horizontalY,
							type: "corner"
						});
					}
				} else {
					// T-shaped - continue down to next sibling
					// Find the next sibling's Y position
					let nextSiblingY = horizontalY + rowHeight;

					// Look through remaining nodes to find next sibling at same depth under same parent
					for (let j = idx + 1; j < layoutArray.length; j++) {
						const [nextBlockId, nextLayout] = layoutArray[j];
						const nextBlock = blocks.get(nextBlockId);

						if (!nextBlock) continue;

						// Same parent = sibling
						if (nextBlock.parentId === block.parentId) {
							nextSiblingY = nextLayout.y + rowHeight / 2;
							break;
						}

						// Different parent but shallower depth = we've left this parent's children
						if (nextLayout.depth < layout.depth) break;
					}

					segments.push({
						x: horizontalX,
						y1: horizontalY,
						y2: nextSiblingY,
						type: "vertical"
					});
				}

				// Draw vertical lines for all ancestors that have more siblings below this node
				let currentBlock = block;
				let currentDepth = layout.depth - 1;

				while (currentBlock.parentId && currentDepth >= 0) {
					const ancestor = blocks.get(currentBlock.parentId);
					if (!ancestor) break;

					// Check if ancestor has more siblings
					if (ancestor.parentId) {
						const grandParent = blocks.get(ancestor.parentId);
						if (
							grandParent?.children &&
							grandParent.children[grandParent.children.length - 1] !== ancestor.id
						) {
							// Ancestor has more siblings, draw vertical line at that level
							const lineX = layout.x - (layout.depth - currentDepth) * indent + 10;

							// Find where this line should extend to
							let extendToY = layout.y + rowHeight;

							// Find next node at the ancestor's depth with same grandparent
							for (let j = idx + 1; j < layoutArray.length; j++) {
								const [nextBlockId, nextLayout] = layoutArray[j];
								const nextBlock = blocks.get(nextBlockId);

								if (nextLayout.depth === currentDepth && nextBlock?.parentId === ancestor.parentId) {
									extendToY = nextLayout.y + rowHeight / 2;
									break;
								}

								if (nextLayout.depth < currentDepth) break;
							}

							segments.push({
								x: lineX,
								y1: layout.y - rowHeight / 2,
								y2: extendToY,
								type: "vertical"
							});
						}
					}

					currentBlock = ancestor;
					currentDepth--;
				}
			}
		}

		return segments;
	});
</script>

<svg class="tree-guide-lines" style:pointer-events="none">
	{#each lineSegments as segment}
		{#if segment.type === "horizontal"}
			<line
				x1={segment.x}
				y1={segment.y1}
				x2={segment.x + 10}
				y2={segment.y2}
				stroke="rgba(110, 118, 129, 0.3)"
				stroke-width="1"
			/>
		{:else}
			<line
				x1={segment.x}
				y1={segment.y1}
				x2={segment.x}
				y2={segment.y2}
				stroke="rgba(110, 118, 129, 0.3)"
				stroke-width="1"
			/>
		{/if}
	{/each}
</svg>

<style>
	.tree-guide-lines {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
		overflow: visible;
	}
</style>
