/**
 * Graph Layout Engine
 * Provides automatic layout algorithms for the graph view
 */

export interface LayoutConfig {
	horizontalSpacing: number;
	verticalSpacing: number;
	nodeWidth: number;
	nodeHeight: number;
	mode: 'compact' | 'spacious';
	containerPadding: number;
	childrenPerRow: number; // For grid layout within containers
}

export interface LayoutResult {
	blockId: string;
	x: number;
	y: number;
}

const DEFAULT_CONFIG: LayoutConfig = {
	horizontalSpacing: 250,
	verticalSpacing: 180,
	nodeWidth: 200,
	nodeHeight: 80,
	mode: 'spacious',
	containerPadding: 40,
	childrenPerRow: 3
};

/**
 * Apply hierarchical tree layout to blocks
 * Uses parentId for relationships (canvas-style nested containers)
 */
export function layoutHierarchical(
	blocks: Map<string, any>,
	config: Partial<LayoutConfig> = {}
): LayoutResult[] {
	const cfg = { ...DEFAULT_CONFIG, ...config };

	// Adjust spacing based on mode
	if (cfg.mode === 'compact') {
		cfg.horizontalSpacing = 180;
		cfg.verticalSpacing = 120;
		cfg.containerPadding = 30;
	}

	// Build parent-child relationships from parentId
	const children = new Map<string, Set<string>>();
	const roots = new Set<string>();

	for (const [id, block] of blocks.entries()) {
		if (block.parentId) {
			// This block has a parent
			if (!children.has(block.parentId)) {
				children.set(block.parentId, new Set());
			}
			children.get(block.parentId)!.add(id);
		} else {
			// This is a root block (no parent)
			roots.add(id);
		}
	}

	// If no roots found, use screen containers or entry points
	if (roots.size === 0) {
		for (const [id, block] of blocks.entries()) {
			if (block.type === 'screen-container' || block.isEntryPoint) {
				roots.add(id);
			}
		}
	}

	// Still no roots? Just take the first few nodes
	if (roots.size === 0 && blocks.size > 0) {
		const firstIds = Array.from(blocks.keys()).slice(0, 3);
		firstIds.forEach(id => roots.add(id));
	}

	const positions = new Map<string, { x: number; y: number }>();

	// Position screen containers side-by-side and layout their children
	const containerSizes = new Map<string, { width: number; height: number }>();
	let treeOffsetX = 100;
	const startY = 100;

	for (const rootId of roots) {
		const rootBlock = blocks.get(rootId);
		if (!rootBlock) continue;

		// Position the container
		positions.set(rootId, { x: treeOffsetX, y: startY });

		// Calculate container size from children's bounding box
		const containerChildren = Array.from(children.get(rootId) || []);
		const { width, height } = calculateContainerBounds(
			rootId,
			containerChildren,
			treeOffsetX,
			startY,
			blocks,
			children,
			cfg
		);

		containerSizes.set(rootId, { width, height });
		treeOffsetX += width + cfg.horizontalSpacing;
	}

	// Convert to result array with container sizes
	const results: LayoutResult[] = [];
	for (const [blockId, pos] of positions.entries()) {
		results.push({
			blockId,
			x: pos.x,
			y: pos.y
		});
	}

	// Also return container size updates
	for (const [containerId, size] of containerSizes.entries()) {
		const result = results.find(r => r.blockId === containerId);
		if (result) {
			(result as any).width = size.width;
			(result as any).height = size.height;
		}
	}

	return results;
}

/**
 * Calculate bounding box for container based on children's absolute positions
 * Returns the total width and height needed for the container
 */
function calculateContainerBounds(
	containerId: string,
	childIds: string[],
	containerX: number,
	containerY: number,
	blocks: Map<string, any>,
	children: Map<string, Set<string>>,
	config: LayoutConfig
): { width: number; height: number } {
	const padding = config.containerPadding;

	if (childIds.length === 0) {
		return {
			width: 300,  // Smaller default width for empty container
			height: 200  // Smaller default height for empty container
		};
	}

	// Calculate bounding box from all descendants (recursive)
	let minX = Infinity;
	let minY = Infinity;
	let maxX = -Infinity;
	let maxY = -Infinity;

	function processBlock(blockId: string) {
		const block = blocks.get(blockId);
		if (!block) return;

		const blockX = block.x;
		const blockY = block.y;
		const blockWidth = block.width || config.nodeWidth;
		const blockHeight = block.height || config.nodeHeight;

		minX = Math.min(minX, blockX);
		minY = Math.min(minY, blockY);
		maxX = Math.max(maxX, blockX + blockWidth);
		maxY = Math.max(maxY, blockY + blockHeight);

		// Process children recursively
		const blockChildren = children.get(blockId);
		if (blockChildren) {
			for (const childId of blockChildren) {
				processBlock(childId);
			}
		}
	}

	// Process all direct children and their descendants
	for (const childId of childIds) {
		processBlock(childId);
	}

	// Calculate container size with padding
	const contentWidth = maxX - minX;
	const contentHeight = maxY - minY;

	// Add padding on all sides, plus extra for header badge
	const totalWidth = contentWidth + (padding * 2);
	const totalHeight = contentHeight + (padding * 2) + 40; // Extra 40px for header

	return {
		width: Math.max(totalWidth, 300),  // Minimum size reduced
		height: Math.max(totalHeight, 150)
	};
}

/**
 * Position a tree rooted at the given node
 * Returns the total width of the tree
 */
