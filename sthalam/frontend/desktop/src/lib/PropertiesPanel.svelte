<script lang="ts">
	interface Block {
		id: string;
		type: string;
		x: number;
		y: number;
		width: number;
		height: number;
		zIndex: number;
		content: string;
		styles: Record<string, string>;
	}

	interface Props {
		selectedBlock: Block | null;
		onUpdateBlock: (blockId: string, updates: Partial<Block>) => void;
		onBringForward: (blockId: string) => void;
		onSendBackward: (blockId: string) => void;
	}

	let { selectedBlock, onUpdateBlock, onBringForward, onSendBackward }: Props = $props();

	function updateStyle(key: string, value: string) {
		if (selectedBlock) {
			onUpdateBlock(selectedBlock.id, {
				styles: { ...selectedBlock.styles, [key]: value },
			});
		}
	}

	function updateDimension(key: "width" | "height", value: number) {
		if (selectedBlock) {
			onUpdateBlock(selectedBlock.id, { [key]: value });
		}
	}
</script>

<div class="properties-panel">
	{#if selectedBlock}
		<div class="panel-header">
			<h3>Properties</h3>
			<span class="block-type-badge">{selectedBlock.type}</span>
		</div>

		<div class="panel-content">
			<!-- Position & Size -->
			<div class="property-group">
				<h4>Dimensions</h4>
				<div class="property-row">
					<label>
						<span>Width</span>
						<input
							type="number"
							value={selectedBlock.width}
							oninput={(e) => updateDimension("width", parseInt(e.currentTarget.value))}
						/>
					</label>
					<label>
						<span>Height</span>
						<input
							type="number"
							value={selectedBlock.height}
							oninput={(e) => updateDimension("height", parseInt(e.currentTarget.value))}
						/>
					</label>
				</div>
			</div>

			<!-- Text Styles -->
			{#if selectedBlock.type === "heading" || selectedBlock.type === "text"}
				<div class="property-group">
					<h4>Text</h4>
					<label>
						<span>Font Size</span>
						<input
							type="text"
							value={selectedBlock.styles.fontSize || "16px"}
							oninput={(e) => updateStyle("fontSize", e.currentTarget.value)}
							placeholder="16px"
						/>
					</label>
					<label>
						<span>Font Weight</span>
						<select
							value={selectedBlock.styles.fontWeight || "400"}
							onchange={(e) => updateStyle("fontWeight", e.currentTarget.value)}
						>
							<option value="300">Light</option>
							<option value="400">Normal</option>
							<option value="600">Semi-bold</option>
							<option value="700">Bold</option>
						</select>
					</label>
					<label>
						<span>Color</span>
						<input
							type="color"
							value={selectedBlock.styles.color || "#333333"}
							oninput={(e) => updateStyle("color", e.currentTarget.value)}
						/>
					</label>
				</div>
			{/if}

			<!-- Background & Border -->
			<div class="property-group">
				<h4>Appearance</h4>
				<label>
					<span>Background</span>
					<input
						type="color"
						value={selectedBlock.styles.backgroundColor || "#ffffff"}
						oninput={(e) => updateStyle("backgroundColor", e.currentTarget.value)}
					/>
				</label>
				<label>
					<span>Border</span>
					<input
						type="text"
						value={selectedBlock.styles.border || "2px solid #ddd"}
						oninput={(e) => updateStyle("border", e.currentTarget.value)}
						placeholder="2px solid #ddd"
					/>
				</label>
				<label>
					<span>Border Radius</span>
					<input
						type="text"
						value={selectedBlock.styles.borderRadius || "4px"}
						oninput={(e) => updateStyle("borderRadius", e.currentTarget.value)}
						placeholder="4px"
					/>
				</label>
				<label>
					<span>Padding</span>
					<input
						type="text"
						value={selectedBlock.styles.padding || "12px"}
						oninput={(e) => updateStyle("padding", e.currentTarget.value)}
						placeholder="12px"
					/>
				</label>
			</div>

			<!-- Layering -->
			<div class="property-group">
				<h4>Layering</h4>
				<div class="button-group">
					<button onclick={() => onBringForward(selectedBlock.id)}>
						Bring Forward
					</button>
					<button onclick={() => onSendBackward(selectedBlock.id)}>
						Send Backward
					</button>
				</div>
			</div>
		</div>
	{:else}
		<div class="panel-empty">
			<p>Select a block to edit its properties</p>
		</div>
	{/if}
</div>

<style>
	.properties-panel {
		width: 280px;
		height: 100%;
		background: #ffffff;
		border-left: 1px solid #e0e0e0;
		display: flex;
		flex-direction: column;
	}

	.panel-header {
		padding: 1rem;
		border-bottom: 1px solid #e0e0e0;
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.panel-header h3 {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: #333;
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.block-type-badge {
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
		padding: 0.25rem 0.5rem;
		border-radius: 4px;
		font-size: 0.75rem;
		font-weight: 600;
		text-transform: uppercase;
	}

	.panel-content {
		flex: 1;
		padding: 1rem;
		overflow-y: auto;
	}

	.panel-empty {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2rem;
		text-align: center;
	}

	.panel-empty p {
		color: #999;
		font-size: 0.875rem;
	}

	.property-group {
		margin-bottom: 1.5rem;
	}

	.property-group h4 {
		margin: 0 0 0.75rem 0;
		font-size: 0.75rem;
		font-weight: 600;
		color: #666;
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		margin-bottom: 0.75rem;
	}

	label span {
		font-size: 0.75rem;
		color: #666;
		font-weight: 500;
	}

	input[type="text"],
	input[type="number"],
	select {
		padding: 0.5rem;
		border: 1px solid #e0e0e0;
		border-radius: 4px;
		font-size: 0.875rem;
		transition: border-color 0.2s;
	}

	input[type="text"]:focus,
	input[type="number"]:focus,
	select:focus {
		outline: none;
		border-color: #667eea;
	}

	input[type="color"] {
		width: 100%;
		height: 40px;
		border: 1px solid #e0e0e0;
		border-radius: 4px;
		cursor: pointer;
	}

	.property-row {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}

	.button-group {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	button {
		padding: 0.5rem;
		border: 1px solid #e0e0e0;
		border-radius: 4px;
		background: white;
		font-size: 0.75rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	button:hover {
		background: #f5f5ff;
		border-color: #667eea;
	}
</style>
