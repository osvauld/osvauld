import { EditorView } from "prosemirror-view";
import { EditorState } from "prosemirror-state";
import { Node, ResolvedPos } from "prosemirror-model";

// Function to set text alignment
export function setTextAlign(view: EditorView, align: string) {
  const { state, dispatch } = view;
  const { tr, selection } = state;
  const { from, to } = selection;
  
  // Check if the document is empty or has no content
  const docIsEmpty = state.doc.childCount === 0 || 
    (state.doc.childCount === 1 && state.doc.firstChild && state.doc.firstChild.content.size === 0);
  
  if (docIsEmpty) {
    // Create a new paragraph node with the desired alignment
    // Apply alignment directly when creating the node
    const alignAttrs = align === "left" ? {} : { align };
    const paragraph = state.schema.nodes.paragraph.create(alignAttrs, []);
    
    // Replace any existing content with our aligned paragraph
    tr.replaceWith(0, state.doc.content.size, paragraph);
    dispatch(tr);
    view.focus();
    return;
  }

  // Determine if any nodes in the selection already have alignment
  let hasExistingAlignment = false;
  state.doc.nodesBetween(from, to, (node, pos) => {
    if (
      node.type.name === "paragraph" &&
      node.attrs && 
      node.attrs.align &&
      node.attrs.align !== "left"
    ) {
      hasExistingAlignment = true;
    }
  });

  // Apply alignment to all selected blocks
  state.doc.nodesBetween(from, to, (node, pos) => {
    if (node.isBlock && node.attrs && "align" in node.attrs) {
      // Only set the attribute if the align value is different
      if (node.attrs.align !== align) {
        const attrs = { ...node.attrs };

        // If aligning left and there's no special indentation, we can remove the align attribute
        if (align === "left" && !hasExistingAlignment) {
          delete attrs.align;
        } else {
          attrs.align = align;
        }

        tr.setNodeMarkup(pos, null, attrs);
      }
    }
  });

  dispatch(tr);
  view.focus();
}

// Helper function to get current text alignment
export function getCurrentTextAlignment(state: EditorState): string {
  const { $from } = state.selection;
  const node = $from.parent;
  return node.attrs && node.attrs.align ? node.attrs.align : "left";
}

// Update button states to highlight active alignment
export function updateAlignmentButtonStates(menuNode: HTMLElement, state: EditorState): void {
  const currentAlignment = getCurrentTextAlignment(state);

  menuNode.querySelectorAll<HTMLElement>("[data-alignment]").forEach((button) => {
    const alignment = button.dataset.alignment;
    button.classList.toggle(
      "editor-menuitem-active",
      alignment === currentAlignment
    );
  });
}

// Add CSS for text alignment
export const alignmentStyle = `
  /* Text alignment styles */
  .ProseMirror [style*="text-align: center"] {
    text-align: center;
  }
  
  .ProseMirror [style*="text-align: right"] {
    text-align: right;
  }
  
  /* Indentation styles */
  .ProseMirror [data-indent="1"] {
    margin-left: 2em;
  }
  
  .ProseMirror [data-indent="2"] {
    margin-left: 4em;
  }
  
  .ProseMirror [data-indent="3"] {
    margin-left: 6em;
  }
`; 