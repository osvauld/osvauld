/**
 * Tree Layout Engine
 * Calculates positions for tree-based block rendering on canvas
 */

export interface TreeLayout {
	x: number; // X position on canvas
	y: number; // Y position on canvas
	depth: number; // Nesting level (for indentation)
	isExpanded: boolean; // Whether this node is expanded
}

export interface TreeLayoutConfig {
	rowHeight: number; // Height per tree node
	indent: number; // Indentation per nesting level
	nodeWidth: number; // Fixed width per node
	screenSpacing: number; // Vertical space between screens
	startX: number; // Starting X position
	startY: number; // Starting Y position
}

export const DEFAULT_TREE_CONFIG: TreeLayoutConfig = {
	rowHeight: 32,
	indent: 24,
	nodeWidth: 300,
	screenSpacing: 50,
	startX: 50,
	startY: 50
};

/**
 * Layout a single screen as a tree
 */
export function layoutTreeForScreen(
	screenId: string,
	blocks: Map<string, any>,
	collapsedBlocks: Set<string>,
	startX: number,
	startY: number,
	config: TreeLayoutConfig = DEFAULT_TREE_CONFIG
): Map<string, TreeLayout> {
	const layouts = new Map<string, TreeLayout>();
	let currentY = startY;

	function layoutBlock(blockId: string, depth: number) {
		const block = blocks.get(blockId);
		if (!block) return;

		const isExpanded = !collapsedBlocks.has(blockId);
		const hasChildren =
			block.children && Array.isArray(block.children) && block.children.length > 0;

		// Store layout for this block
		layouts.set(blockId, {
			x: startX + depth * config.indent,
			y: currentY,
			depth,
			isExpanded
		});

		// Move to next row
		currentY += config.rowHeight;

		// Layout children if expanded and has children
		if (isExpanded && hasChildren) {
			for (const childId of block.children) {
				layoutBlock(childId, depth + 1);
			}
		}
	}

	// Start with screen root at depth 0
	layoutBlock(screenId, 0);

	return layouts;
}

/**
 * Layout all screens as separate trees
 */
export function layoutAllScreens(
	screens: string[],
	blocks: Map<string, any>,
	collapsedBlocks: Set<string>,
	config: TreeLayoutConfig = DEFAULT_TREE_CONFIG
): Map<string, Map<string, TreeLayout>> {
	const allLayouts = new Map<string, Map<string, TreeLayout>>();
	let currentY = config.startY;

	for (const screenId of screens) {
		const screenLayout = layoutTreeForScreen(
			screenId,
			blocks,
			collapsedBlocks,
			config.startX,
			currentY,
			config
		);

		allLayouts.set(screenId, screenLayout);

		// Calculate height of this screen's tree and add spacing
		const screenHeight = screenLayout.size * config.rowHeight;
		currentY += screenHeight + config.screenSpacing;
	}

	return allLayouts;
}

/**
 * Get all screen-container blocks
 */
export function getScreenBlocks(blocks: Map<string, any>): string[] {
	const screens: string[] = [];
	for (const [id, block] of blocks.entries()) {
		if (block.type === "screen-container") {
			screens.push(id);
		}
	}
	return screens;
}

/**
 * Get children of a block (handles both array and undefined)
 */
export function getBlockChildren(block: any): string[] {
	if (!block.children || !Array.isArray(block.children)) {
		return [];
	}
	return block.children;
}
