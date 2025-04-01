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

    const scrollContainer = editorView.dom.closest<HTMLElement>('.editor-main');
    if (!scrollContainer) {
      console.warn("Floating menu: Could not find '.editor-main' scroll container. Positioning may be incorrect.");
      // Attempt to position relative to editor as fallback, but might be wrong if editor itself scrolls differently.
      // Consider hiding if scroll container is essential. For now, proceed cautiously.
       hideMenu(); // Hide if the essential scroll container is missing.
       return;
    }
    // The menu's position is relative to its offsetParent.
    const offsetParent = menu.offsetParent as HTMLElement || document.body;


    // Get coordinates relative to window
    const { from, to } = selection;
    const startCoords = editorView.coordsAtPos(from); // Use start for vertical reference
    const endCoords = editorView.coordsAtPos(to);

    // Get rects relative to window
    const scrollContainerRect = scrollContainer.getBoundingClientRect();
    const offsetParentRect = offsetParent.getBoundingClientRect();
    const menuRect = menu.getBoundingClientRect(); // Assumes menu is visible enough to measure

    // --- Visibility Check (Selection) ---
    // Check if the start of the selection is roughly within the visible part of the scroll container
    const selectionStartVisible = startCoords.top >= scrollContainerRect.top && startCoords.bottom <= scrollContainerRect.bottom;

    if (!selectionStartVisible) {
        // If the start point isn't visible, don't show the menu
        hideMenu();
        return;
    }

    // --- Calculate Target Position (relative to window) ---
    const horizontalCenter = (startCoords.left + endCoords.left) / 2;
    // Default position: Above selection, centered horizontally
    let targetTopWindow = startCoords.top - menuRect.height - 10;
    let targetLeftWindow = horizontalCenter - menuRect.width / 2;

    // --- Adjust for Viewport Overflow (within scrollContainer) ---

    // Check if positioning ABOVE fits vertically within scrollContainer's visible area
    const spaceAbove = startCoords.top - scrollContainerRect.top;
    if (spaceAbove < menuRect.height + 10) {
        // Not enough space above, try positioning BELOW
        const spaceBelow = scrollContainerRect.bottom - endCoords.bottom;
        if (spaceBelow >= menuRect.height + 10) {
            // Enough space below
            targetTopWindow = endCoords.bottom + 10;
        } else {
            // Not enough space above or below. Clamp position to be just inside the top visible boundary.
             // This might not be ideal, hiding could be better. Let's try clamping first.
            targetTopWindow = scrollContainerRect.top + 5;
            // Alternatively: hideMenu(); return; // If clamping looks bad
        }
    }

    // Check horizontal fit within scrollContainer's visible area
    if (targetLeftWindow < scrollContainerRect.left) {
        // Clamp to left edge
        targetLeftWindow = scrollContainerRect.left + 5;
    } else if (targetLeftWindow + menuRect.width > scrollContainerRect.right) {
        // Clamp to right edge
        targetLeftWindow = scrollContainerRect.right - menuRect.width - 5;
    }


    // --- Convert Window Coordinates to Offset Parent Coordinates ---
    // The final CSS top/left must be relative to the offsetParent
    const finalTop = targetTopWindow - offsetParentRect.top;
    const finalLeft = targetLeftWindow - offsetParentRect.left;

    // --- Apply Styles ---
    menu.style.top = `${finalTop}px`;
    menu.style.left = `${finalLeft}px`;
    showMenu(); // Make sure showMenu is called after position is set
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