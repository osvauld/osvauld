<script lang="ts">
	import { open } from '@tauri-apps/plugin-dialog';
	import { readFile } from '@tauri-apps/plugin-fs';

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
		onBringToFront?: (blockId: string) => void;
		onSendToBack?: (blockId: string) => void;
	}

	let { selectedBlock, onUpdateBlock, onBringForward, onSendBackward, onBringToFront, onSendToBack }: Props = $props();

	// Collapsible sections state
	let expandedSections = $state({
		dimensions: true,
		content: true,
		text: false,
		appearance: false,
		layering: false,
	});

	function toggleSection(section: keyof typeof expandedSections) {
		expandedSections[section] = !expandedSections[section];
	}

	// Resizable panel width
	let panelWidth = $state(280);
	let isResizing = $state(false);
	let resizeStartX = $state(0);
	let resizeStartWidth = $state(0);

	function handleResizeStart(e: MouseEvent) {
		isResizing = true;
		resizeStartX = e.clientX;
		resizeStartWidth = panelWidth;
		e.preventDefault();
	}

	function handleResizeMove(e: MouseEvent) {
		if (isResizing) {
			const dx = resizeStartX - e.clientX; // Inverted because we're resizing from the left
			const newWidth = Math.max(200, Math.min(600, resizeStartWidth + dx));
			panelWidth = newWidth;
		}
	}

	function handleResizeEnd() {
		isResizing = false;
	}

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

	// Form field management
	const formConfig = $derived(
		selectedBlock?.type === "form" && selectedBlock.content
			? (() => {
				try {
					return JSON.parse(selectedBlock.content);
				} catch {
					return { fields: [], submitButtonText: "Submit" };
				}
			})()
			: { fields: [], submitButtonText: "Submit" }
	);

	const formFields = $derived(formConfig.fields || []);

	function updateFormField(index: number, key: string, value: any) {
		const updatedFields = [...formConfig.fields];
		updatedFields[index] = { ...updatedFields[index], [key]: value };

		onUpdateBlock(selectedBlock!.id, {
			content: JSON.stringify({ ...formConfig, fields: updatedFields })
		});
	}

	function addFormField() {
		const newField = {
			id: `field-${Date.now()}`,
			type: "text",
			label: "New Field",
			placeholder: "",
			required: false
		};

		onUpdateBlock(selectedBlock!.id, {
			content: JSON.stringify({ ...formConfig, fields: [...formConfig.fields, newField] })
		});
	}

	function removeFormField(index: number) {
		const updatedFields = formConfig.fields.filter((_: any, i: number) => i !== index);

		onUpdateBlock(selectedBlock!.id, {
			content: JSON.stringify({ ...formConfig, fields: updatedFields })
		});
	}

	function updateSubmitButtonText(text: string) {
		onUpdateBlock(selectedBlock!.id, {
			content: JSON.stringify({ ...formConfig, submitButtonText: text })
		});
	}

	async function handleImageUpload() {
		try {
			const selected = await open({
				multiple: false,
				filters: [{
					name: 'Images',
					extensions: ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp']
				}]
			});

			if (selected && selectedBlock) {
				// Read file as bytes
				const fileContent = await readFile(selected);

				// Convert to base64
				const base64 = btoa(
					new Uint8Array(fileContent).reduce(
						(data, byte) => data + String.fromCharCode(byte),
						''
					)
				);

				// Determine MIME type from extension
				const ext = selected.split('.').pop()?.toLowerCase();
				const mimeTypes: Record<string, string> = {
					'png': 'image/png',
					'jpg': 'image/jpeg',
					'jpeg': 'image/jpeg',
					'gif': 'image/gif',
					'svg': 'image/svg+xml',
					'webp': 'image/webp'
				};
				const mimeType = mimeTypes[ext || 'png'] || 'image/png';

				// Create data URL
				const dataUrl = `data:${mimeType};base64,${base64}`;

				// Update block content with data URL
				onUpdateBlock(selectedBlock.id, { content: dataUrl });
			}
		} catch (error) {
			console.error('Failed to upload image:', error);
		}
	}
</script>

<svelte:window onmousemove={handleResizeMove} onmouseup={handleResizeEnd} />

