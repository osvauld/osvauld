import { EditorView } from "prosemirror-view";
import { Schema, MarkType, NodeType } from "prosemirror-model";
import { toggleMark, setBlockType } from "prosemirror-commands";
import { EditorState, Selection } from "prosemirror-state";

// Helper function to check if a mark is active
export function markActive(state: EditorState, type: MarkType): boolean {
  try {
    const { from, $from, to, empty } = state.selection;

    if (empty) {
      const marks = type.isInSet($from.marks());
      return Boolean(marks);
    } else {
      return state.doc.rangeHasMark(from, to, type);
    }
  } catch (e) {
    console.error("Error checking mark active state:", e);
    return false;
  }
}

// Helper function to check if node type is active at selection
export function nodeActive(state: EditorState, type: NodeType, attrs: Record<string, unknown> = {}): boolean {
  const { selection } = state;
  const { $from, $to } = selection;

  if ('node' in selection) {
    const node = (selection as any).node;
    if (node) {
      return node.hasMarkup(type, attrs);
    }
  }

  let active = false;
  if ($from.depth > 0) {
    const parent = $from.node($from.depth);
    if (parent.type === type) {
      if (type.name === "heading" && attrs.level !== undefined) {
        active = parent.attrs.level === attrs.level;
      } else {
        active = true;
      }
    }
  }

  return active;
}

// Create a button with text, title and click handler
export function createButton(text: string, title: string, onClick: () => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.className = "editor-menuitem";
  button.textContent = text;
  button.title = title;
  button.type = "button";
  button.addEventListener("click", onClick);
  button.classList.remove("editor-menuitem-active");
  return button;
}

// Update active/disabled states for menu items
export function updateButtonStates(menuNode: HTMLElement, view: EditorView): void {
  const { state } = view;
  const { schema } = state.doc.type;

  // Update mark buttons (bold, italic, code)
  menuNode.querySelectorAll<HTMLElement>("[data-mark-type]").forEach((button) => {
    try {
      const markName = button.dataset.markType;
      if (!markName || !schema.marks[markName]) return;

      const markType = schema.marks[markName];
      button.classList.remove("editor-menuitem-active");
      const isActive = markActive(state, markType);
      if (isActive) {
        button.classList.add("editor-menuitem-active");
      }
    } catch (e) {
      console.error("Error updating mark button state:", e);
    }
  });

  // Update font size input
  const fontSizeInput = menuNode.querySelector<HTMLInputElement>(".font-size-input");
  if (fontSizeInput) {
    const { from, to, empty } = state.selection;
    if (!empty) {
      let fontSize = null;
      state.doc.nodesBetween(from, to, (node) => {
        node.marks.forEach(mark => {
          if (mark.type === schema.marks.fontSize) {
            fontSize = mark.attrs.size;
          }
        });
      });
      if (fontSize) {
        fontSizeInput.value = fontSize;
      }
    }
  }

  // Update node type buttons (headings, paragraph)
  menuNode.querySelectorAll<HTMLElement>("[data-node-type]").forEach((button) => {
    const nodeName = button.dataset.nodeType;
    if (!nodeName) return;
    
    const nodeType = schema.nodes[nodeName];
    if (!nodeType) return;

    if (nodeName === "heading" && button.dataset.level) {
      const level = parseInt(button.dataset.level);
      const isActive = nodeActive(state, nodeType, { level });
      button.classList.toggle("editor-menuitem-active", isActive);
    } else {
      const isActive = nodeActive(state, nodeType);
      button.classList.toggle("editor-menuitem-active", isActive);
    }
  });
} 