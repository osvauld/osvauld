<script lang="ts">
	import { run } from 'svelte/legacy';

	import { setBlockType, toggleMark } from "prosemirror-commands";
	import { wrapInList } from "prosemirror-schema-list";
	import type { EditorView } from "prosemirror-view";
	import type { NodeType, MarkType, Schema } from "prosemirror-model";

	interface Props {
		editorView?: EditorView | null;
		isDarkMode?: boolean;
	}

	let { editorView = null, isDarkMode = $bindable(false) }: Props = $props();


	let isH1 = $state(false);
	let isH2 = $state(false);
	let isH3 = $state(false);
	let isParagraph = $state(false);
	let isBulletList = $state(false);
	let isOrderedList = $state(false);
	let isCodeBlock = $state(false);
	let isBold = $state(false);
	let isItalic = $state(false);

	// Check if a node type is active at the current selection
	function isNodeActive(typeName: string, attrs: Record<string, any> = {}): boolean {
		if (!editorView || !editorView.state) return false;

		const state = editorView.state;
		const { from, to, empty } = state.selection;

		if (empty) {
			const $from = state.selection.$from;
			const node = $from.parent;
			const nodeType = state.schema.nodes[typeName];

			if (!nodeType) return false;

			return node.type === nodeType && node.hasMarkup(nodeType, attrs);
		}

		const node = state.doc.cut(from, to).content.firstChild;
		return node ? node.type.name === typeName : false;
	}

	// Check if a mark is active at the current selection
	function isMarkActive(markName: string): boolean {
		if (!editorView || !editorView.state) return false;

		const state = editorView.state;
		const { empty, from, to } = state.selection;
		const markType = state.schema.marks[markName];

		if (!markType) return false;

		if (empty) {
			const storedMarks = state.storedMarks;
			if (storedMarks) {
				return !!storedMarks.find((mark) => mark.type.name === markName);
			}
			return !!state.selection.$from
				.marks()
				.find((mark) => mark.type.name === markName);
		}

		let hasMatch = false;
		state.doc.nodesBetween(from, to, (node) => {
			if (node.marks.some((mark) => mark.type.name === markName)) {
				hasMatch = true;
			}
		});

		return hasMatch;
	}

	// Helper function to execute ProseMirror commands
	function runCommand(cmd: (state: any, dispatch: any, view: any) => boolean) {
		return () => {
			if (!editorView) return;
			cmd(editorView.state, editorView.dispatch, editorView);
			editorView.focus();
		};
	}

	// Define commands
	const commands = {
		paragraph: () => {
			if (!editorView?.state.schema.nodes.paragraph) return;
			runCommand(setBlockType(editorView.state.schema.nodes.paragraph))();
		},

		h1: () => {
			if (!editorView?.state.schema.nodes.heading) return;
			runCommand(setBlockType(editorView.state.schema.nodes.heading, { level: 1 }))();
		},

		h2: () => {
			if (!editorView?.state.schema.nodes.heading) return;
			runCommand(setBlockType(editorView.state.schema.nodes.heading, { level: 2 }))();
		},

		h3: () => {
			if (!editorView?.state.schema.nodes.heading) return;
			runCommand(setBlockType(editorView.state.schema.nodes.heading, { level: 3 }))();
		},

		bulletList: () => {
			if (!editorView?.state.schema.nodes.bullet_list) return;
			runCommand(wrapInList(editorView.state.schema.nodes.bullet_list))();
		},

		orderedList: () => {
			if (!editorView?.state.schema.nodes.ordered_list) return;
			runCommand(wrapInList(editorView.state.schema.nodes.ordered_list))();
		},

		codeBlock: () => {
			if (!editorView?.state.schema.nodes.code_block) return;
			runCommand(setBlockType(editorView.state.schema.nodes.code_block))();
		},

		toggleBold: () => {
			if (!editorView?.state.schema.marks.strong) return;
			runCommand(toggleMark(editorView.state.schema.marks.strong))();
		},

		toggleItalic: () => {
			if (!editorView?.state.schema.marks.em) return;
			runCommand(toggleMark(editorView.state.schema.marks.em))();
		},

		setTextColor: (color: string) => {
			if (!editorView?.state.schema.marks.textColor) return;
			runCommand(toggleMark(editorView.state.schema.marks.textColor, { color }))();
		},
	};
	// These reactive statements track the active state of each formatting option
	run(() => {
		if (editorView) {
			isH1 = isNodeActive("heading", { level: 1 });
			isH2 = isNodeActive("heading", { level: 2 });
			isH3 = isNodeActive("heading", { level: 3 });
			isParagraph = isNodeActive("paragraph");
			isBulletList = isNodeActive("bullet_list");
			isOrderedList = isNodeActive("ordered_list");
			isCodeBlock = isNodeActive("code_block");
			isBold = isMarkActive("strong");
			isItalic = isMarkActive("em");
		}
	});
