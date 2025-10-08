<script lang="ts">
	let showHelp = $state(false);
</script>

<button class="help-button" onclick={() => showHelp = !showHelp} title="Keyboard shortcuts">
	⌨️
</button>

{#if showHelp}
	<div class="help-panel">
		<div class="help-header">
			<h3>Keyboard Shortcuts</h3>
			<button class="close-btn" onclick={() => showHelp = false}>✕</button>
		</div>
		<div class="help-content">
			<div class="shortcut-group">
				<h4>Moving Blocks</h4>
				<div class="shortcut">
					<kbd>Ctrl</kbd> + <kbd>Click</kbd>
					<span>Drag block</span>
				</div>
			</div>

			<div class="shortcut-group">
				<h4>Resizing Blocks</h4>
				<div class="shortcut">
					<kbd>Drag handle</kbd>
					<span>Resize freely</span>
				</div>
				<div class="shortcut highlight">
					<kbd>Shift</kbd> + <kbd>Drag border</kbd>
					<span>Resize from edge 🆕</span>
				</div>
				<div class="shortcut-note">
					💡 No scrollbar conflict! Hover near edge + hold Shift
				</div>
			</div>

			<div class="shortcut-group">
				<h4>Editing</h4>
				<div class="shortcut">
					<kbd>Click text</kbd>
					<span>Edit content</span>
				</div>
				<div class="shortcut">
					<kbd>Select block</kbd>
					<span>Edit in Properties →</span>
				</div>
			</div>
		</div>
	</div>
{/if}

<style>
	.help-button {
		position: fixed;
		bottom: 20px;
		right: 20px;
		width: 50px;
		height: 50px;
		border-radius: 50%;
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
		border: none;
		font-size: 24px;
		cursor: pointer;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
		transition: transform 0.2s, box-shadow 0.2s;
		z-index: 1000;
	}

	.help-button:hover {
		transform: scale(1.1);
		box-shadow: 0 6px 16px rgba(0, 0, 0, 0.3);
	}

	.help-panel {
		position: fixed;
		bottom: 80px;
		right: 20px;
		width: 320px;
		background: var(--bg-primary, white);
		border: 1px solid var(--border-color, #e0e0e0);
		border-radius: 12px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.15);
		z-index: 1000;
		overflow: hidden;
	}

	.help-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 1rem;
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
	}

	.help-header h3 {
		margin: 0;
		font-size: 1rem;
		font-weight: 600;
	}

	.close-btn {
		background: rgba(255, 255, 255, 0.2);
		border: none;
		color: white;
		width: 28px;
		height: 28px;
		border-radius: 50%;
		cursor: pointer;
		font-size: 16px;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: background 0.2s;
	}

	.close-btn:hover {
		background: rgba(255, 255, 255, 0.3);
	}

	.help-content {
		padding: 1rem;
		max-height: 400px;
		overflow-y: auto;
	}

	.shortcut-group {
		margin-bottom: 1.5rem;
	}

	.shortcut-group:last-child {
		margin-bottom: 0;
	}

	.shortcut-group h4 {
		margin: 0 0 0.5rem 0;
		font-size: 0.875rem;
		color: var(--text-secondary, #666);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		font-weight: 600;
	}

	.shortcut {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0;
		border-bottom: 1px solid var(--border-color, #e0e0e0);
	}

	.shortcut:last-child {
		border-bottom: none;
	}

	kbd {
		background: var(--bg-secondary, #f5f5f5);
		border: 1px solid var(--border-color, #e0e0e0);
		border-radius: 4px;
		padding: 0.25rem 0.5rem;
		font-family: monospace;
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--text-primary, #333);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
	}

	.shortcut span {
		flex: 1;
		font-size: 0.875rem;
		color: var(--text-primary, #333);
	}

	.shortcut.highlight {
		background: linear-gradient(90deg, rgba(102, 126, 234, 0.1) 0%, transparent 100%);
		padding: 0.75rem 0.5rem;
		margin: 0 -0.5rem;
		border-radius: 4px;
		border-left: 3px solid #667eea;
	}

	.shortcut-note {
		font-size: 0.75rem;
		color: var(--text-secondary, #666);
		padding: 0.5rem;
		background: var(--bg-secondary, #f5f5f5);
		border-radius: 4px;
		margin-top: 0.5rem;
		text-align: center;
	}
</style>
