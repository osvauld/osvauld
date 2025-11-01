<script lang="ts">
	import ThreadBlock from './ThreadBlock.svelte';
	import FormField from './FormField.svelte';
	import NavButton from './NavButton.svelte';
	import MarkdownText from './MarkdownText.svelte';
	import BlockRenderer from './BlockRenderer.svelte';
	import type * as Y from 'yjs';
	import { templateState } from '../templateState.svelte';
	import { evaluateValue, hasJEXL } from '../../utils/jexlEvaluator';
	import { untrack } from 'svelte';

	interface Props {
		block: any;
		children: any[];
		ydoc?: Y.Doc;
		commentsDoc?: Y.Doc;
		submissionsDoc?: Y.Doc;
		allBlocks: Map<string, any>;
		onNavigate?: (screenId: string) => void;
		loopContext?: Record<string, any>; // Loop context for forEach children
	}

	let { block, children, ydoc, commentsDoc, submissionsDoc, allBlocks, onNavigate, loopContext }: Props = $props();

	// Reactive evaluations using $derived and $state
	// $effect will automatically track ONLY the state properties accessed in JEXL expressions
	// and re-run only when those specific properties change

	let isVisible = $state(true);
	let evaluatedContent = $state(block.content || '');
	let evaluatedCss = $state(block.css || '');
	let evaluatedBlock = $state<any>(block);
	let isEvaluating = false;

	// forEach handling
	let forEachItems = $state<any[]>([]);
	let hasForEach = $state(false);
	let evaluatedInstances = $state<Array<{
		content: string;
		css: string;
		visible: boolean;
		loopContext: Record<string, any>;
		evaluatedBlock: any;
	}>>([]);

	// Helper function to get children of a block
	function getChildren(parentId: string): any[] {
		if (!allBlocks) return [];
		return Array.from(allBlocks.values())
			.filter(b => b.parentId === parentId)
			.sort((a, b) => (a.order || 0) - (b.order || 0));
	}

	// Evaluate JEXL expressions when template state or block properties change
	function evaluateBlock() {
		if (isEvaluating) return;
		isEvaluating = true;

		try {
			// Check for forEach - if present, get array from templateState signal
			if (block.forEach) {
				hasForEach = true;
				const arrayName = block.forEach;
				const items = templateState.getValue(arrayName); // Read from signal
				forEachItems = Array.isArray(items) ? items : [];
				console.log(`🔁 [BlockRenderer] forEach detected: ${arrayName}, items:`, forEachItems);

				// Pre-evaluate all instances with their loop context
				evaluatedInstances = forEachItems.map((item, index) => evaluateBlockWithContext(item, index));
				console.log(`🔁 [BlockRenderer] Evaluated ${evaluatedInstances.length} instances`);
				return; // Skip regular evaluation when forEach is present
			} else {
				hasForEach = false;
				forEachItems = [];
				evaluatedInstances = [];
			}

			// Evaluate visibility (with loop context if provided)
			const visible = block.visible;
			if (visible === undefined || visible === true) {
				isVisible = true;
			} else if (visible === false) {
				isVisible = false;
			} else if (typeof visible === 'string' && hasJEXL(visible)) {
				const result = evaluateValue(visible, loopContext);
				isVisible = Boolean(result);
			} else {
				isVisible = Boolean(visible);
			}

			// Evaluate content if it has JEXL (with loop context if provided)
			if (block.content && hasJEXL(block.content)) {
				evaluatedContent = evaluateValue(block.content, loopContext);
			} else {
				evaluatedContent = block.content || '';
			}

			// Evaluate CSS if it has JEXL (with loop context if provided)
			if (block.css && hasJEXL(block.css)) {
				evaluatedCss = evaluateValue(block.css, loopContext);
			} else {
				evaluatedCss = block.css || '';
			}

			// If loopContext is provided, evaluate ALL properties (like value, stateUpdates, etc.)
			if (loopContext) {
				console.log(`🔄 [BlockRenderer] Evaluating all properties with loopContext for block ${block.id}`);
				evaluatedBlock = { ...block, content: evaluatedContent, css: evaluatedCss };

				function evaluateProperty(value: any): any {
					if (typeof value === 'string' && hasJEXL(value)) {
						const result = evaluateValue(value, loopContext);
						console.log(`  ↳ Evaluated:`, value, '=>', result);
						return result;
					} else if (value && typeof value === 'object' && !Array.isArray(value)) {
						// Recursively evaluate objects (like stateUpdates)
						const evaluated: any = {};
						for (const [k, v] of Object.entries(value)) {
							evaluated[k] = evaluateProperty(v);
						}
						return evaluated;
					} else {
						return value;
					}
				}

				for (const [key, value] of Object.entries(block)) {
					if (key !== 'content' && key !== 'css' && key !== 'children' && key !== 'forEach') {
						evaluatedBlock[key] = evaluateProperty(value);
					}
				}
			} else {
				// No loop context, use original block
				evaluatedBlock = { ...block, content: evaluatedContent, css: evaluatedCss };
			}
		} catch (error) {
			console.error(`❌ [BlockRenderer] Failed to evaluate JEXL for block ${block.id}:`, error);
		} finally {
			isEvaluating = false;
		}
	}

	// Helper to evaluate block properties with loop item context
	function evaluateBlockWithContext(item: any, index: number) {
		const itemVarName = block.forEachAs || 'item';
		const loopContext = {
			[itemVarName]: item,
			item: item, // Always available as 'item' too
			index: index,
			first: index === 0,
			last: index === forEachItems.length - 1
		};

		// Evaluate content with loop context
		let content = block.content || '';
		if (hasJEXL(block.content)) {
			content = evaluateValue(block.content, loopContext);
			console.log(`[evaluateBlockWithContext] Evaluated content for ${block.type}:`, block.content, '=>', content);
		}

		// Evaluate CSS with loop context
		let css = block.css || '';
		if (hasJEXL(block.css)) {
			css = evaluateValue(block.css, loopContext);
		}

		// Evaluate visibility with loop context
		let visible = true;
		if (block.visible !== undefined) {
			if (typeof block.visible === 'string' && hasJEXL(block.visible)) {
				visible = Boolean(evaluateValue(block.visible, loopContext));
			} else {
				visible = Boolean(block.visible);
			}
		}

		// Evaluate other JEXL properties (like value, stateValue, etc.)
		// Need to handle nested objects like stateUpdates
		const evaluatedBlock: any = { ...block, content, css };

		function evaluateProperty(value: any): any {
			if (typeof value === 'string' && hasJEXL(value)) {
				return evaluateValue(value, loopContext);
			} else if (value && typeof value === 'object' && !Array.isArray(value)) {
				// Recursively evaluate objects (like stateUpdates)
				const evaluated: any = {};
				for (const [k, v] of Object.entries(value)) {
					evaluated[k] = evaluateProperty(v);
				}
				return evaluated;
			} else {
				return value;
			}
		}

		for (const [key, value] of Object.entries(block)) {
			if (key !== 'content' && key !== 'css' && key !== 'children' && key !== 'forEach') {
				const evaluated = evaluateProperty(value);
				if (typeof value === 'string' && hasJEXL(value)) {
					console.log(`[evaluateBlockWithContext] Evaluated ${key}:`, value, '=>', evaluated);
				}
				evaluatedBlock[key] = evaluated;
			}
		}

		return { content, css, visible, loopContext, evaluatedBlock };
	}

	// Automatic fine-grained reactivity using Svelte $effect
	// This will re-run when:
	// 1. State properties accessed in JEXL expressions change
	// 2. loopContext changes (for children of forEach blocks)
	$effect(() => {
		// Track the state version to trigger on any state change
		// In the future, this could be more fine-grained by parsing JEXL expressions
		const currentVersion = templateState.getVersion();
		const currentLoopContext = loopContext;

		// Use untrack to prevent state assignments from triggering this effect again
		untrack(() => {
			console.log('🔄 [BlockRenderer] Evaluating block', block.id);
			evaluateBlock();
		});
	});
