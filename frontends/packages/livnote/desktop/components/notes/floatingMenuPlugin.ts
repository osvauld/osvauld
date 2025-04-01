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

    // Get editor element's position and dimensions
    const editorRect = editorView.dom.getBoundingClientRect();
    const editorViewport = (editorView.dom.parentNode as HTMLElement)?.getBoundingClientRect() || editorRect;
    const menuRect = menu.getBoundingClientRect();

    // Calculate initial position
    let top = start.top - editorRect.top + 45; // Reduced offset from 120 to 20
    let left = (start.left + end.left) / 2 - editorRect.left - (menuRect.width / 2);

    // Check bottom overflow
    const viewportHeight = editorViewport.height;
    const scrollTop = (editorView.dom.parentNode as HTMLElement)?.scrollTop || 0;
    const bottomOverflow = start.top - editorRect.top + menuRect.height + 20 > viewportHeight + scrollTop;

    if (bottomOverflow) {
      // Position above selection instead
      top = start.top - editorRect.top - menuRect.height + 10; // Reduced offset from 90 to 10
      // Ensure it doesn't go above the visible area
      top = Math.max(10, top);
    }

    // Check right overflow
    const rightOverflow = left + menuRect.width > editorRect.width;
    if (rightOverflow) {
      // Align right edge of menu with selection
      left = left - menuRect.width + 20;
      // Ensure it doesn't go too far left
      left = Math.max(10, left);
    }

    // Set position
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