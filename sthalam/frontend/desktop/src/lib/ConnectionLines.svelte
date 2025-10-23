<script lang="ts">
	interface Props {
		blocks: Map<string, any>;
		viewport: { x: number; y: number; zoom: number };
		selectedBlockId: string | null;
		onConnectionSelect?: (blockId: string, connectionId: string) => void;
	}

	let { blocks, viewport, selectedBlockId, onConnectionSelect }: Props = $props();

	function handleConnectionClick(e: MouseEvent, fromBlockId: string, connectionId: string) {
		e.stopPropagation();
		if (onConnectionSelect) {
			onConnectionSelect(fromBlockId, connectionId);
		}
	}

	interface Connection {
		id: string;
		path: string;
		type: 'nav' | 'manual';
		fromId: string;
		toId: string;
		isHighlighted: boolean;
		color?: string;
		arrowSize?: string;
		style?: string;
	}

	// Get handle position for a block and side
	function getHandlePosition(block: any, side: string) {
		// Try to get actual rendered dimensions from DOM (without zoom/pan transform)
		const element = document.querySelector(`[data-block-id="${block.id}"]`) as HTMLElement;
		let width = block.width || 200;
		let height = block.height || 60;

		if (element) {
			// Use offsetWidth/Height to get actual rendered size without transforms
			width = element.offsetWidth || width;
			height = element.offsetHeight || height;
		}

		const x = block.x;
		const y = block.y;

		switch (side) {
			case 'top':
				return { x: x + width / 2, y: y };
			case 'right':
				return { x: x + width, y: y + height / 2 };
			case 'bottom':
				return { x: x + width / 2, y: y + height };
			case 'left':
				return { x: x, y: y + height / 2 };
			default:
				return { x: x + width / 2, y: y + height / 2 };
		}
	}

	// Calculate all connections (only navigation and manual, NOT parent-child)
	const connections = $derived(() => {
		const lines: Connection[] = [];

		for (const [id, block] of blocks.entries()) {
			// Manual connections (Miro-style with specific handles)
			if (block.manualConnections && Array.isArray(block.manualConnections)) {
				for (const conn of block.manualConnections) {
					const target = blocks.get(conn.to);
					if (target) {
						const fromSide = conn.from || 'right';
						const toSide = conn.toSide || 'left';
						lines.push({
							id: conn.id || `${id}-${conn.to}`,
							path: getMiroPathFromHandles(block, target, fromSide, toSide),
							type: 'manual',
							fromId: id,
							toId: conn.to,
							isHighlighted: selectedBlockId === id,
							color: conn.color || '#89b4fa',
							arrowSize: conn.arrowSize || 'small',
							style: conn.style || 'solid'
						});
					}
				}
			}

			// Navigation → target connections
			if (block.type === 'nav-button' && block.targetContainerId) {
				const target = blocks.get(block.targetContainerId);
				if (target) {
					lines.push({
						id: `nav-${id}-${block.targetContainerId}`,
						path: getMiroPathFromCenters(block, target),
						type: 'nav',
						fromId: id,
						toId: block.targetContainerId,
						isHighlighted: selectedBlockId === id
					});
				}
			}
		}

		return lines;
	});

	// Generate orthogonal path from block centers (for parent-child)
	function getMiroPathFromCenters(from: any, to: any): string {
		// Start: center-bottom of source node
		const x1 = from.x + (from.width || 200) / 2;
		const y1 = from.y + (from.height || 60);

		// End: center-top of target node
		const x2 = to.x + (to.width || 200) / 2;
		const y2 = to.y;

		return generateOrthogonalPath(x1, y1, x2, y2, 'bottom', 'top');
	}

	// Get direction vector for a handle side
	function getHandleDirection(side: string): { dx: number; dy: number } {
		switch (side) {
			case 'top':
				return { dx: 0, dy: -1 };
			case 'right':
				return { dx: 1, dy: 0 };
			case 'bottom':
				return { dx: 0, dy: 1 };
			case 'left':
				return { dx: -1, dy: 0 };
			default:
				return { dx: 0, dy: 0 };
		}
	}

	// Generate orthogonal path from specific handles
	function getMiroPathFromHandles(from: any, to: any, fromSide: string, toSide: string): string {
		const startPos = getHandlePosition(from, fromSide);
		const endPos = getHandlePosition(to, toSide);

		return generateOrthogonalPath(
			startPos.x, startPos.y,
			endPos.x, endPos.y,
			fromSide,
			toSide
		);
	}

	// Generate orthogonal (right-angle) path between two points
	function generateOrthogonalPath(
		x1: number, y1: number,
		x2: number, y2: number,
		fromSide: string,
		toSide: string
	): string {
		const minSegmentLength = 40; // Minimum distance from handle before turning

		// Determine if we can make a simple 2-segment path
		const dx = x2 - x1;
		const dy = y2 - y1;

		let path = `M ${x1} ${y1}`;

		// Simple case: straight line if handles are aligned
		if (fromSide === 'bottom' && toSide === 'top' && Math.abs(dx) < 5) {
			path += ` L ${x2} ${y2}`;
			return path;
		}

		if (fromSide === 'right' && toSide === 'left' && Math.abs(dy) < 5) {
			path += ` L ${x2} ${y2}`;
			return path;
		}

		// Orthogonal routing based on handle directions
		if (fromSide === 'bottom' || fromSide === 'top') {
			// Start vertical
			const verticalExit = fromSide === 'bottom' ? minSegmentLength : -minSegmentLength;
			const midY = y1 + verticalExit;

			if (toSide === 'top' || toSide === 'bottom') {
				// End vertical - use 3 segments
				const verticalEntry = toSide === 'top' ? -minSegmentLength : minSegmentLength;
				const targetMidY = y2 + verticalEntry;
				const midX = (x1 + x2) / 2;

				path += ` L ${x1} ${midY}`;
				path += ` L ${midX} ${midY}`;
				path += ` L ${midX} ${targetMidY}`;
				path += ` L ${x2} ${targetMidY}`;
				path += ` L ${x2} ${y2}`;
			} else {
				// End horizontal - use 2 segments
				path += ` L ${x1} ${midY}`;
				path += ` L ${x2} ${midY}`;
				path += ` L ${x2} ${y2}`;
			}
		} else {
			// Start horizontal (left or right)
			const horizontalExit = fromSide === 'right' ? minSegmentLength : -minSegmentLength;
			const midX = x1 + horizontalExit;

			if (toSide === 'left' || toSide === 'right') {
				// End horizontal - use 3 segments
				const horizontalEntry = toSide === 'left' ? -minSegmentLength : minSegmentLength;
				const targetMidX = x2 + horizontalEntry;
				const midY = (y1 + y2) / 2;

				path += ` L ${midX} ${y1}`;
				path += ` L ${midX} ${midY}`;
				path += ` L ${targetMidX} ${midY}`;
				path += ` L ${targetMidX} ${y2}`;
				path += ` L ${x2} ${y2}`;
			} else {
				// End vertical - use 2 segments
				path += ` L ${midX} ${y1}`;
				path += ` L ${midX} ${y2}`;
				path += ` L ${x2} ${y2}`;
			}
		}

		return path;
	}