</script>

{#if hasForEach}
	<!-- forEach loop: render multiple instances -->
	{#each evaluatedInstances as instance, idx (idx)}
		{#if instance.visible}
			{#if block.type === 'section-container'}
				<div
					class="container-section-container"
					style={instance.css}
					data-block-id={`${block.id}-${idx}`}
				>
					<!-- Render children recursively with loop context -->
					{#each children as child (child.id)}
						<BlockRenderer
							block={child}
							children={getChildren(child.id)}
							{ydoc}
							{commentsDoc}
							{submissionsDoc}
							{allBlocks}
							{onNavigate}
							loopContext={instance.loopContext}
						/>
					{/each}
				</div>
			{:else if block.type === 'nav-button'}
				<NavButton blockId={`${block.id}-${idx}`} blockData={instance.evaluatedBlock} {allBlocks} {ydoc} {onNavigate} loopContext={instance.loopContext} />
			{:else if block.type === 'heading'}
				<h1 style={instance.css} data-block-id={`${block.id}-${idx}`}>{instance.content}</h1>
			{:else if block.type === 'text'}
				<p style={instance.css} data-block-id={`${block.id}-${idx}`}>{instance.content}</p>
			{:else if block.type === 'image'}
				<img src={instance.content} alt="" style={instance.css} data-block-id={`${block.id}-${idx}`} />
			{/if}
		{/if}
	{/each}
{:else if isVisible}
	{#if block.type === 'section-container'}
		{@const isModal = block.isModal || false}
		{@const containerStyle = `${evaluatedCss}; ${isModal && isVisible ? `position: fixed; z-index: ${block.zIndex || 1000};` : ''}`}

		{#if isModal && isVisible}
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
				<BlockRenderer
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
		<FormField blockId={block.id} blockData={evaluatedBlock} />
	{:else if block.type === 'nav-button'}
		<NavButton blockId={block.id} blockData={evaluatedBlock} {allBlocks} {ydoc} {onNavigate} {loopContext} />
	{:else if block.type === 'heading'}
		<h1 style={evaluatedCss} data-block-id={block.id}>{evaluatedContent}</h1>
	{:else if block.type === 'text'}
		<p style={evaluatedCss} data-block-id={block.id}>{evaluatedContent}</p>
	{:else if block.type === 'markdown-text'}
		<MarkdownText blockId={block.id} blockData={block} />
	{:else if block.type === 'image'}
		<img src={evaluatedContent} alt="" style={evaluatedCss} data-block-id={block.id} />
	{:else if block.type === 'form'}
		<!-- Form metadata is invisible -->
	{:else if block.type === 'html'}
		<div class="html-block" style={evaluatedCss} data-block-id={block.id}>
			{@html evaluatedContent}
		</div>
	{/if}
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
