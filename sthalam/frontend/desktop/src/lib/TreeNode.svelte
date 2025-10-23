<script lang="ts">
	import {
		getBlockIcon,
		getBlockColor,
		getContentPreview,
		canHaveChildren,
		getChildCount
	} from "../utils/blockIcons";

	interface Props {
		block: any;
		depth: number;
		isExpanded: boolean;
		isSelected: boolean;
		onToggle: (e: MouseEvent) => void;
		onSelect: () => void;
	}

	let { block, depth, isExpanded, isSelected, onToggle, onSelect }: Props = $props();

	const hasChildren = $derived(canHaveChildren(block.type) && getChildCount(block) > 0);
	const icon = $derived(getBlockIcon(block.type));
	const preview = $derived(getContentPreview(block));
	const childCount = $derived(getChildCount(block));
	const accentColor = $derived(getBlockColor(block.type));

	function handleClick(e: MouseEvent) {
		// Prevent event bubbling
		e.stopPropagation();
		onSelect();
	}

	function handleToggleClick(e: MouseEvent) {
		e.stopPropagation();
		onToggle(e);
	}
</script>

<div class="tree-node-container" style:padding-left="{depth * 20}px">
	<!-- Node content wrapper -->
	<div
		class="tree-node"
		class:selected={isSelected}
		onclick={handleClick}
		role="button"
		tabindex="0"
	>
		<!-- Expand/collapse toggle -->
		{#if hasChildren}
			<button
				class="toggle"
				onclick={handleToggleClick}
				aria-label={isExpanded ? "Collapse" : "Expand"}
			>
				{isExpanded ? "▼" : "▶"}
			</button>
		{:else}
			<span class="toggle-spacer"></span>
		{/if}

		<!-- Block icon -->
		<span class="icon" style:color={accentColor}>{icon}</span>

		<!-- Content preview -->
		<span class="label" title={preview}>{preview}</span>

		<!-- Child count badge -->
		{#if hasChildren}
			<span class="badge">({childCount})</span>
		{/if}

		<!-- Entry point badge for screens -->
		{#if block.type === "screen-container" && block.isEntryPoint}
			<span class="entry-badge">Entry</span>
		{/if}
	</div>
</div>

<style>
	.tree-node-container {
		position: relative;
		height: 32px;
		display: flex;
		align-items: center;
	}

	.tree-node {
		display: flex;
		align-items: center;
		height: 100%;
		padding: 4px 8px 4px 4px;
		cursor: pointer;
		user-select: none;
		background: transparent;
		border-bottom: 1px solid rgba(33, 38, 45, 0.5);
		transition: background 0.1s ease;
		position: relative;
		border-radius: 4px;
	}

	.tree-node:hover {
		background: rgba(33, 38, 45, 0.8);
	}

	.tree-node.selected {
		background: #30363d;
		border-left: 3px solid #89b4fa;
		padding-left: 1px;
	}

	.toggle {
		width: 16px;
		height: 16px;
		padding: 0;
		margin-right: 4px;
		background: none;
		border: none;
		color: #6e7681;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 10px;
		transition: color 0.1s ease;
	}

	.toggle:hover {
		color: #89b4fa;
	}

	.toggle-spacer {
		width: 20px;
		display: inline-block;
	}

	.icon {
		margin-right: 8px;
		font-size: 14px;
		flex-shrink: 0;
	}

	.label {
		flex: 1;
		font-size: 13px;
		color: #c9d1d9;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans", Helvetica, Arial,
			sans-serif;
		/* Optimize text for Tauri WebView */
		-webkit-font-smoothing: antialiased;
		text-rendering: optimizeLegibility;
	}

	.badge {
		margin-left: 8px;
		padding: 2px 6px;
		background: #21262d;
		border-radius: 10px;
		font-size: 11px;
		color: #6e7681;
		flex-shrink: 0;
	}

	.entry-badge {
		margin-left: 8px;
		padding: 2px 6px;
		background: #89b4fa;
		color: #010409;
		border-radius: 4px;
		font-size: 10px;
		font-weight: 600;
		flex-shrink: 0;
	}
</style>
