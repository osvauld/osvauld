<script lang="ts">
	import { open } from '@tauri-apps/plugin-dialog';
	import { readFile } from '@tauri-apps/plugin-fs';
	import { BinIcon } from "@osvauld/icons";

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
		name?: string;
		css?: string;
		targetContainerId?: string;
		formId?: string;
		fieldName?: string;
		value?: any;
		action?: string;
	}

	interface Props {
		selectedBlock: Block | null;
		blocks?: Map<string, any>;
		selectedConnection?: any | null;
		selectedConnectionId?: string | null;
		onUpdateBlock: (blockId: string, updates: Partial<Block>) => void;
		onDeleteBlock?: (blockId: string) => void;
		onEditContent?: (blockId: string, content: string, type: string) => void;
	}

	let { selectedBlock, blocks, selectedConnection, selectedConnectionId, onUpdateBlock, onDeleteBlock, onEditContent }: Props = $props();

	// Get all blocks as array for dropdowns
	const allBlocks = $derived(blocks ? Array.from(blocks.entries()).map(([id, block]) => ({ id, ...block })) : []);

	// Get all forms for form field dropdowns
	const allForms = $derived(allBlocks.filter(b => b.type === 'form'));

	// Get all screen and section containers for parent selection
	const allScreenContainers = $derived(allBlocks.filter(b => b.type === 'screen-container'));
	const allSectionContainers = $derived(allBlocks.filter(b => b.type === 'section-container'));

	// Combined list of all valid parent containers
	const allParentContainers = $derived([...allScreenContainers, ...allSectionContainers]);

	// Find branching question in same container as selected block (for nav buttons)
	const branchingQuestionInContainer = $derived(() => {
		if (!selectedBlock || selectedBlock.type !== 'nav-button') return null;

		// Find which container this nav button is in
		const container = allBlocks.find(b => {
			if (b.type !== 'form-container' && b.type !== 'container') return false;

			return selectedBlock.x >= b.x &&
				selectedBlock.x + selectedBlock.width <= b.x + b.width &&
				selectedBlock.y >= b.y &&
				selectedBlock.y + selectedBlock.height <= b.y + b.height;
		});

		if (!container) return null;

		// Find branching question in the same container
		return allBlocks.find(b => {
			if (b.type !== 'branching-question') return false;

			return b.x >= container.x &&
				b.x + b.width <= container.x + container.width &&
				b.y >= container.y &&
				b.y + b.height <= container.y + container.height;
		});
	});

	// Collapsible sections state
	let expandedSections = $state({
		hierarchy: true,
		content: true,
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
			<div class="header-actions">
				{#if selectedConnectionId}
					<span class="block-type-badge connection-badge">Connection</span>
				{:else}
					<span class="block-type-badge">{selectedBlock.type}</span>
				{/if}
				{#if onDeleteBlock && !selectedConnectionId}
					<button class="delete-btn" onclick={() => onDeleteBlock?.(selectedBlock.id)} title="Delete block (Delete key)">
						<BinIcon size={16} color="white" />
					</button>
				{/if}
			</div>
		</div>

		<div class="panel-content">
			<!-- Connection Properties (when connection is selected) -->
			{#if selectedConnection && selectedConnectionId}
				<div class="property-group">
					<h4>Connection Properties</h4>
					<div class="connection-info">
						<p><strong>ID:</strong> {selectedConnectionId}</p>
						<p><strong>From:</strong> {selectedBlock.id} ({selectedConnection.from} handle)</p>
						<p><strong>To:</strong> {selectedConnection.to} ({selectedConnection.toSide} handle)</p>
					</div>
					<label>
						<span>Color</span>
						<input
							type="color"
							value={selectedConnection.color || '#89b4fa'}
							oninput={(e) => {
								const connections = selectedBlock.manualConnections || [];
								const index = connections.findIndex((c: any) => c.id === selectedConnectionId);
								if (index >= 0) {
									connections[index].color = e.currentTarget.value;
									onUpdateBlock(selectedBlock.id, { manualConnections: connections });
								}
							}}
						/>
					</label>
					<label>
						<span>Arrow Size</span>
						<select
							value={selectedConnection.arrowSize || 'small'}
							onchange={(e) => {
								const connections = selectedBlock.manualConnections || [];
								const index = connections.findIndex((c: any) => c.id === selectedConnectionId);
								if (index >= 0) {
									connections[index].arrowSize = e.currentTarget.value;
									onUpdateBlock(selectedBlock.id, { manualConnections: connections });
								}
							}}
						>
							<option value="small">Small</option>
							<option value="medium">Medium</option>
							<option value="large">Large</option>
						</select>
					</label>
					<label>
						<span>Style</span>
						<select
							value={selectedConnection.style || 'solid'}
							onchange={(e) => {
								const connections = selectedBlock.manualConnections || [];
								const index = connections.findIndex((c: any) => c.id === selectedConnectionId);
								if (index >= 0) {
									connections[index].style = e.currentTarget.value;
									onUpdateBlock(selectedBlock.id, { manualConnections: connections });
								}
							}}
						>
							<option value="solid">Solid</option>
							<option value="dashed">Dashed</option>
							<option value="dotted">Dotted</option>
						</select>
					</label>
				</div>
			{/if}
			<!-- Parent Container & CSS (for all blocks except screen-container) -->
			{#if selectedBlock.type !== 'screen-container'}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('hierarchy')}>
						<span class="section-toggle">{expandedSections.hierarchy ? '▼' : '▶'}</span>
						<h4>Hierarchy & Layout</h4>
					</button>
					{#if expandedSections.hierarchy}
						<label>
							<span>Parent Container</span>
							<select
								value={selectedBlock.parentId || ""}
								onchange={(e) => onUpdateBlock(selectedBlock.id, { parentId: e.currentTarget.value })}
							>
								<option value="">-- No Parent (Root) --</option>
								<optgroup label="Screen Containers">
									{#each allScreenContainers as container}
										<option value={container.id}>
											🖥️ {container.name || "Unnamed Screen"}
										</option>
									{/each}
								</optgroup>
								<optgroup label="Section Containers">
									{#each allSectionContainers as container}
										<option value={container.id}>
											📦 {container.name || "Unnamed Section"}
										</option>
									{/each}
								</optgroup>
							</select>
						</label>

						{#if !selectedBlock.parentId}
							<div class="info-box" style="background: #fff3cd; color: #856404; border-left: 3px solid #ffc107;">
								⚠️ This block has no parent container. It won't be part of the responsive layout in viewer mode.
							</div>
						{/if}

						<label>
							<span>Custom CSS</span>
							<textarea
								value={selectedBlock.css || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { css: e.currentTarget.value })}
								placeholder="font-size: 24px;&#10;color: #333;&#10;width: 100%;"
								rows="8"
								style="font-family: monospace;"
							></textarea>
						</label>

						<div class="info-box">
							💡 Define CSS styles for this block. In viewer mode, it will be positioned according to its parent container's layout.
						</div>
					{/if}
				</div>
			{/if}

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

			<!-- Screen Container Properties -->
			{#if selectedBlock.type === 'screen-container'}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>Screen Container Settings</h4>
					</button>
					{#if expandedSections.content}
						<label>
							<span>Container Name</span>
							<input
								type="text"
								value={selectedBlock.name || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { name: e.currentTarget.value })}
								placeholder="Home Page"
							/>
						</label>

						<label class="checkbox-label">
							<input
								type="checkbox"
								checked={selectedBlock.isEntryPoint || false}
								onchange={(e) => onUpdateBlock(selectedBlock.id, { isEntryPoint: e.currentTarget.checked })}
							/>
							<span>Set as Entry Point (first screen in viewer)</span>
						</label>

						{#if selectedBlock.isEntryPoint}
							<div class="info-box" style="background: #d4edda; color: #155724; border-left: 3px solid #28a745;">
								✓ This screen will load first in viewer mode
							</div>
						{/if}

						<label>
							<span>Custom CSS</span>
							<textarea
								value={selectedBlock.css || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { css: e.currentTarget.value })}
								placeholder="max-width: 1200px;&#10;margin: 0 auto;&#10;padding: 40px;"
								rows="10"
								style="font-family: monospace;"
							></textarea>
						</label>

						<div class="info-box">
							🖥️ The main page container. All sections and blocks should be children of this. Define responsive CSS here.
						</div>
					{/if}
				</div>
			{/if}

			<!-- Section Container Properties -->
			{#if selectedBlock.type === 'section-container'}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>Section Container Settings</h4>
					</button>
					{#if expandedSections.content}
						<label>
							<span>Parent Container</span>
							<select
								value={selectedBlock.parentId || ""}
								onchange={(e) => onUpdateBlock(selectedBlock.id, { parentId: e.currentTarget.value })}
							>
								<option value="">-- No Parent --</option>
								<optgroup label="Screen Containers">
									{#each allScreenContainers as container}
										<option value={container.id}>
											🖥️ {container.name || "Unnamed Screen"}
										</option>
									{/each}
								</optgroup>
								<optgroup label="Section Containers">
									{#each allSectionContainers.filter(c => c.id !== selectedBlock.id) as container}
										<option value={container.id}>
											📦 {container.name || "Unnamed Section"}
										</option>
									{/each}
								</optgroup>
							</select>
						</label>

						<label>
							<span>Section Name</span>
							<input
								type="text"
								value={selectedBlock.name || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { name: e.currentTarget.value })}
								placeholder="Hero Section"
							/>
						</label>

						<label class="checkbox-label">
							<input
								type="checkbox"
								checked={selectedBlock.isModal || false}
								onchange={(e) => onUpdateBlock(selectedBlock.id, { isModal: e.currentTarget.checked })}
							/>
							<span>Modal Container</span>
						</label>

						{#if selectedBlock.isModal}
							<div class="info-box" style="background: #e3f2fd; color: #1565c0; border-left: 3px solid #2196f3;">
								🪟 Modal containers appear as overlays with a backdrop. Use navigation buttons to show/hide them.
							</div>

							<label class="checkbox-label">
								<input
									type="checkbox"
									checked={selectedBlock.visible !== false}
									onchange={(e) => onUpdateBlock(selectedBlock.id, { visible: e.currentTarget.checked })}
								/>
								<span>Initially Visible</span>
							</label>

							<label>
								<span>Z-Index (Layer Order)</span>
								<input
									type="number"
									value={selectedBlock.zIndex || 1000}
									oninput={(e) => onUpdateBlock(selectedBlock.id, { zIndex: parseInt(e.currentTarget.value) || 1000 })}
									min="1"
									max="9999"
								/>
							</label>

							<div class="info-box" style="font-size: 0.75rem;">
								💡 Higher z-index values appear on top. Use 1000+ for modals.
							</div>
						{:else}
							<label class="checkbox-label">
								<input
									type="checkbox"
									checked={selectedBlock.visible !== false}
									onchange={(e) => onUpdateBlock(selectedBlock.id, { visible: e.currentTarget.checked })}
								/>
								<span>Visible by Default</span>
							</label>
						{/if}

						<label>
							<span>Custom CSS</span>
							<textarea
								value={selectedBlock.css || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { css: e.currentTarget.value })}
								placeholder="display: flex;&#10;justify-content: center;&#10;gap: 20px;&#10;padding: 60px 40px;"
								rows="10"
								style="font-family: monospace;"
							></textarea>
						</label>

						<div class="info-box">
							📦 A layout section within the screen. Define flex/grid layout CSS here. Child blocks will follow this layout.
						</div>
					{/if}
				</div>
			{/if}

			<!-- Container Properties (Legacy) -->
			{#if selectedBlock.type === 'container'}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>Container Settings (Legacy)</h4>
					</button>
					{#if expandedSections.content}
						<label>
							<span>Container Label</span>
							<input
								type="text"
								value={selectedBlock.label || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { label: e.currentTarget.value })}
								placeholder="Screen 1"
							/>
						</label>
						<div class="info-box">
							💡 Give this container a name to easily identify it in navigation dropdowns. Each container becomes a full-screen in viewer mode.
						</div>
					{/if}
				</div>
			{/if}

			<!-- Branching Question Properties -->
			{#if selectedBlock.type === 'branching-question'}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>Question Settings</h4>
					</button>
					{#if expandedSections.content}
						<label>
							<span>Question Text</span>
							<input
								type="text"
								value={selectedBlock.question || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { question: e.currentTarget.value })}
								placeholder="Are you a new user?"
							/>
						</label>

						<label>
							<span>"Yes" Button Label</span>
							<input
								type="text"
								value={selectedBlock.yesLabel || "Yes"}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { yesLabel: e.currentTarget.value })}
								placeholder="Yes"
							/>
						</label>

						<label>
							<span>"No" Button Label</span>
							<input
								type="text"
								value={selectedBlock.noLabel || "No"}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { noLabel: e.currentTarget.value })}
								placeholder="No"
							/>
						</label>

						<div class="info-box">
							💡 This question will be used by Nav Buttons to create branching paths.
						</div>
					{/if}
				</div>
			{/if}

			<!-- Form Metadata Properties -->
			{#if selectedBlock.type === 'form'}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>Form Settings</h4>
					</button>
					{#if expandedSections.content}
						<label>
							<span>Form Name (Required)</span>
							<input
								type="text"
								value={selectedBlock.name || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { name: e.currentTarget.value })}
								placeholder="Contact Form"
							/>
						</label>

						<label>
							<span>Event Name (Required)</span>
							<input
								type="text"
								value={selectedBlock.eventName || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { eventName: e.currentTarget.value })}
								placeholder="contact_form_submission"
							/>
						</label>

						<div class="info-box">
							🏷️ The event name identifies this form's data when sent to the backend. Use snake_case (e.g., "contact_form_submission", "newsletter_signup").
						</div>

						<label>
							<span>Description (Optional)</span>
							<textarea
								value={selectedBlock.description || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { description: e.currentTarget.value })}
								placeholder="A brief description of this form..."
								rows="3"
							></textarea>
						</label>

						<div class="info-box">
							📋 This form is invisible in viewer mode. Form fields and submit buttons can reference this form by selecting it from a dropdown.
						</div>
					{/if}
				</div>
			{/if}

			<!-- Thread Block Properties -->
			{#if selectedBlock.type === 'thread'}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>Thread Settings</h4>
					</button>
					{#if expandedSections.content}
						<label>
							<span>Thread Name (Required)</span>
							<input
								type="text"
								value={selectedBlock.name || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { name: e.currentTarget.value })}
								placeholder="Discussion Thread"
							/>
						</label>

						<label>
							<span>Description (Optional)</span>
							<textarea
								value={selectedBlock.description || ""}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { description: e.currentTarget.value })}
								placeholder="What is this thread about?"
								rows="3"
							></textarea>
						</label>

						<div class="info-box">
							💬 This is a collaborative comment thread. Viewers can read and write comments. Updates sync in real-time.
						</div>
					{/if}
				</div>
			{/if}

			<!-- Nav Button Properties -->
			{#if selectedBlock.type === 'nav-button'}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>Navigation Settings</h4>
					</button>
					{#if expandedSections.content}
						<label>
							<span>Button Text</span>
							<input
								type="text"
								value={selectedBlock.content || "Next"}
								oninput={(e) => onUpdateBlock(selectedBlock.id, { content: e.currentTarget.value })}
								placeholder="Next"
							/>
						</label>

						<label>
							<span>Action</span>
							<select
								value={selectedBlock.action || "navigate"}
								onchange={(e) => onUpdateBlock(selectedBlock.id, { action: e.currentTarget.value })}
							>
								<option value="navigate">Navigate to Screen</option>
								<option value="show">Show Container</option>
								<option value="hide">Hide Container</option>
								<option value="toggle">Toggle Container</option>
							</select>
						</label>

						<!-- Form Integration -->
						<label>
							<span>Form (Optional)</span>
							<select
								value={selectedBlock.formId || ""}
								onchange={(e) => onUpdateBlock(selectedBlock.id, { formId: e.currentTarget.value })}
							>
								<option value="">-- No Form --</option>
								{#each allForms as form}
									<option value={form.id}>
										{form.name || "Unnamed Form"}
									</option>
								{/each}
							</select>
						</label>

						{#if selectedBlock.formId}
							<div class="info-box" style="background: #e3f2fd; color: #1565c0; border-left: 3px solid #2196f3; padding: 0.75rem; margin-bottom: 0.75rem;">
								📋 <strong>Form Button</strong><br/>
								This button will automatically cache all form data when clicked.
								{#if selectedBlock.submit}
									<br/>✅ <strong>Will submit form</strong> {selectedBlock.targetContainerId ? 'then navigate' : '(stays on same screen)'}
								{:else}
									<br/>➡️  <strong>Will navigate only</strong> (form data cached, not submitted)
								{/if}
							</div>

							<!-- Submit Checkbox -->
							<label style="display: flex; align-items: center; gap: 0.5rem;">
								<input
									type="checkbox"
									checked={selectedBlock.submit || false}
									onchange={(e) => onUpdateBlock(selectedBlock.id, { submit: e.currentTarget.checked })}
								/>
								<span>Submit Form (check this for final submit button)</span>
							</label>

							<!-- Choice Field -->
							<label>
								<span>Field Name (Optional - for choice buttons)</span>
								<input
									type="text"
									value={selectedBlock.fieldName || ""}
									oninput={(e) => onUpdateBlock(selectedBlock.id, { fieldName: e.currentTarget.value })}
									placeholder="e.g., tshirt_size"
								/>
							</label>

							{#if selectedBlock.fieldName}
								<label>
									<span>Field Value</span>
									<input
										type="text"
										value={selectedBlock.value || ""}
										oninput={(e) => onUpdateBlock(selectedBlock.id, { value: e.currentTarget.value })}
										placeholder="e.g., large"
									/>
								</label>
							{/if}
						{/if}

						<div class="info-box" style="font-size: 0.75rem; padding: 0.5rem;">
							{#if selectedBlock.action === 'navigate' || !selectedBlock.action}
								🎯 <strong>Navigate:</strong> Switch to a different screen
							{:else if selectedBlock.action === 'show'}
								👁️ <strong>Show:</strong> Make a hidden container visible (like opening a modal)
							{:else if selectedBlock.action === 'hide'}
								🚫 <strong>Hide:</strong> Hide a visible container (like closing a modal)
							{:else if selectedBlock.action === 'toggle'}
								🔄 <strong>Toggle:</strong> Show if hidden, hide if visible
							{/if}
						</div>

						{#if branchingQuestionInContainer() && (selectedBlock.action === 'navigate' || !selectedBlock.action)}
							<!-- Branching navigation detected -->
							<div class="info-box">
								🔀 Branching question detected: "{branchingQuestionInContainer().question}"
							</div>

							<div class="branch-config">
								<label>
									<span>When "Yes" → Navigate to Screen</span>
									<select
										value={selectedBlock.yesTargetId || ""}
										onchange={(e) => {
											const targetId = e.currentTarget.value;
											onUpdateBlock(selectedBlock.id, {
												yesTargetId: targetId,
												questionId: branchingQuestionInContainer().id
											});
										}}
									>
										<option value="">-- Select Screen --</option>
										{#each allScreenContainers as container}
											<option value={container.id}>
												🖥️ {container.name || "Unnamed Screen"}
											</option>
										{/each}
									</select>
								</label>

								{#if selectedBlock.yesTargetId}
									<div class="target-preview">
										✓ YES path configured
									</div>
								{/if}

								<label>
									<span>When "No" → Navigate to Screen</span>
									<select
										value={selectedBlock.noTargetId || ""}
										onchange={(e) => {
											const targetId = e.currentTarget.value;
											onUpdateBlock(selectedBlock.id, {
												noTargetId: targetId,
												questionId: branchingQuestionInContainer().id
											});
										}}
									>
										<option value="">-- Select Screen --</option>
										{#each allScreenContainers as container}
											<option value={container.id}>
												🖥️ {container.name || "Unnamed Screen"}
											</option>
										{/each}
									</select>
								</label>

								{#if selectedBlock.noTargetId}
									<div class="target-preview">
										✓ NO path configured
									</div>
								{/if}
							</div>
						{:else}
							<!-- Simple action configuration -->
							<label>
								<span>
									{#if selectedBlock.action === 'navigate' || !selectedBlock.action}
										Navigate to Screen
									{:else}
										Target Container
									{/if}
								</span>
								<select
									value={selectedBlock.targetContainerId || ""}
									onchange={(e) => onUpdateBlock(selectedBlock.id, { targetContainerId: e.currentTarget.value })}
								>
									<option value="">-- Select Target --</option>
									{#if selectedBlock.action === 'navigate' || !selectedBlock.action}
										<!-- Show only screen containers for navigation -->
										{#each allScreenContainers as container}
											<option value={container.id}>
												🖥️ {container.name || "Unnamed Screen"}
											</option>
										{/each}
									{:else}
										<!-- Show section containers for show/hide/toggle -->
										{#each allSectionContainers as container}
											<option value={container.id}>
												📦 {container.name || "Unnamed Section"}
											</option>
										{/each}
									{/if}
								</select>
							</label>

							<!-- Show what will happen when clicked -->
							{#if selectedBlock.targetContainerId}
								{@const targetScreen = allScreenContainers.find(s => s.id === selectedBlock.targetContainerId)}
								{@const targetSection = allSectionContainers.find(s => s.id === selectedBlock.targetContainerId)}
								{@const targetName = targetScreen?.name || targetSection?.name || 'Unknown'}

								<div class="info-box" style="background: #e8f5e9; color: #2e7d32; border-left: 3px solid #4caf50;">
									✓ <strong>Target configured:</strong> {targetName}
								</div>
							{/if}

							<div class="info-box">
								💡 Place a branching question in the same container to enable yes/no navigation paths (works with Navigate action only).
							</div>
						{/if}
					{/if}
				</div>
			{/if}

			<!-- Form Field Properties -->
			{#if selectedBlock.type.startsWith('form-field-') || selectedBlock.type === 'form-container' || selectedBlock.type === 'form-submit-button'}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>Form Settings</h4>
					</button>
					{#if expandedSections.content}
						{#if selectedBlock.type.startsWith('form-field-')}
							<!-- Form Field Properties -->
							<label>
								<span>Belongs to Form</span>
								<select
									value={selectedBlock.formId || ""}
									onchange={(e) => onUpdateBlock(selectedBlock.id, { formId: e.currentTarget.value })}
								>
									<option value="">-- No Form Selected --</option>
									{#each allForms as form}
										<option value={form.id}>
											{form.name || "Unnamed Form"}
										</option>
									{/each}
								</select>
							</label>

							{#if !selectedBlock.formId}
								<div class="info-box" style="background: #fff3cd; color: #856404; border-left: 3px solid #ffc107;">
									⚠️ This field is not linked to any form. Select a form above to connect it to a submit button.
								</div>
							{/if}

							<label>
								<span>Label</span>
								<input
									type="text"
									value={selectedBlock.label || ""}
									oninput={(e) => onUpdateBlock(selectedBlock.id, { label: e.currentTarget.value })}
									placeholder="Field label"
								/>
							</label>

							{#if selectedBlock.type !== 'form-field-checkbox'}
								<label>
									<span>Placeholder</span>
									<input
										type="text"
										value={selectedBlock.placeholder || ""}
										oninput={(e) => onUpdateBlock(selectedBlock.id, { placeholder: e.currentTarget.value })}
										placeholder="Placeholder text"
									/>
								</label>
							{/if}

							<label>
								<span>Field Name (for JSON)</span>
								<input
									type="text"
									value={selectedBlock.fieldName || ""}
									oninput={(e) => onUpdateBlock(selectedBlock.id, { fieldName: e.currentTarget.value })}
									placeholder="field_name"
								/>
							</label>

							<label class="checkbox-label">
								<input
									type="checkbox"
									checked={selectedBlock.required || false}
									onchange={(e) => onUpdateBlock(selectedBlock.id, { required: e.currentTarget.checked })}
								/>
								<span>Required field</span>
							</label>
						{:else if selectedBlock.type === 'form-container'}
							<!-- Form Container Properties -->
							<label>
								<span>Form Label</span>
								<input
									type="text"
									value={selectedBlock.label || ""}
									oninput={(e) => onUpdateBlock(selectedBlock.id, { label: e.currentTarget.value })}
									placeholder="Form Container"
								/>
							</label>
							<div class="info-box">
								💡 Place form fields inside this container. Only fields inside will be submitted together.
							</div>
						{:else if selectedBlock.type === 'form-submit-button'}
							<!-- Submit Button Properties -->
							<label>
								<span>Submits Form</span>
								<select
									value={selectedBlock.formId || ""}
									onchange={(e) => onUpdateBlock(selectedBlock.id, { formId: e.currentTarget.value })}
								>
									<option value="">-- No Form Selected --</option>
									{#each allForms as form}
										<option value={form.id}>
											{form.name || "Unnamed Form"}
										</option>
									{/each}
								</select>
							</label>

							{#if !selectedBlock.formId}
								<div class="info-box" style="background: #fff3cd; color: #856404; border-left: 3px solid #ffc107;">
									⚠️ This button is not linked to any form. Select a form above to collect field data on submit.
								</div>
							{/if}

							<label>
								<span>Button Text</span>
								<input
									type="text"
									value={selectedBlock.content || "Submit"}
									oninput={(e) => onUpdateBlock(selectedBlock.id, { content: e.currentTarget.value })}
									placeholder="Submit"
								/>
							</label>

							<label>
								<span>After Submit, Navigate to Screen</span>
								<select
									value={selectedBlock.targetContainerId || ""}
									onchange={(e) => onUpdateBlock(selectedBlock.id, { targetContainerId: e.currentTarget.value })}
								>
									<option value="">-- Stay on Current Screen --</option>
									{#each allScreenContainers as container}
										<option value={container.id}>
											🖥️ {container.name || "Unnamed Screen"}
										</option>
									{/each}
								</select>
							</label>

							<div class="info-box">
								💡 This button will collect data from all form fields with the same form ID and navigate to the next screen.
							</div>
						{/if}
					{/if}
				</div>
			{/if}

			<!-- Text/Heading/Markdown Content -->
			{#if selectedBlock.type === "heading" || selectedBlock.type === "text" || selectedBlock.type === "markdown-text"}
				<div class="property-group">
					<button class="section-header" onclick={() => toggleSection('content')}>
						<span class="section-toggle">{expandedSections.content ? '▼' : '▶'}</span>
						<h4>Content</h4>
					</button>
					{#if expandedSections.content}
						{#if onEditContent}
							<button
								class="edit-content-btn"
								onclick={() => onEditContent?.(selectedBlock.id, selectedBlock.content || "", selectedBlock.type)}
							>
								✏️ Edit Content
							</button>
						{/if}

						<div class="content-preview">
							<span class="preview-label">Preview:</span>
							<div class="preview-text">
								{selectedBlock.content || "(Empty)"}
							</div>
						</div>
					{/if}
				</div>
			{/if}

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
		background: #010409;
		border-left: 1px solid #21262d;
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
		border-bottom: 1px solid #21262d;
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.panel-header h3 {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: #c9d1d9;
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.header-actions {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.block-type-badge {
		background: #89b4fa;
		color: #1e1e2e;
		padding: 0.25rem 0.5rem;
		border-radius: 4px;
		font-size: 0.75rem;
		font-weight: 600;
		text-transform: uppercase;
	}

	.delete-btn {
		padding: 0.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		background: #f38ba8;
		color: white;
		border: none;
		border-radius: 4px;
		cursor: pointer;
		transition: all 0.2s;
	}

	.delete-btn :global(svg) {
		width: 16px;
		height: 16px;
	}

	.delete-btn:hover {
		background: #eba0ac;
		transform: scale(1.1);
	}

	.panel-content {
		flex: 1;
		padding: 1rem;
		overflow-y: auto;
		overflow-x: hidden;
	}

	.panel-content::-webkit-scrollbar {
		width: 4px;
		height: 4px;
	}

	.panel-content::-webkit-scrollbar-track {
		background: transparent;
	}

	.panel-content::-webkit-scrollbar-thumb {
		background: #30363d;
		border-radius: 4px;
	}

	.panel-content::-webkit-scrollbar-thumb:hover {
		background: #484f58;
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
		color: #6e7681;
		font-size: 0.875rem;
	}

	.property-group {
		margin-bottom: 1rem;
		border-bottom: 1px solid #21262d;
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
		background: #161b22;
		border-radius: 4px;
	}

	.section-toggle {
		font-size: 0.75rem;
		color: #8b949e;
		transition: transform 0.2s;
	}

	.property-group h4 {
		margin: 0;
		font-size: 0.75rem;
		font-weight: 600;
		color: #8b949e;
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
		color: #8b949e;
		font-weight: 500;
	}

	input[type="text"],
	input[type="number"],
	select,
	textarea {
		padding: 0.5rem;
		border: 1px solid #30363d;
		border-radius: 4px;
		font-size: 0.875rem;
		background: #0d0e13;
		color: #c9d1d9;
		transition: border-color 0.2s;
		font-family: monospace;
	}

	input[type="text"]:focus,
	input[type="number"]:focus,
	select:focus,
	textarea:focus {
		outline: none;
		border-color: #667eea;
		background: #161b22;
	}

	/* Fix dropdown option styling */
	select {
		appearance: none;
		background-image: url("data:image/svg+xml,%3Csvg width='12' height='8' viewBox='0 0 12 8' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L6 6L11 1' stroke='%238b949e' stroke-width='2' stroke-linecap='round'/%3E%3C/svg%3E");
		background-repeat: no-repeat;
		background-position: right 0.5rem center;
		padding-right: 2rem;
	}

	select option {
		background: #0d0e13;
		color: #c9d1d9;
		padding: 0.5rem;
	}

	select optgroup {
		background: #0d0e13;
		color: #8b949e;
		font-weight: 600;
	}

	/* Force dark background on focused/active state */
	select:focus option,
	select:active option {
		background: #161b22;
		color: #c9d1d9;
	}

	textarea {
		resize: vertical;
		line-height: 1.5;
	}

	input[type="color"] {
		width: 100%;
		height: 40px;
		border: 1px solid #30363d;
		border-radius: 4px;
		cursor: pointer;
		background: #0d0e13;
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
		border: 1px solid #30363d;
		border-radius: 4px;
		background: #0d0e13;
		color: #c9d1d9;
		font-size: 0.75rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	.button-grid button:hover {
		background: #161b22;
		border-color: #667eea;
	}

	.button-grid button.primary-btn {
		background: #89b4fa;
		color: #1e1e2e;
		border: none;
		font-weight: 600;
	}

	.button-grid button.primary-btn:hover {
		background: #74c7ec;
		transform: translateY(-1px);
		box-shadow: 0 2px 8px rgba(137, 180, 250, 0.4);
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
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 6px;
		margin-bottom: 0.75rem;
	}

	.z-index-badge {
		background: #89b4fa;
		color: #1e1e2e;
		padding: 0.25rem 0.75rem;
		border-radius: 20px;
		font-size: 0.75rem;
		font-weight: 700;
	}

	.layer-type {
		font-size: 0.875rem;
		color: #8b949e;
		font-weight: 500;
		text-transform: capitalize;
	}

	.info-box {
		background: #1c2128;
		border: 1px solid #30363d;
		padding: 0.75rem;
		border-radius: 4px;
		font-size: 0.75rem;
		color: #58a6ff;
		margin-top: 0.5rem;
	}

	.upload-btn {
		width: 100%;
		padding: 0.75rem !important;
		background: #89b4fa !important;
		color: #1e1e2e !important;
		border: none !important;
		border-radius: 6px;
		font-weight: 600;
		cursor: pointer;
		transition: transform 0.2s, box-shadow 0.2s, background 0.2s;
		margin-bottom: 1rem;
	}

	.upload-btn:hover {
		background: #74c7ec !important;
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(137, 180, 250, 0.4);
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
		border-bottom: 1px solid #30363d;
	}

	.divider span {
		padding: 0 0.5rem;
		color: #6e7681;
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
		color: #c9d1d9;
	}

	.form-field-item {
		padding: 12px;
		background: #161b22;
		border-radius: 6px;
		border: 1px solid #30363d;
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

	/* Branching navigation */
	.branch-config {
		padding: 1rem;
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 6px;
		margin-top: 0.5rem;
	}

	.target-preview {
		padding: 0.75rem;
		background: #1c2d20;
		border: 1px solid #2ea043;
		border-left: 3px solid #2ea043;
		border-radius: 4px;
		font-size: 0.75rem;
		color: #7ee787;
		margin-top: 0.5rem;
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.target-preview small {
		font-family: monospace;
		opacity: 0.7;
	}

	.edit-content-btn {
		width: 100%;
		padding: 0.75rem;
		background: #89b4fa;
		color: #1e1e2e;
		border: none;
		border-radius: 6px;
		font-weight: 600;
		font-size: 0.875rem;
		cursor: pointer;
		transition: all 0.2s;
		margin-bottom: 0.75rem;
	}

	.edit-content-btn:hover {
		background: #74c7ec;
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(137, 180, 250, 0.4);
	}

	.content-preview {
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 6px;
		padding: 0.75rem;
	}

	.preview-label {
		font-size: 0.75rem;
		color: #8b949e;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		display: block;
		margin-bottom: 0.5rem;
	}

	.preview-text {
		color: #c9d1d9;
		font-size: 0.875rem;
		line-height: 1.5;
		max-height: 120px;
		overflow-y: auto;
		white-space: pre-wrap;
		word-break: break-word;
	}

	.preview-text::-webkit-scrollbar {
		width: 4px;
	}

	.preview-text::-webkit-scrollbar-track {
		background: transparent;
	}

	.preview-text::-webkit-scrollbar-thumb {
		background: #30363d;
		border-radius: 4px;
	}
</style>
