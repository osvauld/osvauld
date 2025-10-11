<script lang="ts">
	import { onMount, onDestroy, untrack } from "svelte";
	import { dataState, uiState } from "../state";
	import NavigationPanel from "../components/NavigationPanel.svelte";
	import FolderManager from "../components/FolderManager.svelte";
	import type { YjsDocuments } from "./yjsManager";

	let yDocs: YjsDocuments | null = null;
	let formFields = $state<any[]>([]);
	let selectedFieldId = $state<string | null>(null);
	let autoSaveInterval: number | null = null;
	let previewMode = $state(false);

	// Form configuration
	let submitButtonText = $state("Submit");

	// Preview mode - form data and submission
	let formData = $state<Record<string, any>>({});
	let submittedJSON = $state<string | null>(null);

	const selectedField = $derived(
		selectedFieldId ? formFields.find((f) => f.id === selectedFieldId) || null : null
	);

	// Track if we have a resource selected
	const hasResource = $derived(!!dataState.currentResourceId);

	// Field types available
	const fieldTypes = [
		{ type: "text", label: "Text Input", icon: "T" },
		{ type: "email", label: "Email", icon: "@" },
		{ type: "number", label: "Number", icon: "#" },
		{ type: "textarea", label: "Text Area", icon: "¶" },
		{ type: "checkbox", label: "Checkbox", icon: "☑" },
	];

	onMount(async () => {
		console.log("🚀 Initializing Form Builder...");

		// Set up auto-save every 10 seconds
		autoSaveInterval = window.setInterval(async () => {
			const currentResourceId = dataState.currentResourceId;
			if (currentResourceId) {
				try {
					console.log("💾 Auto-saving form:", currentResourceId);
					await saveForm();
					console.log("✅ Auto-save completed");
				} catch (error) {
					console.error("❌ Auto-save failed:", error);
				}
			}
		}, 10000); // 10 seconds

		console.log("✅ Form Builder initialized with auto-save!");
	});

	// React to resource changes
	$effect(() => {
		const resourceId = dataState.currentResourceId;
		console.log("🔄 Resource changed:", resourceId);

		if (!resourceId) {
			// No resource selected - clear everything
			console.log("❌ No resource selected - clearing workspace");
			yDocs = null;
			formFields = [];
			selectedFieldId = null;
			submitButtonText = "Submit";
			return;
		}

		// Resource selected - load form data
		console.log("✅ Resource selected - loading form");

		untrack(() => {
			// Get the unified coordinator from dataState
			const coordinator = dataState.getBlocksuiteCoordinator();
			if (!coordinator) {
				console.warn("⚠️ Coordinator not ready yet, will retry when available");
				return;
			}

			console.log("✅ Got coordinator, fetching documents...");

			// Get the Yjs documents
			const docs = coordinator.getDocuments();
			if (!docs) {
				console.error("❌ No Yjs documents available");
				return;
			}

			console.log("✅ Got Yjs documents");
			yDocs = docs;

			// Load form fields from Yjs
			loadFormFromYjs();
		});
	});

	function loadFormFromYjs() {
		if (!yDocs) return;

		// Try to load form config from blocks
		const formConfig = yDocs.blocks.get("form-config");
		if (formConfig && formConfig.content) {
			try {
				const config = JSON.parse(formConfig.content);
				formFields = config.fields || [];
				submitButtonText = config.submitButtonText || "Submit";
				console.log("📦 Form loaded:", formFields.length, "fields");
			} catch (e) {
				console.error("Failed to parse form config:", e);
				formFields = [];
			}
		} else {
			formFields = [];
		}
	}

	function saveFormToYjs() {
		if (!yDocs) return;

		const formConfig = {
			fields: formFields,
			submitButtonText: submitButtonText,
		};

		yDocs.blocks.set("form-config", {
			id: "form-config",
			type: "form-config",
			content: JSON.stringify(formConfig),
			x: 0,
			y: 0,
			width: 0,
			height: 0,
			zIndex: 0,
			styles: {},
		});

		console.log("💾 Form saved to Yjs");
	}

	async function saveForm() {
		try {
			const currentResourceId = dataState.currentResourceId;
			if (!currentResourceId) return;

			saveFormToYjs();
			await dataState.saveCurrentResource(currentResourceId);
		} catch (error) {
			console.error("❌ Error saving form:", error);
			throw error;
		}
	}

	onDestroy(() => {
		// Clear auto-save interval
		if (autoSaveInterval !== null) {
			clearInterval(autoSaveInterval);
			autoSaveInterval = null;
		}

		// Note: Coordinator cleanup is handled by dataState.clearAllState() when needed
	});

	function addField(type: string) {
		const id = `field-${Date.now()}`;
		const newField = {
			id,
			type,
			label: `${type.charAt(0).toUpperCase() + type.slice(1)} Field`,
			placeholder: `Enter ${type}...`,
			required: false,
		};

		formFields = [...formFields, newField];
		selectedFieldId = id;
		saveFormToYjs();
	}

	function updateField(fieldId: string, updates: any) {
		formFields = formFields.map((f) =>
			f.id === fieldId ? { ...f, ...updates } : f
		);
		saveFormToYjs();
	}

	function deleteField(fieldId: string) {
		formFields = formFields.filter((f) => f.id !== fieldId);
		if (selectedFieldId === fieldId) {
			selectedFieldId = null;
		}
		saveFormToYjs();
	}

	function moveFieldUp(index: number) {
		if (index === 0) return;
		const newFields = [...formFields];
		[newFields[index - 1], newFields[index]] = [newFields[index], newFields[index - 1]];
		formFields = newFields;
		saveFormToYjs();
	}

	function moveFieldDown(index: number) {
		if (index === formFields.length - 1) return;
		const newFields = [...formFields];
		[newFields[index], newFields[index + 1]] = [newFields[index + 1], newFields[index]];
		formFields = newFields;
		saveFormToYjs();
	}

	function handleFormSubmit(e: Event) {
		e.preventDefault();

		// Create JSON output
		const output = {
			timestamp: new Date().toISOString(),
			formData: formData,
		};

		submittedJSON = JSON.stringify(output, null, 2);
		console.log("📝 Form submitted:", output);
	}

	function togglePreviewMode() {
		if (previewMode) {
			// Switching back to edit mode - clear form data
			formData = {};
			submittedJSON = null;
		}
		previewMode = !previewMode;
	}
