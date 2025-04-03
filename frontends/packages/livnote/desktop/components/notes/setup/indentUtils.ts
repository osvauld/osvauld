import { EditorView } from "prosemirror-view";
import { liftListItem, sinkListItem } from "prosemirror-schema-list";
import { EditorState } from "prosemirror-state";
import { Node } from "prosemirror-model";

interface NodeAttributes {
  indent?: number;
  level?: number;
  [key: string]: unknown;
}

// Helper function to get current indentation level
export function getCurrentIndent(state: EditorState): number {
  const { $from } = state.selection;
  const node = $from.parent;
  return (node.attrs.indent as number) || 0;
}

// Helper function to preserve existing attributes
export function getExistingAttributes(state: EditorState, pos: number): NodeAttributes {
  const node = state.doc.nodeAt(pos);
  return node ? { ...node.attrs } : {};
}

// Helper function to check if we're in a list
export function isInList(state: EditorState): boolean {
  const { $from } = state.selection;
  let depth = $from.depth;
  while (depth > 0) {
    const node = $from.node(depth);
    if (node.type.name === "bullet_list" || node.type.name === "ordered_list") {
      return true;
    }
    depth--;
  }
  return false;
}

// Function to handle indent right
export function indentRight(view: EditorView): boolean {
  const { state, dispatch } = view;
  const { selection } = state;

  // Check if we're in a list
  if (isInList(state)) {
    // Use ProseMirror's built-in list item sinking
    return sinkListItem(state.schema.nodes.list_item)(state, dispatch);
  } else {
    // Apply custom indentation for non-list content
    const tr = state.tr;
    const { $from } = selection;
    
    // Get the closest parent block node
    let depth = $from.depth;
    while (depth > 0 && !$from.node(depth).type.isBlock) {
      depth--;
    }
    
    if (depth === 0) return false;
    
    const pos = $from.before(depth);
    const node = $from.node(depth);

    // Get current indentation level from the block node
    const currentIndent = node.attrs.indent || 0;
    const newIndent = Math.min(3, currentIndent + 1); // Maximum 3 levels of indentation

    // Prepare the attributes
    const attrs: NodeAttributes = { ...node.attrs, indent: newIndent };

    // If it's a heading, ensure we preserve the level
    if (node.type.name === "heading") {
      attrs.level = node.attrs.level;
    }

    // Apply the changes to the block node
    tr.setNodeMarkup(pos, null, attrs);
    dispatch(tr);
  }

  view.focus();
  return true;
}

// Function to handle indent left (outdent)
export function indentLeft(view: EditorView): boolean {
  const { state, dispatch } = view;
  const { selection } = state;

  // Check if we're in a list
  if (isInList(state)) {
    // Use ProseMirror's built-in list item lifting
    return liftListItem(state.schema.nodes.list_item)(state, dispatch);
  } else {
    // Apply custom outdentation for non-list content
    const tr = state.tr;
    const { $from } = selection;
    
    // Get the closest parent block node
    let depth = $from.depth;
    while (depth > 0 && !$from.node(depth).type.isBlock) {
      depth--;
    }
    
    if (depth === 0) return false;
    
    const pos = $from.before(depth);
    const node = $from.node(depth);

    // Get current indentation level from the block node
    const currentIndent = node.attrs.indent || 0;
    const newIndent = Math.max(0, currentIndent - 1);

    // Prepare the attributes
    const attrs: NodeAttributes = { ...node.attrs };
    
    if (newIndent === 0) {
      // Remove indent attribute when at level 0
      delete attrs.indent;
    } else {
      attrs.indent = newIndent;
    }

    // If it's a heading, ensure we preserve the level
    if (node.type.name === "heading") {
      attrs.level = node.attrs.level;
    }

    // Apply the changes to the block node
    tr.setNodeMarkup(pos, null, attrs);
    dispatch(tr);
  }

  view.focus();
  return true;
} 