</script>

<svg class="connections-layer">
	<defs>
		<!-- Arrowhead marker for navigation lines -->
		<marker
			id="arrowhead-nav"
			markerWidth="10"
			markerHeight="7"
			refX="9"
			refY="3.5"
			orient="auto"
		>
			<polygon points="0 0, 10 3.5, 0 7" fill="#a6e3a1" />
		</marker>

		<!-- Arrowhead marker for manual connections -->
		<marker
			id="arrowhead-manual"
			markerWidth="8"
			markerHeight="6"
			refX="7"
			refY="3"
			orient="auto"
		>
			<polygon points="0 0, 8 3, 0 6" fill="#89b4fa" />
		</marker>

		<!-- Arrowhead marker for manual connections (highlighted) -->
		<marker
			id="arrowhead-manual-highlight"
			markerWidth="9"
			markerHeight="7"
			refX="8"
			refY="3.5"
			orient="auto"
		>
			<polygon points="0 0, 9 3.5, 0 7" fill="#667eea" />
		</marker>
	</defs>

	{#each connections() as conn (conn.id)}
		<!-- For manual connections, render an invisible wide path for clicking -->
		{#if conn.type === 'manual'}
			<path
				d={conn.path}
				class="connection-hitarea"
				onclick={(e) => handleConnectionClick(e, conn.fromId, conn.id)}
			/>
		{/if}

		<!-- The visible connection line -->
		<path
			d={conn.path}
			class="connection"
			class:parent={conn.type === 'parent'}
			class:nav={conn.type === 'nav'}
			class:manual={conn.type === 'manual'}
			class:highlighted={conn.isHighlighted}
			stroke={conn.color || undefined}
			marker-end={
				conn.type === 'manual'
					? (conn.isHighlighted ? 'url(#arrowhead-manual-highlight)' : 'url(#arrowhead-manual)')
					: conn.type === 'nav' && conn.isHighlighted
						? 'url(#arrowhead-nav)'
						: undefined
			}
		/>
	{/each}
</svg>

<style>
	.connections-layer {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
		transform-origin: 0 0;
		overflow: visible;
		z-index: 0;
	}

	.connection {
		stroke-width: 2;
		fill: none;
		transition: all 0.3s;
		pointer-events: none;
		stroke-linejoin: round;
		stroke-linecap: round;
	}

	/* Invisible wide path for clicking manual connections */
	.connection-hitarea {
		stroke: transparent;
		stroke-width: 16;
		fill: none;
		pointer-events: stroke;
		cursor: pointer;
	}

	.connection-hitarea:hover + .connection.manual {
		stroke-width: 4;
		filter: drop-shadow(0 0 4px currentColor);
	}

	/* Navigation: dashed green */
	.connection.nav {
		stroke: #a6e3a1;
		stroke-dasharray: 6 4;
		opacity: 0.4;
	}

	/* Highlighted navigation: animated! */
	.connection.nav.highlighted {
		stroke: #a6e3a1;
		stroke-width: 3;
		opacity: 1;
		animation: flow 1.5s linear infinite;
	}

	/* Manual connections: solid blue with arrow */
	.connection.manual {
		stroke: #89b4fa;
		stroke-width: 3;
		opacity: 0.7;
	}

	/* Highlighted manual: purple glow */
	.connection.manual.highlighted {
		stroke: #667eea;
		stroke-width: 4;
		opacity: 1;
		filter: drop-shadow(0 0 4px rgba(102, 126, 234, 0.6));
	}

	@keyframes flow {
		to {
			stroke-dashoffset: -20;
		}
	}
</style>