<div class="properties-panel" style:width="{panelWidth}px" class:resizing={isResizing}>
	<div class="resize-handle" onmousedown={handleResizeStart}></div>
	{#if selectedBlock}
		<div class="panel-header">
			<h3>Properties</h3>
			<span class="block-type-badge">{selectedBlock.type}</span>
		</div>

		<div class="panel-content">
			<!-- Position & Size -->
			<div class="property-group">
				<button class="section-header" onclick={() => toggleSection('dimensions')}>
					<span class="section-toggle">{expandedSections.dimensions ? '▼' : '▶'}</span>
					<h4>Dimensions</h4>
				</button>
				{#if expandedSections.dimensions}
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
				{/if}
			</div>

			<!-- Content Section (Image or HTML) -->
			{#if selectedBlock.type === "image" || selectedBlock.type === "html"}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>{selectedBlock.type === "image" ? "Image" : "HTML/CSS"}</h4>
					</button>
					{#if expandedSections.content}
						{#if selectedBlock.type === "image"}
							<button class="upload-btn" onclick={handleImageUpload}>
								📁 Upload Image/GIF
							</button>
							<div class="divider">
								<span>or paste URL</span>
							</div>
							<label>
								<span>Image URL</span>
								<input
									type="text"
									value={selectedBlock.content || ""}
									oninput={(e) => onUpdateBlock(selectedBlock.id, { content: e.currentTarget.value })}
									placeholder="https://example.com/image.png"
								/>
							</label>
							<div class="info-box">
								💡 Upload: JPG, PNG, GIF, SVG, WebP
							</div>
						{:else if selectedBlock.type === "html"}
							<label>
								<span>HTML Code</span>
								<textarea
									value={selectedBlock.content || ""}
									oninput={(e) => onUpdateBlock(selectedBlock.id, { content: e.currentTarget.value })}
									placeholder="<div>Your HTML here...</div>"
									rows="10"
								></textarea>
							</label>
							<div class="info-box">
								⚠️ JavaScript is disabled for security
							</div>
							<label>
								<span>CSS Code</span>
								<textarea
									value={selectedBlock.styles.css || ""}
									oninput={(e) => updateStyle("css", e.currentTarget.value)}
									placeholder=".my-class &#123; color: blue; &#125;"
									rows="10"
								></textarea>
							</label>
							<div class="info-box">
								💡 Styles apply to HTML above
							</div>
						{/if}
					{/if}
				</div>
			{/if}

			<!-- Form Builder -->
			{#if selectedBlock.type === "form"}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>Form Builder</h4>
					</button>
					{#if expandedSections.content}
						<div class="form-builder">
							<h4>Form Fields</h4>
							{#each formFields as field, index}
								<div class="form-field-item">
									<div class="field-header">
										<span class="field-number">#{index + 1}</span>
										<select
											value={field.type}
											onchange={(e) => updateFormField(index, 'type', e.currentTarget.value)}
										>
											<option value="text">Text</option>
											<option value="email">Email</option>
											<option value="number">Number</option>
											<option value="textarea">Textarea</option>
											<option value="checkbox">Checkbox</option>
										</select>
										<button class="field-delete" onclick={() => removeFormField(index)}>✕</button>
									</div>
									<label>
										<span>Label</span>
										<input
											type="text"
											value={field.label}
											oninput={(e) => updateFormField(index, 'label', e.currentTarget.value)}
										/>
									</label>
									<label>
										<span>Placeholder</span>
										<input
											type="text"
											value={field.placeholder}
											oninput={(e) => updateFormField(index, 'placeholder', e.currentTarget.value)}
										/>
									</label>
									<label class="checkbox-label">
										<input
											type="checkbox"
											checked={field.required}
											onchange={(e) => updateFormField(index, 'required', e.currentTarget.checked)}
										/>
										<span>Required</span>
									</label>
								</div>
							{/each}
							<button class="add-field-btn" onclick={addFormField}>+ Add Field</button>

							<label>
								<span>Submit Button Text</span>
								<input
									type="text"
									value={formConfig.submitButtonText}
									oninput={(e) => updateSubmitButtonText(e.currentTarget.value)}
								/>
							</label>
						</div>
					{/if}
				</div>
			{/if}

			<!-- Text Styles -->
			{#if selectedBlock.type === "heading" || selectedBlock.type === "text"}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('text')}>
						<span class="section-toggle">{expandedSections.text ? '▼' : '▶'}</span>
						<h4>Text</h4>
					</button>
					{#if expandedSections.text}
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
					{/if}
				</div>
			{/if}

			<!-- Background & Border -->
			<div class="property-group">
				<button class="section-header" onclick={() => toggleSection('appearance')}>
					<span class="section-toggle">{expandedSections.appearance ? '▼' : '▶'}</span>
					<h4>Appearance</h4>
				</button>
				{#if expandedSections.appearance}
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
				{/if}
			</div>

			<!-- Layering -->
			<div class="property-group">
				<button class="section-header" onclick={() => toggleSection('layering')}>
					<span class="section-toggle">{expandedSections.layering ? '▼' : '▶'}</span>
					<h4>Layering</h4>
				</button>
				{#if expandedSections.layering}
					<div class="layer-indicator">
						<div class="z-index-badge">
							Layer {selectedBlock.zIndex}
						</div>
						<div class="layer-type">
							{selectedBlock.type}
						</div>
					</div>
					<div class="button-grid">
						{#if onBringToFront}
							<button class="primary-btn" onclick={() => onBringToFront?.(selectedBlock.id)}>
								⬆️ To Front
							</button>
						{/if}
						<button onclick={() => onBringForward(selectedBlock.id)}>
							↑ Forward
						</button>
						<button onclick={() => onSendBackward(selectedBlock.id)}>
							↓ Backward
						</button>
						{#if onSendToBack}
							<button class="primary-btn" onclick={() => onSendToBack?.(selectedBlock.id)}>
								⬇️ To Back
							</button>
						{/if}
					</div>
					{#if selectedBlock.type === "container"}
						<div class="layer-hint">
							💡 Use "⬇️ To Back" for containers
						</div>
					{/if}
				{/if}
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
		height: 100%;
		background: var(--bg-primary, #ffffff);
		border-left: 1px solid var(--border-color, #e0e0e0);
		display: flex;
		flex-direction: column;
		position: relative;
	}

	.resize-handle {
		position: absolute;
		left: 0;
		top: 0;
		bottom: 0;
		width: 5px;
		cursor: col-resize;
		background: transparent;
		z-index: 100;
		transition: background 0.2s;
	}

	.resize-handle:hover {
		background: #667eea;
	}

	.resize-handle:active {
		background: #5568d3;
	}

	.properties-panel.resizing {
		user-select: none;
	}

	.properties-panel.resizing .resize-handle {
		background: #667eea;
	}

	.panel-header {
		padding: 1rem;
		border-bottom: 1px solid var(--border-color, #e0e0e0);
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.panel-header h3 {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--text-primary, #333);
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

	.panel-content::-webkit-scrollbar {
		width: 8px;
	}

	.panel-content::-webkit-scrollbar-track {
		background: var(--bg-secondary, #f5f5f5);
	}

	.panel-content::-webkit-scrollbar-thumb {
		background: var(--border-color, #e0e0e0);
		border-radius: 4px;
	}

	.panel-content::-webkit-scrollbar-thumb:hover {
		background: #999;
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
		color: var(--text-muted, #999);
		font-size: 0.875rem;
	}

	.property-group {
		margin-bottom: 1rem;
		border-bottom: 1px solid var(--border-color, #e0e0e0);
		padding-bottom: 0.5rem;
	}

	.section-header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		width: 100%;
		padding: 0.5rem 0;
		background: transparent;
		border: none;
		cursor: pointer;
		text-align: left;
		transition: background 0.2s;
	}

	.section-header:hover {
		background: var(--bg-hover, #f5f5ff);
		border-radius: 4px;
	}

	.section-toggle {
		font-size: 0.75rem;
		color: var(--text-secondary, #666);
		transition: transform 0.2s;
	}

	.property-group h4 {
		margin: 0;
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--text-secondary, #666);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		flex: 1;
	}

	label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		margin-bottom: 0.75rem;
	}

	label span {
		font-size: 0.75rem;
		color: var(--text-secondary, #666);
		font-weight: 500;
	}

	input[type="text"],
	input[type="number"],
	select,
	textarea {
		padding: 0.5rem;
		border: 1px solid var(--border-color, #e0e0e0);
		border-radius: 4px;
		font-size: 0.875rem;
		background: var(--bg-primary, white);
		color: var(--text-primary, #333);
		transition: border-color 0.2s;
		font-family: monospace;
	}

	input[type="text"]:focus,
	input[type="number"]:focus,
	select:focus,
	textarea:focus {
		outline: none;
		border-color: #667eea;
	}

	textarea {
		resize: vertical;
		line-height: 1.5;
	}

	input[type="color"] {
		width: 100%;
		height: 40px;
		border: 1px solid var(--border-color, #e0e0e0);
		border-radius: 4px;
		cursor: pointer;
	}

	.property-row {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}

	.button-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}

	.button-grid button,
	.upload-btn {
		padding: 0.5rem;
		border: 1px solid var(--border-color, #e0e0e0);
		border-radius: 4px;
		background: var(--bg-primary, white);
		color: var(--text-primary, #333);
		font-size: 0.75rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	.button-grid button:hover {
		background: var(--bg-hover, #f5f5ff);
		border-color: #667eea;
	}

	.button-grid button.primary-btn {
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
		border: none;
		font-weight: 600;
	}

	.button-grid button.primary-btn:hover {
		transform: translateY(-1px);
		box-shadow: 0 2px 8px rgba(102, 126, 234, 0.3);
	}

	.layer-hint {
		margin-top: 0.5rem;
		padding: 0.5rem;
		background: #fff3cd;
		border-left: 3px solid #ffc107;
		border-radius: 4px;
		font-size: 0.75rem;
		color: #856404;
	}

	.layer-indicator {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem;
		background: var(--bg-secondary, #f5f5f5);
		border-radius: 6px;
		margin-bottom: 0.75rem;
	}

	.z-index-badge {
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
		padding: 0.25rem 0.75rem;
		border-radius: 20px;
		font-size: 0.75rem;
		font-weight: 700;
	}

	.layer-type {
		font-size: 0.875rem;
		color: var(--text-secondary, #666);
		font-weight: 500;
		text-transform: capitalize;
	}

	.info-box {
		background: #e3f2fd;
		padding: 0.75rem;
		border-radius: 4px;
		font-size: 0.75rem;
		color: #1976d2;
		margin-top: 0.5rem;
	}

	.upload-btn {
		width: 100%;
		padding: 0.75rem !important;
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%) !important;
		color: white !important;
		border: none !important;
		border-radius: 6px;
		font-weight: 600;
		cursor: pointer;
		transition: transform 0.2s, box-shadow 0.2s;
		margin-bottom: 1rem;
	}

	.upload-btn:hover {
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);
	}

	.divider {
		display: flex;
		align-items: center;
		text-align: center;
		margin: 1rem 0;
	}

	.divider::before,
	.divider::after {
		content: '';
		flex: 1;
		border-bottom: 1px solid var(--border-color, #e0e0e0);
	}

	.divider span {
		padding: 0 0.5rem;
		color: var(--text-muted, #999);
		font-size: 0.75rem;
	}

	/* Form Builder */
	.form-builder {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.form-builder h4 {
		margin: 0 0 0.5rem 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--text-primary, #333);
	}

	.form-field-item {
		padding: 12px;
		background: #f8f9fa;
		border-radius: 6px;
		border: 1px solid #e0e0e0;
		margin-bottom: 8px;
	}

	.field-header {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 12px;
	}

	.field-number {
		font-size: 0.75rem;
		font-weight: 600;
		color: #667eea;
		min-width: 24px;
	}

	.field-header select {
		flex: 1;
	}

	.field-delete {
		width: 24px;
		height: 24px;
		padding: 0;
		background: #dc3545;
		color: white;
		border: none;
		border-radius: 4px;
		cursor: pointer;
		font-size: 0.875rem;
		transition: background 0.2s;
	}

	.field-delete:hover {
		background: #c82333;
	}

	.checkbox-label {
		flex-direction: row !important;
		align-items: center;
		gap: 8px;
	}

	.checkbox-label input[type="checkbox"] {
		width: auto;
		height: auto;
		margin: 0;
	}

	.add-field-btn {
		padding: 10px;
		background: #28a745;
		color: white;
		border: none;
		border-radius: 6px;
		font-weight: 600;
		cursor: pointer;
		transition: background 0.2s;
	}

	.add-field-btn:hover {
		background: #218838;
	}
</style>
