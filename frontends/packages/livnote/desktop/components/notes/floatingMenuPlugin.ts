import { Plugin, PluginKey } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import { toggleMark } from "prosemirror-commands";

export const floatingMenuKey = new PluginKey("floating-menu");

export function floatingMenuPlugin(schema: Schema) {
  let menu: HTMLElement | null = null;
  let view: EditorView | null = null;
  let isMenuVisible = false;

  // Create the menu element
  function createMenu() {
    if (menu) return menu;

    menu = document.createElement("div");
    menu.className = "floating-menu";
    menu.style.position = "absolute";
    menu.style.zIndex = "50";
    menu.style.background = "#16171f";
    menu.style.border = "1px solid #2a2b2f";
    menu.style.borderRadius = "4px";
    menu.style.padding = "4px";
    menu.style.display = "none";
    menu.style.gap = "4px";
    menu.style.boxShadow = "0 2px 8px rgba(0, 0, 0, 0.25)";
    menu.style.opacity = "0";
    menu.style.transform = "translateY(8px)";
    menu.style.transition = "opacity 0.15s ease, transform 0.15s ease";

    // Add formatting buttons
    if (schema.marks.strong) {
      const boldButton = createButton("Bold", "B", () => toggleMark(schema.marks.strong));
      menu.appendChild(boldButton);
    }

    if (schema.marks.em) {
      const italicButton = createButton("Italic", "I", () => toggleMark(schema.marks.em));
      menu.appendChild(italicButton);
    }

    if (schema.marks.code) {
      const codeButton = createButton("Code", "</>", () => toggleMark(schema.marks.code));
      menu.appendChild(codeButton);
    }

    return menu;
  }

  // Helper to create a button
  function createButton(title: string, label: string, command: () => any) {
    const button = document.createElement("button");
    button.className = "floating-menu-button";
    button.title = title;
    button.textContent = label;
    button.style.background = "#2a2b2f";
    button.style.color = "#bfc0cc";
    button.style.border = "none";
    button.style.padding = "4px 8px";
    button.style.borderRadius = "4px";
    button.style.cursor = "pointer";
    button.style.fontSize = "14px";
    button.style.minWidth = "30px";
    button.style.display = "flex";
    button.style.alignItems = "center";
    button.style.justifyContent = "center";

    // Add hover effect
    button.addEventListener("mouseenter", () => {
      button.style.background = "#3a3b44";
    });
    button.addEventListener("mouseleave", () => {
      button.style.background = "#2a2b2f";
    });

    // Add click handler
    button.addEventListener("mousedown", (e) => {
      e.preventDefault();
      e.stopPropagation();
      if (view) {
        command()(view.state, view.dispatch, view);
        view.focus();
      }
    });

    return button;
  }

  function hideMenu() {
    if (!menu || !isMenuVisible) return;
    menu.style.opacity = "0";
    menu.style.transform = "translateY(8px)";
    setTimeout(() => {
      if (menu) menu.style.display = "none";
    }, 150);
    isMenuVisible = false;
  }

  function showMenu() {
    if (!menu || isMenuVisible) return;
    menu.style.display = "flex";
    // Force a reflow
    menu.getBoundingClientRect();
    menu.style.opacity = "1";
    menu.style.transform = "translateY(0)";
    isMenuVisible = true;
  }

  // Position the menu near the selection
  function positionMenu(editorView: EditorView) {
    if (!menu) return;

    const { selection } = editorView.state;
    if (selection.empty) {
      hideMenu();
      return;
    }

    // Get coordinates of the selection
    const { from, to } = selection;
    const start = editorView.coordsAtPos(from);
    const end = editorView.coordsAtPos(to);

    // Get editor container and its scroll position
    const editorContainer = editorView.dom.closest('.editor-main');
    if (!editorContainer) return;

    const containerRect = editorContainer.getBoundingClientRect();
    const editorRect = editorView.dom.getBoundingClientRect();
    const menuRect = menu.getBoundingClientRect();
    const scrollTop = editorContainer.scrollTop;

    // Calculate the center position of the selection relative to the viewport
    const selectionCenter = {
      top: Math.min(start.top, end.top),
      left: (start.left + end.left) / 2
    };

    // Check if selection is within visible viewport
    const selectionTopInViewport = selectionCenter.top - containerRect.top;
    const selectionBottomInViewport = Math.max(end.bottom, start.bottom) - containerRect.top;

    // If selection is outside viewport, don't show menu
    if (selectionTopInViewport < 0 || selectionBottomInViewport > containerRect.height) {
      hideMenu();
      return;
    }

    // Position menu above selection, accounting for scroll
    let top = selectionCenter.top - editorRect.top - menuRect.height - 5;
    let left = selectionCenter.left - editorRect.left - (menuRect.width / 2);

    // If there's not enough space above in the viewport, position below
    if ((selectionCenter.top - containerRect.top) < menuRect.height + 5) {
      top = Math.max(end.bottom, start.bottom) - editorRect.top + 5;
    }

    // Ensure menu stays within horizontal bounds
    left = Math.max(2, Math.min(left, editorRect.width - menuRect.width - 2));

    // Set the position
    menu.style.top = `${top}px`;
    menu.style.left = `${left}px`;
    showMenu();
  }

  // Handle clicks outside the menu
  function handleClickOutside(event: MouseEvent) {
    if (!menu || !isMenuVisible) return;
    
    const target = event.target as Node;
    if (!menu.contains(target)) {
      hideMenu();
    }
  }

  // Handle scroll events
  function handleScroll() {
    if (isMenuVisible) {
      hideMenu();
    }
  }

  return new Plugin({
    key: floatingMenuKey,
    view(editorView) {
      view = editorView;
      menu = createMenu();
      editorView.dom.parentNode?.appendChild(menu);

      // Add event listeners
      document.addEventListener("mousedown", handleClickOutside);
      editorView.dom.addEventListener("scroll", handleScroll);
      const editorContainer = editorView.dom.closest(".editor-main");
      if (editorContainer) {
        editorContainer.addEventListener("scroll", handleScroll);
      }

      return {
        update(view) {
          const { selection } = view.state;
          if (!selection.empty) {
            positionMenu(view);
          } else if (isMenuVisible) {
            hideMenu();
          }
        },
        destroy() {
          document.removeEventListener("mousedown", handleClickOutside);
          editorView.dom.removeEventListener("scroll", handleScroll);
          const editorContainer = editorView.dom.closest(".editor-main");
          if (editorContainer) {
            editorContainer.removeEventListener("scroll", handleScroll);
          }
          if (menu && menu.parentNode) {
            menu.parentNode.removeChild(menu);
          }
          menu = null;
          view = null;
        }
      };
    }
  });
} 