function positionTree(
	nodeId: string,
	x: number,
	y: number,
	blocks: Map<string, any>,
	children: Map<string, Set<string>>,
	positions: Map<string, { x: number; y: number }>,
	config: LayoutConfig
): number {
	const block = blocks.get(nodeId);
	if (!block) return 0;

	// Get actual node width or use default
	const nodeWidth = block.width || config.nodeWidth;

	const childIds = Array.from(children.get(nodeId) || []);

	if (childIds.length === 0) {
		// Leaf node - just position it
		positions.set(nodeId, { x, y });
		return nodeWidth;
	}

	// Position children first (bottom-up)
	const childPositions: Array<{ id: string; width: number; x: number }> = [];
	let totalChildrenWidth = 0;
	let currentChildX = x;

	for (let i = 0; i < childIds.length; i++) {
		const childId = childIds[i];
		const childWidth = positionTree(
			childId,
			currentChildX,
			y + config.verticalSpacing,
			blocks,
			children,
			positions,
			config
		);

		childPositions.push({
			id: childId,
			width: childWidth,
			x: currentChildX
		});

		totalChildrenWidth += childWidth;
		if (i < childIds.length - 1) {
			totalChildrenWidth += config.horizontalSpacing;
			currentChildX += childWidth + config.horizontalSpacing;
		}
	}

	// Position parent centered above children
	const firstChildX = childPositions[0].x;
	const lastChildX = childPositions[childPositions.length - 1].x + childPositions[childPositions.length - 1].width;
	const childrenCenterX = (firstChildX + lastChildX) / 2;

	// Center parent above children
	const parentX = childrenCenterX - nodeWidth / 2;
	positions.set(nodeId, { x: parentX, y });

	// Return the wider of: parent width or children total width
	return Math.max(nodeWidth, totalChildrenWidth);
}

/**
 * Layout nodes in a simple grid (alternative layout)
 */
export function layoutGrid(
	blocks: Map<string, any>,
	columns: number = 4,
	spacing: number = 250
): LayoutResult[] {
	const results: LayoutResult[] = [];
	const blockArray = Array.from(blocks.entries());

	blockArray.forEach(([id], index) => {
		const col = index % columns;
		const row = Math.floor(index / columns);

		results.push({
			blockId: id,
			x: 100 + col * spacing,
			y: 100 + row * spacing
		});
	});

	return results;
}

/**
 * Layout nodes to minimize edge crossings (simplified force-directed)
 */
export function layoutForceDirected(
	blocks: Map<string, any>,
	iterations: number = 50
): LayoutResult[] {
	// Simple spring-based layout
	const positions = new Map<string, { x: number; y: number; vx: number; vy: number }>();

	// Initialize random positions
	for (const [id] of blocks.entries()) {
		positions.set(id, {
			x: Math.random() * 1000,
			y: Math.random() * 800,
			vx: 0,
			vy: 0
		});
	}

	const repulsionStrength = 5000;
	const attractionStrength = 0.01;
	const damping = 0.8;

	// Build edge list
	const edges: Array<[string, string]> = [];
	for (const [id, block] of blocks.entries()) {
		if (block.children) {
			for (const childId of block.children) {
				edges.push([id, childId]);
			}
		}
	}

	// Run simulation
	for (let iter = 0; iter < iterations; iter++) {
		// Repulsion between all nodes
		for (const [id1, pos1] of positions.entries()) {
			for (const [id2, pos2] of positions.entries()) {
				if (id1 === id2) continue;

				const dx = pos2.x - pos1.x;
				const dy = pos2.y - pos1.y;
				const distSq = dx * dx + dy * dy + 1;
				const force = repulsionStrength / distSq;

				pos1.vx -= (dx / Math.sqrt(distSq)) * force;
				pos1.vy -= (dy / Math.sqrt(distSq)) * force;
			}
		}

		// Attraction along edges
		for (const [from, to] of edges) {
			const pos1 = positions.get(from);
			const pos2 = positions.get(to);
			if (!pos1 || !pos2) continue;

			const dx = pos2.x - pos1.x;
			const dy = pos2.y - pos1.y;
			const force = attractionStrength;

			pos1.vx += dx * force;
			pos1.vy += dy * force;
			pos2.vx -= dx * force;
			pos2.vy -= dy * force;
		}

		// Update positions
		for (const pos of positions.values()) {
			pos.x += pos.vx;
			pos.y += pos.vy;
			pos.vx *= damping;
			pos.vy *= damping;
		}
	}

	// Convert to results
	return Array.from(positions.entries()).map(([blockId, pos]) => ({
		blockId,
		x: Math.max(50, pos.x),
		y: Math.max(50, pos.y)
	}));
}

/**
 * Calculate bounding box for all blocks
 */
export function calculateBoundingBox(blocks: Map<string, any>): {
	minX: number;
	minY: number;
	maxX: number;
	maxY: number;
	width: number;
	height: number;
} {
	let minX = Infinity;
	let minY = Infinity;
	let maxX = -Infinity;
	let maxY = -Infinity;

	for (const block of blocks.values()) {
		const width = block.width || 200;
		const height = block.height || 80;

		minX = Math.min(minX, block.x);
		minY = Math.min(minY, block.y);
		maxX = Math.max(maxX, block.x + width);
		maxY = Math.max(maxY, block.y + height);
	}

	return {
		minX,
		minY,
		maxX,
		maxY,
		width: maxX - minX,
		height: maxY - minY
	};
}