</script>

{#if !hasResource}
	<!-- No resource selected - show empty state -->
	<div class="empty-state">
		<NavigationPanel />
		<div class="empty-message">
			<h2>No form selected</h2>
			<p>Select a form from the sidebar or create a new one to get started.</p>
		</div>
	</div>
{:else}
	<!-- Resource selected - show form builder -->
	<div class="builder-container">
		<NavigationPanel />

		<!-- Field Palette -->
		<div class="palette">
			<div class="palette-header">
				<h3>Form Fields</h3>
			</div>

			<div class="palette-content">
				{#each fieldTypes as fieldType}
					<button
						class="field-type-btn"
						onclick={() => addField(fieldType.type)}
						title="Add {fieldType.label}"
					>
						<span class="icon">{fieldType.icon}</span>
						<span class="label">{fieldType.label}</span>
					</button>
				{/each}
			</div>
		</div>

		<!-- Form Preview/Editor -->
		<div class="form-preview">
			<div class="preview-header">
				<h2>{previewMode ? 'Test Form' : 'Form Preview'}</h2>
				{#if formFields.length > 0}
					<button class="preview-toggle-btn" onclick={togglePreviewMode}>
						{previewMode ? '✏️ Edit Mode' : '▶️ Preview Mode'}
					</button>
				{/if}
			</div>

			<div class="preview-content">
				{#if formFields.length === 0 && !previewMode}
					<div class="empty-form">
						<p>👈 Add form fields from the palette</p>
					</div>
				{:else if previewMode}
					<!-- Preview/Test Mode - Actual Form -->
					<form class="test-form" onsubmit={handleFormSubmit}>
						{#each formFields as field}
							<div class="form-field">
								<label>
									{field.label}
									{#if field.required}<span class="required">*</span>{/if}
								</label>
								{#if field.type === "textarea"}
									<textarea
										name={field.id}
										placeholder={field.placeholder}
										required={field.required}
										bind:value={formData[field.id]}
									></textarea>
								{:else if field.type === "checkbox"}
									<label class="checkbox-field">
										<input
											type="checkbox"
											name={field.id}
											bind:checked={formData[field.id]}
										/>
										<span>{field.placeholder || field.label}</span>
									</label>
								{:else}
									<input
										type={field.type}
										name={field.id}
										placeholder={field.placeholder}
										required={field.required}
										bind:value={formData[field.id]}
									/>
								{/if}
							</div>
						{/each}
						<button type="submit" class="test-submit-btn">
							{submitButtonText}
						</button>
					</form>

					{#if submittedJSON}
						<div class="json-output">
							<div class="json-header">
								<h3>📋 Submitted Data (JSON)</h3>
								<button
									class="copy-btn"
									onclick={() => {
										navigator.clipboard.writeText(submittedJSON || '');
										alert('Copied to clipboard!');
									}}
								>
									Copy
								</button>
							</div>
							<pre>{submittedJSON}</pre>
						</div>
					{/if}
				{:else}
					<!-- Edit Mode - Field List -->
					<div class="form-fields">
						{#each formFields as field, index (field.id)}
							<div
								class="form-field-item"
								class:selected={selectedFieldId === field.id}
								onclick={() => (selectedFieldId = field.id)}
							>
								<div class="field-header">
									<span class="field-type-badge">{field.type}</span>
									<span class="field-label">{field.label}</span>
									{#if field.required}
										<span class="required-badge">Required</span>
									{/if}
									<div class="field-actions">
										<button
											class="icon-btn"
											onclick={(e) => {
												e.stopPropagation();
												moveFieldUp(index);
											}}
											disabled={index === 0}
											title="Move up"
										>
											↑
										</button>
										<button
											class="icon-btn"
											onclick={(e) => {
												e.stopPropagation();
												moveFieldDown(index);
											}}
											disabled={index === formFields.length - 1}
											title="Move down"
										>
											↓
										</button>
										<button
											class="icon-btn delete"
											onclick={(e) => {
												e.stopPropagation();
												deleteField(field.id);
											}}
											title="Delete"
										>
											×
										</button>
									</div>
								</div>

								<div class="field-preview">
									{#if field.type === "textarea"}
										<textarea placeholder={field.placeholder} disabled></textarea>
									{:else if field.type === "checkbox"}
										<label class="checkbox-preview">
											<input type="checkbox" disabled />
											<span>{field.placeholder || field.label}</span>
										</label>
									{:else}
										<input
											type={field.type}
											placeholder={field.placeholder}
											disabled
										/>
									{/if}
								</div>
							</div>
						{/each}
					</div>

					<div class="submit-section">
						<input
							type="text"
							bind:value={submitButtonText}
							placeholder="Submit button text"
							class="submit-text-input"
							onchange={() => saveFormToYjs()}
						/>
						<button class="submit-preview" disabled>{submitButtonText}</button>
					</div>
				{/if}
			</div>
		</div>

		<!-- Properties Panel -->
		{#if selectedField}
			<div class="properties-panel">
				<div class="properties-header">
					<h3>Field Properties</h3>
					<button
						class="close-btn"
						onclick={() => (selectedFieldId = null)}
					>
						×
					</button>
				</div>

				<div class="properties-content">
					<div class="property-group">
						<label>Label</label>
						<input
							type="text"
							bind:value={selectedField.label}
							onchange={() => updateField(selectedField.id, { label: selectedField.label })}
						/>
					</div>

					<div class="property-group">
						<label>Placeholder</label>
						<input
							type="text"
							bind:value={selectedField.placeholder}
							onchange={() => updateField(selectedField.id, { placeholder: selectedField.placeholder })}
						/>
					</div>

					<div class="property-group">
						<label class="checkbox-label">
							<input
								type="checkbox"
								bind:checked={selectedField.required}
								onchange={() => updateField(selectedField.id, { required: selectedField.required })}
							/>
							<span>Required field</span>
						</label>
					</div>
				</div>
			</div>
		{/if}

		{#if uiState.showFolderManager}
			<FolderManager position="navigationPanel" />
		{/if}
	</div>
{/if}

<style>
	.builder-container {
		width: 100%;
		height: 100vh;
		overflow: hidden;
		background: #010409;
		display: flex;
	}

	.empty-state {
		width: 100%;
		height: 100vh;
		display: flex;
		background: #010409;
	}

	.empty-message {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		color: #8b949e;
	}

	.empty-message h2 {
		font-size: 1.5rem;
		margin-bottom: 0.5rem;
		color: #c9d1d9;
	}

	.empty-message p {
		font-size: 1rem;
	}

	/* Palette */
	.palette {
		width: 200px;
		height: 100%;
		background: #0d1117;
		border-right: 1px solid #21262d;
		display: flex;
		flex-direction: column;
	}

	.palette-header {
		padding: 1rem;
		border-bottom: 1px solid #21262d;
	}

	.palette-header h3 {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: #c9d1d9;
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.palette-content {
		flex: 1;
		padding: 1rem;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.field-type-btn {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem;
		border: 1px solid #21262d;
		border-radius: 6px;
		background: #0d1117;
		cursor: pointer;
		transition: all 0.2s;
		text-align: left;
		width: 100%;
		color: #c9d1d9;
	}

	.field-type-btn:hover {
		background: #161b22;
		border-color: #58a6ff;
		box-shadow: 0 2px 8px rgba(88, 166, 255, 0.1);
	}

	.icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
		border-radius: 6px;
		font-weight: 600;
		font-size: 16px;
	}

	.label {
		font-size: 0.875rem;
		font-weight: 500;
	}

	/* Form Preview */
	.form-preview {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.preview-header {
		padding: 1.5rem 2rem;
		border-bottom: 1px solid #21262d;
		background: #0d1117;
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.preview-header h2 {
		margin: 0;
		font-size: 1.25rem;
		color: #c9d1d9;
	}

	.preview-toggle-btn {
		padding: 0.5rem 1rem;
		background: #21262d;
		color: #c9d1d9;
		border: 1px solid #30363d;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	.preview-toggle-btn:hover {
		background: #30363d;
		border-color: #58a6ff;
	}

	.preview-content {
		flex: 1;
		padding: 2rem;
		overflow-y: auto;
		background: #010409;
	}

	.empty-form {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		color: #8b949e;
		font-size: 1.125rem;
	}

	.form-fields {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		max-width: 600px;
		margin: 0 auto;
	}

	.form-field-item {
		padding: 1rem;
		background: #0d1117;
		border: 2px solid #21262d;
		border-radius: 8px;
		cursor: pointer;
		transition: all 0.2s;
	}

	.form-field-item:hover {
		border-color: #30363d;
	}

	.form-field-item.selected {
		border-color: #58a6ff;
		background: #161b22;
	}

	.field-header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-bottom: 0.75rem;
	}

	.field-type-badge {
		padding: 0.25rem 0.5rem;
		background: #21262d;
		color: #8b949e;
		border-radius: 4px;
		font-size: 0.75rem;
		text-transform: uppercase;
		font-weight: 600;
	}

	.field-label {
		flex: 1;
		color: #c9d1d9;
		font-weight: 500;
	}

	.required-badge {
		padding: 0.25rem 0.5rem;
		background: #da3633;
		color: white;
		border-radius: 4px;
		font-size: 0.75rem;
		font-weight: 600;
	}

	.field-actions {
		display: flex;
		gap: 0.25rem;
	}

	.icon-btn {
		width: 28px;
		height: 28px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: #21262d;
		border: none;
		border-radius: 4px;
		color: #c9d1d9;
		cursor: pointer;
		transition: all 0.2s;
		font-size: 1rem;
	}

	.icon-btn:hover:not(:disabled) {
		background: #30363d;
	}

	.icon-btn:disabled {
		opacity: 0.3;
		cursor: not-allowed;
	}

	.icon-btn.delete {
		color: #da3633;
		font-size: 1.5rem;
	}

	.icon-btn.delete:hover {
		background: #da3633;
		color: white;
	}

	.field-preview input,
	.field-preview textarea {
		width: 100%;
		padding: 0.5rem;
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 6px;
		color: #c9d1d9;
		font-size: 0.875rem;
	}

	.field-preview textarea {
		min-height: 80px;
		resize: vertical;
	}

	.checkbox-preview {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		color: #c9d1d9;
		cursor: not-allowed;
	}

	.checkbox-preview input {
		width: auto;
		cursor: not-allowed;
	}

	.submit-section {
		margin-top: 2rem;
		max-width: 600px;
		margin-left: auto;
		margin-right: auto;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.submit-text-input {
		padding: 0.5rem;
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 6px;
		color: #c9d1d9;
		font-size: 0.875rem;
	}

	.submit-preview {
		padding: 0.75rem 1.5rem;
		background: #238636;
		color: white;
		border: none;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 600;
		cursor: not-allowed;
		opacity: 0.7;
		align-self: flex-start;
	}

	/* Properties Panel */
	.properties-panel {
		width: 300px;
		height: 100%;
		background: #0d1117;
		border-left: 1px solid #21262d;
		display: flex;
		flex-direction: column;
	}

	.properties-header {
		padding: 1rem;
		border-bottom: 1px solid #21262d;
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.properties-header h3 {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: #c9d1d9;
	}

	.close-btn {
		width: 28px;
		height: 28px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: none;
		color: #8b949e;
		cursor: pointer;
		font-size: 1.5rem;
		border-radius: 4px;
		transition: all 0.2s;
	}

	.close-btn:hover {
		background: #21262d;
		color: #c9d1d9;
	}

	.properties-content {
		flex: 1;
		padding: 1rem;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.property-group {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.property-group label {
		font-size: 0.875rem;
		font-weight: 500;
		color: #8b949e;
	}

	.property-group input[type="text"] {
		padding: 0.5rem;
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 6px;
		color: #c9d1d9;
		font-size: 0.875rem;
	}

	.property-group input[type="text"]:focus {
		outline: none;
		border-color: #58a6ff;
	}

	.checkbox-label {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		color: #c9d1d9;
		cursor: pointer;
	}

	.checkbox-label input {
		cursor: pointer;
	}

	/* Test/Preview Mode Styles */
	.test-form {
		max-width: 600px;
		margin: 0 auto;
		padding: 2rem;
		background: #0d1117;
		border: 1px solid #21262d;
		border-radius: 8px;
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
	}

	.test-form .form-field {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.test-form .form-field > label {
		font-size: 0.875rem;
		font-weight: 500;
		color: #c9d1d9;
	}

	.test-form .required {
		color: #da3633;
		margin-left: 2px;
	}

	.test-form input[type="text"],
	.test-form input[type="email"],
	.test-form input[type="number"],
	.test-form textarea {
		padding: 0.75rem;
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 6px;
		color: #c9d1d9;
		font-size: 0.875rem;
	}

	.test-form input:focus,
	.test-form textarea:focus {
		outline: none;
		border-color: #58a6ff;
	}

	.test-form textarea {
		min-height: 100px;
		resize: vertical;
	}

	.test-form .checkbox-field {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		cursor: pointer;
		color: #c9d1d9;
	}

	.test-form .checkbox-field input {
		width: 18px;
		height: 18px;
		cursor: pointer;
	}

	.test-submit-btn {
		padding: 0.75rem 1.5rem;
		background: #238636;
		color: white;
		border: none;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 600;
		cursor: pointer;
		transition: background 0.2s;
		align-self: flex-start;
	}

	.test-submit-btn:hover {
		background: #2ea043;
	}

	.json-output {
		max-width: 600px;
		margin: 2rem auto 0;
		padding: 1.5rem;
		background: #0d1117;
		border: 2px solid #238636;
		border-radius: 8px;
	}

	.json-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 1rem;
	}

	.json-header h3 {
		margin: 0;
		font-size: 1rem;
		color: #238636;
	}

	.copy-btn {
		padding: 0.5rem 1rem;
		background: #21262d;
		color: #c9d1d9;
		border: 1px solid #30363d;
		border-radius: 6px;
		font-size: 0.75rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	.copy-btn:hover {
		background: #238636;
		border-color: #238636;
		color: white;
	}

	.json-output pre {
		padding: 1rem;
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 6px;
		color: #c9d1d9;
		font-size: 0.875rem;
		line-height: 1.5;
		overflow-x: auto;
		margin: 0;
	}
</style>