</script>

<style>
	.toolbar {
		display: flex;
		gap: 1rem;
		padding: 0.75rem;
		background: white;
		border: 1px solid #e2e8f0;
		border-bottom: none;
		z-index: 10;
		position: relative;
	}

	.toolbar.dark {
		background-color: #212121;
	}

	.toolbar-group {
		display: flex;
		gap: 0.25rem;
		align-items: center;
		padding: 0 0.5rem;
		border-right: 1px solid #e2e8f0;
	}

	.toolbar-group:last-child {
		border-right: none;
	}

	button {
		padding: 0.5rem 0.75rem;
		border: 1px solid #e2e8f0;
		background: white;
		border-radius: 0.25rem;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 2.5rem;
	}

	button:hover {
		background: #f8fafc;
		border-color: #cbd5e1;
	}

	button.active {
		background: #e2e8f0;
		border-color: #94a3b8;
	}

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	button.dark {
		background-color: #212121;
		color: white;
	}

	select {
		padding: 0.5rem;
		border: 1px solid #e2e8f0;
		border-radius: 0.25rem;
		background: white;
		font-size: 0.875rem;
		cursor: pointer;
	}
</style>

<div class="toolbar" class:dark="{isDarkMode}">
	<div class="toolbar-group">
		<button
			class:active="{isParagraph}"
			onclick={commands.paragraph}
			title="Normal text">
			¶
		</button>
		<button class:active="{isH1}" onclick={commands.h1} title="Heading 1">
			H1
		</button>
		<button class:active="{isH2}" onclick={commands.h2} title="Heading 2">
			H2
		</button>
		<button class:active="{isH3}" onclick={commands.h3} title="Heading 3">
			H3
		</button>
	</div>

	<div class="toolbar-group">
		<button
			class:active="{isBulletList}"
			onclick={commands.bulletList}
			title="Bullet list">
			•
		</button>
		<button
			class:active="{isOrderedList}"
			onclick={commands.orderedList}
			title="Numbered list">
			1.
		</button>
	</div>

	<div class="toolbar-group">
		<button
			class:active="{isCodeBlock}"
			onclick={commands.codeBlock}
			title="Code block">
			&lt;/&gt;
		</button>
	</div>

	<div class="toolbar-group">
		<button
			class:active="{isBold}"
			onclick={commands.toggleBold}
			title="Bold">
			B
		</button>
		<button
			class:active="{isItalic}"
			onclick={commands.toggleItalic}
			title="Italic">
			I
		</button>
		<select
			onchange={(e: Event) => {
				const target = e.target as HTMLSelectElement;
				if (target) commands.setTextColor(target.value);
			}}
			title="Text color">
			<option value="">Color</option>
			<option value="#000000">Black</option>
			<option value="#FF0000">Red</option>
			<option value="#00FF00">Green</option>
			<option value="#0000FF">Blue</option>
			<option value="#FF00FF">Purple</option>
		</select>
	</div>

	<button
		class="theme-switcher"
		class:dark="{isDarkMode}"
		onclick={() => (isDarkMode = !isDarkMode)}>
		{isDarkMode ? "☀️" : "🌙"}
	</button>
</div>
