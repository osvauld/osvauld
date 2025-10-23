<script lang="ts">
	import ThreadBlock from './ThreadBlock.svelte';
	import FormField from './FormField.svelte';
	import NavButton from './NavButton.svelte';
	import MarkdownText from './MarkdownText.svelte';
	import type * as Y from 'yjs';

	interface Props {
		block: any;
		children: any[];
		ydoc?: Y.Doc;
		commentsDoc?: Y.Doc;
		submissionsDoc?: Y.Doc;
		allBlocks: Map<string, any>;
		onNavigate?: (screenId: string) => void;
	}

	let { block, children, ydoc, commentsDoc, submissionsDoc, allBlocks, onNavigate }: Props = $props();

	// Helper function to get children of a block
	function getChildren(parentId: string): any[] {
		if (!allBlocks) return [];
		return Array.from(allBlocks.values())
			.filter(b => b.parentId === parentId)
			.sort((a, b) => (a.order || 0) - (b.order || 0));
	}
</script>

{#if block.type === 'section-container'}
	{@const isModal = block.isModal || false}
	{@const containerStyle = `${block.css || ''}; ${isModal && block.visible !== false ? `position: fixed; z-index: ${block.zIndex || 1000};` : ''}`}

	{#if isModal && block.visible !== false}
		<!-- Modal backdrop -->
		<div
			class="modal-backdrop"
			style="z-index: {(block.zIndex || 1000) - 1};"
			data-backdrop-for={block.id}
		></div>
	{/if}

	<div
		class="container-section-container"
		class:modal-container={isModal}
		style={containerStyle}
		data-block-id={block.id}
	>
		<!-- Render children recursively INSIDE this container -->
		{#each children as child (child.id)}
			<svelte:self
				block={child}
				children={getChildren(child.id)}
				{ydoc}
				{commentsDoc}
				{submissionsDoc}
				{allBlocks}
				{onNavigate}
			/>
		{/each}
	</div>
{:else if block.type === 'thread'}
	<ThreadBlock blockId={block.id} {ydoc} {commentsDoc} />
{:else if block.type.startsWith('form-field-')}
	<FormField blockId={block.id} blockData={block} />
{:else if block.type === 'nav-button'}
	<NavButton blockId={block.id} blockData={block} {allBlocks} {ydoc} {onNavigate} />
{:else if block.type === 'heading'}
	<h1 style={block.css || ""} data-block-id={block.id}>{block.content || ''}</h1>
{:else if block.type === 'text'}
	<p style={block.css || ""} data-block-id={block.id}>{block.content || ''}</p>
{:else if block.type === 'markdown-text'}
	<MarkdownText blockId={block.id} blockData={block} />
{:else if block.type === 'image'}
	<img src={block.content || ''} alt="" style={block.css || ""} data-block-id={block.id} />
{:else if block.type === 'form'}
	<!-- Form metadata is invisible -->
{:else if block.type === 'html'}
	<div class="html-block" style={block.css || ""} data-block-id={block.id}>
		{@html block.content || ''}
	</div>
{/if}

<style>
	.html-block {
		width: 100%;
	}

	/* Modal styles */
	.modal-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.5);
		backdrop-filter: blur(4px);
	}

	.modal-container {
		/* Modal containers can use their own positioning */
		box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
	}
</style>
