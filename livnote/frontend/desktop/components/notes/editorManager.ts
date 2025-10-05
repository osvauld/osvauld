import { EditorState, Plugin } from "prosemirror-state";
import { EditorView, Decoration, DecorationSet } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import { baseKeymap } from "prosemirror-commands";
import { keymap } from "prosemirror-keymap";
import { dropCursor } from "prosemirror-dropcursor";
import { gapCursor } from "prosemirror-gapcursor";
import { undo, redo } from "y-prosemirror";
import type { Transaction } from "prosemirror-state";

// Helper function to check if cursor is inside a table
function isInTable(state: EditorState): boolean {
  const { $from } = state.selection;

  for (let depth = $from.depth; depth > 0; depth--) {
    const node = $from.node(depth);
    if (node.type.spec.tableRole === "table") {
      return true;
    }
  }

  return false;
}

const activeNodePlaceholderPlugin = () => {
	return new Plugin({
		props: {
			decorations(state) {
				const { selection, doc } = state;
				const { $from, empty } = selection;

				if (
					!empty ||
					$from.parent.type.name !== "paragraph" ||
					$from.parent.content.size > 0
				) {
					return null;
				}

				// Check if we're inside a table
				const inTable = isInTable(state);
				const placeholderText = inTable 
					? "Right click for options" 
					: "Write, press '/' for commands...";

				const placeholder = Decoration.node($from.before(), $from.after(), {
					class: "is-empty",
					"data-placeholder": placeholderText,
				});

				return DecorationSet.create(doc, [placeholder]);
			},
		},
	});
};

export interface EditorConfig {
	schema: Schema;
	plugins?: Plugin[];
	onTransaction?: (tr: Transaction, newState: EditorState) => void;
}

export interface EditorInstance {
	view: EditorView;
	state: EditorState;
}

/**
 * Manages ProseMirror editor state and view
 * Separated from YJS and collaboration concerns
 */
export class EditorManager {
	private config: EditorConfig;
	private editorView: EditorView | null = null;
	private editorState: EditorState | null = null;
	private container: HTMLElement | null = null;

	constructor(config: EditorConfig) {
		this.config = config;
	}

	/**
	 * Create base plugins that are always needed
	 */
    private createBasePlugins(): Plugin[] {
        // Base plugins without baseKeymap; we'll append baseKeymap last overall in initializeState
        // Note: Do NOT include history() plugin - we use yUndoPlugin() for collaborative undo
        return [
            keymap({
                "Mod-z": undo,
                "Mod-y": redo,
                "Mod-Shift-z": redo,
            }),
            dropCursor(),
            gapCursor(),
            activeNodePlaceholderPlugin(),
        ];
    }

	/**
	 * Initialize editor state with document
	 */

    initializeState(doc?: any, additionalPlugins: Plugin[] = []): EditorState {
        const base = this.createBasePlugins();
        const fromConfig = this.config.plugins || [];
        const extra = additionalPlugins;
        // Ensure baseKeymap comes LAST across the entire plugin list so custom handlers win first
        const plugins: Plugin[] = [
            ...base,
            ...fromConfig,
            ...extra,
            keymap(baseKeymap),
        ];

		this.editorState = EditorState.create({
			schema: this.config.schema,
			doc: doc || undefined,
			plugins,
		});

		return this.editorState;
	}

	/**
	 * Create editor view in container
	 */
	createView(container: HTMLElement, state?: EditorState): EditorView {
		if (this.editorView) {
			this.destroyView();
		}

		this.container = container;
		const editorState = state || this.editorState;

		if (!editorState) {
			throw new Error("Editor state must be initialized before creating view");
		}

		const dispatchTransaction = (tr: Transaction) => {
			if (!this.editorView) return;

			const newState = this.editorView.state.apply(tr);
			this.editorView.updateState(newState);
			this.editorState = newState;

			if (this.config.onTransaction) {
				this.config.onTransaction(tr, newState);
			}
		};

		this.editorView = new EditorView(container, {
			state: editorState,
			dispatchTransaction,
			// Disable browser autocorrect/capitalization suggestions in the editor
			attributes: {
				role: "textbox",
				"aria-multiline": "true",
		    	"aria-label": "Rich text editor",
				spellcheck: "true",
				autocorrect: "on",
				autocapitalize: "off",
				"data-gramm": "true",
			},
		});

		return this.editorView;
	}
	/**
	 * Update editor state
	 */
	updateState(newState: EditorState): void {
		if (!this.editorView) {
			throw new Error("Editor view not initialized");
		}

		this.editorState = newState;
		this.editorView.updateState(newState);
	}

	/**
	 * Get current editor state
	 */
	getState(): EditorState | null {
		return this.editorState;
	}

	/**
	 * Get editor view
	 */
	getView(): EditorView | null {
		return this.editorView;
	}

	/**
	 * Get editor instance
	 */
	getInstance(): EditorInstance | null {
		if (!this.editorView || !this.editorState) {
			return null;
		}

		return {
			view: this.editorView,
			state: this.editorState,
		};
	}

	/**
	 * Focus the editor
	 */
	focus(): void {
		this.editorView?.focus();
	}

	/**
	 * Destroy the editor view
	 */
	destroyView(): void {
		if (this.editorView) {
			this.editorView.destroy();
			this.editorView = null;
		}
	}

	/**
	 * Destroy everything
	 */
	destroy(): void {
		this.destroyView();
		this.editorState = null;
		this.container = null;
	}

	/**
	 * Check if editor is initialized
	 */
	isInitialized(): boolean {
		return this.editorView !== null && this.editorState !== null;
	}

	/**
	 * Get editor content as JSON
	 */
	getJSON(): any {
		return this.editorState?.toJSON();
	}

	/**
	 * Get document content
	 */
	getDocContent(): any {
		return this.editorState?.doc.toJSON();
	}
}
