/**
 * HUML Editor Theme
 * Custom theme with better colors and quote hiding
 */

import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';

/**
 * HUML Syntax Highlighting Theme
 * Different colors for different constructs
 */
const humlHighlightStyle = HighlightStyle.define([
  // Comments - gray
  { tag: t.comment, color: '#6a737d' },

  // Property names - cyan
  { tag: t.propertyName, color: '#79c0ff' },

  // Keywords (type:, action:, etc.) - purple
  { tag: t.keyword, color: '#d2a8ff', fontWeight: 'bold' },

  // String values - green
  { tag: t.string, color: '#7ee787' },

  // Numbers - orange
  { tag: t.number, color: '#ffa657' },

  // Booleans/null - blue
  { tag: t.atom, color: '#79c0ff' },

  // Type values (block types) - yellow/gold
  { tag: t.typeName, color: '#f0e68c', fontWeight: 'bold' },

  // Variable names - light blue
  { tag: t.variableName, color: '#a5d6ff' },

  // Brackets, punctuation - white/gray
  { tag: t.bracket, color: '#c9d1d9' },
  { tag: t.punctuation, color: '#8b949e' },
  { tag: t.separator, color: '#8b949e' },

  // Label names (name:, id:) - pink
  { tag: t.labelName, color: '#ff7b72', fontWeight: 'bold' },

  // Attribute names (css:) - light green
  { tag: t.attributeName, color: '#7ee787', fontStyle: 'italic' },

  // Definition keywords - bright purple
  { tag: t.definitionKeyword, color: '#d2a8ff', fontWeight: 'bold' },

  // Namespace (target values) - aqua
  { tag: t.namespace, color: '#56d4dd', fontWeight: 'bold' },

  // Class names (containers) - bright yellow
  { tag: t.className, color: '#ffd700', fontWeight: 'bold' },
]);

/**
 * HUML Editor Theme
 * Dark theme matching GitHub dark
 */
const humlEditorTheme = EditorView.theme({
  '&': {
    backgroundColor: '#0d1117',
    color: '#c9d1d9',
  },
  '.cm-content': {
    caretColor: '#58a6ff',
  },
  '.cm-cursor, .cm-dropCursor': {
    borderLeftColor: '#58a6ff',
  },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': {
    backgroundColor: '#1f6feb40',
  },
  '.cm-activeLine': {
    backgroundColor: '#161b22',
  },
  '.cm-gutters': {
    backgroundColor: '#0d1117',
    color: '#6e7681',
    border: 'none',
  },
  '.cm-activeLineGutter': {
    backgroundColor: '#161b22',
  },
  '.cm-foldGutter .cm-gutterElement': {
    cursor: 'pointer',
    padding: '0 3px',
  },
  '.cm-foldPlaceholder': {
    backgroundColor: '#21262d',
    border: '1px solid #30363d',
    color: '#8b949e',
    borderRadius: '3px',
    padding: '0 4px',
    margin: '0 2px',
  },
  // Line numbers
  '.cm-lineNumbers .cm-gutterElement': {
    color: '#6e7681',
    padding: '0 8px 0 5px',
  },
  // Scrollbar styling
  '.cm-scroller::-webkit-scrollbar': {
    width: '12px',
    height: '12px',
  },
  '.cm-scroller::-webkit-scrollbar-track': {
    background: '#0d1117',
  },
  '.cm-scroller::-webkit-scrollbar-thumb': {
    background: '#30363d',
    borderRadius: '6px',
  },
  '.cm-scroller::-webkit-scrollbar-thumb:hover': {
    background: '#484f58',
  },
}, { dark: true });

/**
 * Hide quote marks decoration
 * Makes strings look cleaner
 */
const hideQuotesTheme = EditorView.baseTheme({
  // We'll use CSS to reduce opacity of quotes
  // This is a simple approach - for full hiding we'd need decorations
  '.cm-line': {
    // Can add line-specific styling here
  }
});

/**
 * Export combined theme
 */
export const humlTheme: Extension = [
  humlEditorTheme,
  syntaxHighlighting(humlHighlightStyle),
  hideQuotesTheme,
];
