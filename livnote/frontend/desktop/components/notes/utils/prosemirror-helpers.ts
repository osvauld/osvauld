import { Selection } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import type { EditorState, Transaction } from "prosemirror-state";

/**
 * Safely move cursor to start of document
 */
export function moveToStart(view: EditorView): void {
  try {
    const tr = view.state.tr.setSelection(Selection.atStart(view.state.doc));
    view.dispatch(tr);
  } catch (error) {
    console.warn("Failed to move to document start:", error);
  }
}

/**
 * Safely move cursor to end of document
 */
export function moveToEnd(view: EditorView): void {
  try {
    const tr = view.state.tr.setSelection(Selection.atEnd(view.state.doc));
    view.dispatch(tr);
  } catch (error) {
    console.warn("Failed to move to document end:", error);
  }
}

/**
 * Safely create a selection near a position
 */
export function createNearSelection(state: EditorState, pos: number): Selection {
  try {
    const resolvedPos = state.doc.resolve(Math.max(0, Math.min(pos, state.doc.content.size)));
    return Selection.near(resolvedPos);
  } catch (error) {
    console.warn("Failed to create near selection, falling back to start:", error);
    return Selection.atStart(state.doc);
  }
}

/**
 * Safely place cursor at end of document with proper validation
 */
export function placeCursorAtEnd(view: EditorView): void {
  try {
    const { doc } = view.state;
    const endPos = Math.max(0, doc.content.size - 1);
    const resolvedPos = doc.resolve(endPos);

    const tr = view.state.tr.setSelection(Selection.near(resolvedPos));
    view.dispatch(tr.setMeta("cursorPlacement", true));
  } catch (error) {
    console.warn("Failed to place cursor at end:", error);
    // Fallback to document end
    moveToEnd(view);
  }
}

/**
 * Create a node selection for a specific position
 */
export function createNodeSelection(state: EditorState, pos: number): Selection | null {
  try {
    const resolvedPos = state.doc.resolve(pos);
    return Selection.near(resolvedPos);
  } catch (error) {
    console.warn("Failed to create node selection:", error);
    return null;
  }
}
