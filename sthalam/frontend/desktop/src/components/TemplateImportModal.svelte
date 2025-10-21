<script lang="ts">
	import { TemplateImporter } from '../utils/templateImporter';
	import type { YjsDocuments } from '../lib/yjsManager';
	import { ClosePanel } from '@osvauld/icons';

	interface Props {
		yjsDocuments: YjsDocuments | null;
		onClose: () => void;
	}

	let { yjsDocuments, onClose }: Props = $props();

	let humlInput = $state('');
	let isImporting = $state(false);
	let error = $state<string | null>(null);
	let success = $state(false);

	const exampleTemplate = `name: "My Website"

screens::
  - ::
    id: "home"
    name: "Home"
    isEntryPoint: true
    css: "padding: 2rem; background: linear-gradient(135deg, #1e1e2e 0%, #0d0e13 100%);"
    children::
      - ::
        type: "heading"
        content: "Welcome to Sthalam"
        css: "font-size: 48px; color: #89b4fa; text-align: center;"

      - ::
        type: "text"
        content: "This is a simple example template"
        css: "color: #c9d1d9; text-align: center; margin-top: 20px;"
`;

	function loadExample() {
		humlInput = exampleTemplate;
		error = null;
		success = false;
	}

	async function handleImport() {
		if (!humlInput.trim()) {
			error = 'Please enter a template';
			return;
		}

		if (!yjsDocuments) {
			error = 'No active resource. Please create or select a resource first.';
			return;
		}

		isImporting = true;
		error = null;
		success = false;

		try {
			const importer = new TemplateImporter();
			await importer.importFromHUML(humlInput, yjsDocuments);

			success = true;
			setTimeout(() => {
				onClose();
			}, 1500);
		} catch (err: any) {
			error = err.message || 'Failed to import template';
			console.error('Import error:', err);
		} finally {
			isImporting = false;
		}
	}

	function handleKeyDown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="modal-overlay" onclick={onClose}>
	<div class="modal" onclick={(e) => e.stopPropagation()}>
		<div class="modal-header">
			<h2>Import Template (HUML)</h2>
			<button class="close-btn" onclick={onClose} title="Close">
				<ClosePanel size={20} color="#c9d1d9" />
			</button>
		</div>

		<div class="modal-content">
			<p class="description">
				Paste your HUML template below to automatically generate your website.
				<button class="link-btn" onclick={loadExample}>Load example</button>
			</p>

			<textarea
				bind:value={humlInput}
				placeholder="Paste HUML template here..."
				rows={20}
				disabled={isImporting}
			></textarea>

			{#if error}
				<div class="error-message">
					<strong>Error:</strong> {error}
				</div>
			{/if}

			{#if success}
				<div class="success-message">
					✅ Template imported successfully!
				</div>
			{/if}
		</div>

		<div class="modal-footer">
			<button class="btn-secondary" onclick={onClose} disabled={isImporting}>
				Cancel
			</button>
			<button class="btn-primary" onclick={handleImport} disabled={isImporting || !humlInput.trim()}>
				{isImporting ? 'Importing...' : 'Import Template'}
			</button>
		</div>
	</div>
</div>

<style>
	.modal-overlay {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: rgba(0, 0, 0, 0.8);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		backdrop-filter: blur(4px);
	}

	.modal {
		background: #0d0e13;
		border: 1px solid #30363d;
		border-radius: 12px;
		width: 90%;
		max-width: 800px;
		max-height: 90vh;
		display: flex;
		flex-direction: column;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 1.5rem;
		border-bottom: 1px solid #21262d;
	}

	.modal-header h2 {
		margin: 0;
		font-size: 1.25rem;
		color: #c9d1d9;
		font-weight: 600;
	}

	.close-btn {
		background: none;
		border: none;
		cursor: pointer;
		padding: 0.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 6px;
		transition: background 0.2s;
	}

	.close-btn:hover {
		background: #21262d;
	}

	.modal-content {
		flex: 1;
		padding: 1.5rem;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.description {
		color: #8b949e;
		font-size: 0.875rem;
		margin: 0;
		line-height: 1.5;
	}

	.link-btn {
		background: none;
		border: none;
		color: #89b4fa;
		cursor: pointer;
		text-decoration: underline;
		padding: 0;
		font-size: inherit;
	}

	.link-btn:hover {
		color: #a5c8ff;
	}

	textarea {
		width: 100%;
		min-height: 400px;
		background: #010409;
		border: 1px solid #30363d;
		border-radius: 8px;
		padding: 1rem;
		color: #c9d1d9;
		font-family: 'JetBrains Mono', monospace;
		font-size: 0.875rem;
		line-height: 1.5;
		resize: vertical;
	}

	textarea:focus {
		outline: none;
		border-color: #89b4fa;
		box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.2);
	}

	textarea:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	textarea::placeholder {
		color: #6e7681;
	}

	.error-message {
		background: rgba(248, 81, 73, 0.1);
		border: 1px solid #f85149;
		border-radius: 6px;
		padding: 0.75rem;
		color: #ffa198;
		font-size: 0.875rem;
	}

	.success-message {
		background: rgba(166, 227, 161, 0.1);
		border: 1px solid #a6e3a1;
		border-radius: 6px;
		padding: 0.75rem;
		color: #a6e3a1;
		font-size: 0.875rem;
	}

	.modal-footer {
		display: flex;
		justify-content: flex-end;
		gap: 0.75rem;
		padding: 1.5rem;
		border-top: 1px solid #21262d;
	}

	.btn-secondary,
	.btn-primary {
		padding: 0.625rem 1.25rem;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
		border: none;
	}

	.btn-secondary {
		background: #21262d;
		color: #c9d1d9;
	}

	.btn-secondary:hover:not(:disabled) {
		background: #30363d;
	}

	.btn-primary {
		background: #89b4fa;
		color: #010409;
	}

	.btn-primary:hover:not(:disabled) {
		background: #a5c8ff;
		box-shadow: 0 2px 8px rgba(137, 180, 250, 0.3);
	}

	.btn-secondary:disabled,
	.btn-primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
