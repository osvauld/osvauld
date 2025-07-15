import { EditorState, Transaction } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { Schema, Node, Mark } from "prosemirror-model";

/**
 * Structure for slash command items
 */
export interface SlashCommandItem {
	title: string;
	description?: string;
	icon?: string;
	command: (state: EditorState, dispatch: any, view: EditorView) => boolean;
}

/**
 * Command execution function type
 */
export type CommandFunction = (
	state: EditorState,
	dispatch: ((tr: Transaction) => void) | null | undefined,
	view?: EditorView
) => boolean;

/**
 * Supported text alignment options
 */
export type TextAlignment = 'left' | 'center' | 'right' | 'justify' | null;

/**
 * Supported text indent levels
 */
export type IndentLevel = 0 | 1 | 2 | 3 | null;

/**
 * Node attributes interface for extended nodes
 */
export interface ExtendedNodeAttrs {
	align?: TextAlignment;
	indent?: IndentLevel;
	[key: string]: any;
}

/**
 * Editor state information for the UI
 */
export interface EditorStateInfo {
	isActive: {
		bold: boolean;
		italic: boolean;
		code: boolean;
		bulletList: boolean;
		orderedList: boolean;
		heading1: boolean;
		heading2: boolean;
		heading3: boolean;
	};
	textAlignment: TextAlignment;
	indentLevel: IndentLevel;
}

/**
 * Selection range information
 */
export interface SelectionRange {
	from: number;
	to: number;
	text: string;
} 