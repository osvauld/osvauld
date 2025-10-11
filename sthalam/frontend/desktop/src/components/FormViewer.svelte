<script lang="ts">
	interface Props {
		formConfig: {
			fields: any[];
			submitButtonText: string;
		};
	}

	let { formConfig }: Props = $props();

	let formData = $state<Record<string, any>>({});
	let submittedData = $state<string | null>(null);

	function handleSubmit(e: Event) {
		e.preventDefault();

		// Map field IDs to labels for better readability
		const labeledData: Record<string, any> = {};
		for (const field of formConfig.fields) {
			if (formData[field.id] !== undefined) {
				labeledData[field.label] = formData[field.id];
			}
		}

		// Create JSON output
		const output = {
			timestamp: new Date().toISOString(),
			data: labeledData,
		};

		submittedData = JSON.stringify(output, null, 2);
		console.log("📝 Form submitted:", output);

		// Show alert with JSON
		alert(`Form Submitted!\n\n${submittedData}`);

		// Reset form
		formData = {};
	}
</script>

<div class="form-viewer">
	<div class="form-container">
		<form class="custom-form" onsubmit={handleSubmit}>
			{#each formConfig.fields as field}
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
			<button type="submit" class="form-submit-btn">
				{formConfig.submitButtonText || "Submit"}
			</button>
		</form>

		{#if submittedData}
			<div class="submitted-data">
				<h3>Submitted Data (JSON):</h3>
				<pre>{submittedData}</pre>
			</div>
		{/if}
	</div>
</div>

<style>
	.form-viewer {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2rem;
		background: #010409;
		overflow-y: auto;
	}

	.form-container {
		max-width: 600px;
		width: 100%;
	}

	.custom-form {
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
		padding: 2rem;
		background: #0d1117;
		border: 1px solid #21262d;
		border-radius: 8px;
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.form-field > label {
		font-size: 0.875rem;
		font-weight: 500;
		color: #c9d1d9;
	}

	.form-field .required {
		color: #da3633;
		margin-left: 2px;
	}

	.form-field input[type="text"],
	.form-field input[type="email"],
	.form-field input[type="number"],
	.form-field textarea {
		padding: 0.75rem;
		border: 1px solid #30363d;
		border-radius: 6px;
		font-size: 0.875rem;
		font-family: inherit;
		transition: border-color 0.2s;
		background: #161b22;
		color: #c9d1d9;
	}

	.form-field input:focus,
	.form-field textarea:focus {
		outline: none;
		border-color: #58a6ff;
		box-shadow: 0 0 0 3px rgba(88, 166, 255, 0.1);
	}

	.form-field textarea {
		min-height: 100px;
		resize: vertical;
	}

	.checkbox-field {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		cursor: pointer;
		color: #c9d1d9;
	}

	.checkbox-field input[type="checkbox"] {
		width: 18px;
		height: 18px;
		cursor: pointer;
	}

	.form-submit-btn {
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

	.form-submit-btn:hover {
		background: #2ea043;
	}

	.form-submit-btn:active {
		background: #26a148;
	}

	.submitted-data {
		margin-top: 2rem;
		padding: 1.5rem;
		background: #0d1117;
		border: 1px solid #238636;
		border-radius: 8px;
	}

	.submitted-data h3 {
		margin: 0 0 1rem 0;
		font-size: 1rem;
		color: #238636;
	}

	.submitted-data pre {
		padding: 1rem;
		background: #161b22;
		border: 1px solid #30363d;
		border-radius: 4px;
		overflow-x: auto;
		color: #c9d1d9;
		font-size: 0.875rem;
		line-height: 1.5;
		margin: 0;
	}
</style>
