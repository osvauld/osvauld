<script lang="ts">
	interface Props {
		blockId: string;
		blockData: any;
	}

	let { blockId, blockData }: Props = $props();

	// Store field value in component state (preserved across re-renders)
	let value = $state(blockData.defaultValue || '');
	let checked = $state(blockData.defaultChecked || false);
	let hasError = $state(false);

	// Validate field
	function validate(): boolean {
		if (blockData.required) {
			if (blockData.type === 'form-field-checkbox') {
				hasError = !checked;
				return checked;
			} else {
				hasError = !value.trim();
				return value.trim().length > 0;
			}
		}

		// Email validation
		if (blockData.type === 'form-field-email' && value.trim()) {
			const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
			hasError = !emailRegex.test(value);
			return emailRegex.test(value);
		}

		hasError = false;
		return true;
	}

	function handleBlur() {
		validate();
	}

	// Expose value getter for form submission
	export function getValue() {
		return blockData.type === 'form-field-checkbox' ? checked : value;
	}

	// Expose validation
	export function isValid(): boolean {
		return validate();
	}

	// Expose field name
	export function getFieldName(): string {
		return blockData.fieldName || blockData.label || blockId;
	}
</script>

<div class="form-field" class:has-error={hasError} data-field-id={blockId} data-form-id={blockData.formId}>
	{#if blockData.type === 'form-field-checkbox'}
		<label class="checkbox-label">
			<input
				type="checkbox"
				bind:checked
				onblur={handleBlur}
			/>
			<span>
				{blockData.label}
				{#if blockData.required}<span class="required">*</span>{/if}
			</span>
		</label>
	{:else}
		<label>
			<span class="field-label">
				{blockData.label}
				{#if blockData.required}<span class="required">*</span>{/if}
			</span>
			{#if blockData.type === 'form-field-textarea'}
				<textarea
					bind:value
					placeholder={blockData.placeholder || ''}
					required={blockData.required}
					onblur={handleBlur}
					rows="4"
				></textarea>
			{:else}
				<input
					type={blockData.type === 'form-field-email' ? 'email' : blockData.type === 'form-field-number' ? 'number' : blockData.type === 'form-field-password' ? 'password' : 'text'}
					bind:value
					placeholder={blockData.placeholder || ''}
					required={blockData.required}
					onblur={handleBlur}
				/>
			{/if}
		</label>
	{/if}
	{#if hasError}
		<span class="error-message">
			{#if blockData.type === 'form-field-email'}
				Please enter a valid email address
			{:else if blockData.type === 'form-field-checkbox'}
				This field is required
			{:else}
				This field is required
			{/if}
		</span>
	{/if}
</div>

<style>
	.form-field {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		margin-bottom: 1rem;
	}

	label {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.field-label {
		font-size: 0.875rem;
		font-weight: 500;
		color: #24292e;
	}

	.required {
		color: #d73a49;
		margin-left: 0.25rem;
	}

	input[type="text"],
	input[type="email"],
	input[type="password"],
	input[type="number"],
	textarea {
		width: 100%;
		padding: 0.75rem;
		border: 1px solid #d1d5da;
		border-radius: 6px;
		font-size: 0.875rem;
		font-family: inherit;
		transition: border-color 0.2s;
	}

	input:focus,
	textarea:focus {
		outline: none;
		border-color: #0366d6;
		box-shadow: 0 0 0 3px rgba(3, 102, 214, 0.1);
	}

	.has-error input,
	.has-error textarea {
		border-color: #d73a49;
	}

	.has-error input:focus,
	.has-error textarea:focus {
		box-shadow: 0 0 0 3px rgba(215, 58, 73, 0.1);
	}

	textarea {
		resize: vertical;
		min-height: 80px;
	}

	.checkbox-label {
		flex-direction: row !important;
		align-items: center;
		gap: 0.75rem;
		cursor: pointer;
	}

	.checkbox-label input[type="checkbox"] {
		width: auto;
		height: auto;
		margin: 0;
		cursor: pointer;
	}

	.checkbox-label span {
		font-size: 0.875rem;
		font-weight: 500;
		color: #24292e;
	}

	.error-message {
		font-size: 0.75rem;
		color: #d73a49;
		margin-top: -0.25rem;
	}
</style>
