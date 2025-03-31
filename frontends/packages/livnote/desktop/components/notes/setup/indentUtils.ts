import { EditorView } from "prosemirror-view";
import { liftListItem, sinkListItem } from "prosemirror-schema-list";

// Helper function to get current indentation level
export function getCurrentIndent(state) {
  const { $from } = state.selection;
  const node = $from.parent;
  return node.attrs.indent || 0;
}

// Helper function to preserve existing attributes
export function getExistingAttributes(state, pos) {
  const node = state.doc.nodeAt(pos);
  return node ? { ...node.attrs } : {};
}

// Helper function to check if we're in a list
export function isInList(state) {
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
export function indentRight(view: EditorView) {
  const { state, dispatch } = view;
  const { selection } = state;

  // Check if we're in a list
  if (isInList(state)) {
    // Use ProseMirror's built-in list item sinking
    sinkListItem(state.schema.nodes.list_item)(state, dispatch);
  } else {
    // Apply custom indentation for non-list content
    const tr = state.tr;
    const { from, to } = selection;

    // Get current indentation level
    const currentIndent = getCurrentIndent(state);
    const newIndent = Math.min(3, currentIndent + 1); // Maximum 3 levels of indentation

    // Apply indent to selection
    tr.setBlockType(from, to, state.schema.nodes.paragraph, {
      indent: newIndent,
      ...getExistingAttributes(state, from),
    });

    dispatch(tr);
  }

  view.focus();
}

// Function to handle indent left (outdent)
export function indentLeft(view: EditorView) {
  const { state, dispatch } = view;
  const { selection } = state;

  // Check if we're in a list
  if (isInList(state)) {
    // Use ProseMirror's built-in list item lifting
    liftListItem(state.schema.nodes.list_item)(state, dispatch);
  } else {
    // Apply custom outdentation for non-list content
    const tr = state.tr;
    const { from, to } = selection;

    // Get current indentation level
    const currentIndent = getCurrentIndent(state);
    const newIndent = Math.max(0, currentIndent - 1);

    // Apply new indent level
    const attrs = { ...getExistingAttributes(state, from) };

    if (newIndent === 0) {
      // Remove indent attribute when at level 0
      delete attrs.indent;
    } else {
      attrs.indent = newIndent;
    }

    tr.setBlockType(from, to, state.schema.nodes.paragraph, attrs);
    dispatch(tr);
  }

  view.focus();
} 