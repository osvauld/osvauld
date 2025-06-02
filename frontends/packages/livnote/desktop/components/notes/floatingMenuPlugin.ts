import { Plugin, PluginKey } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { Schema, Mark } from "prosemirror-model";
import { toggleMark } from "prosemirror-commands";
import { Decoration, DecorationSet } from "prosemirror-view";

export const floatingMenuKey = new PluginKey("floating-menu");

type MenuMode = "buttons" | "linkInput";

export function floatingMenuPlugin(schema: Schema) {
  let menu: HTMLElement | null = null;
  let view: EditorView | null = null;
  let isMenuVisible = false;
  let currentMode: MenuMode = "buttons";
  let pseudoSelectionDecoration: Decoration | null = null;

  // References to UI elements
  let buttonsContainer: HTMLElement | null = null;
  let linkInputContainer: HTMLElement | null = null;
  let linkInput: HTMLInputElement | null = null;
  let linkDoneButton: HTMLButtonElement | null = null;
  let linkButton: HTMLButtonElement | null = null; // Reference to the link button itself

  // Create the menu element and its internal structure
  function createMenu() {
    if (menu) return menu;

    menu = document.createElement("div");
    menu.className = "floating-menu";
    menu.style.position = "fixed"; // Changed from absolute to fixed
    menu.style.zIndex = "50";
    menu.style.background = "#16171f";
    menu.style.border = "1px solid #2a2b2f";
    menu.style.borderRadius = "4px";
    menu.style.padding = "4px";
    menu.style.display = "none"; // Start hidden
    menu.style.gap = "4px";
    menu.style.boxShadow = "0 2px 8px rgba(0, 0, 0, 0.25)";
    menu.style.opacity = "0";
    menu.style.transform = "translateY(8px)";
    menu.style.transition = "opacity 0.15s ease, transform 0.15s ease";

    // --- Buttons Container ---
    buttonsContainer = document.createElement("div");
    buttonsContainer.style.display = "flex";
    buttonsContainer.style.gap = "4px";

    if (schema.marks.strong) {
      const boldButton = createButton("Bold", "B", "strong", () => {
        if (view) {
          toggleMark(schema.marks.strong)(view.state, view.dispatch);
          view.focus();
        }
      });
      buttonsContainer.appendChild(boldButton);
    }
    if (schema.marks.em) {
      const italicButton = createButton("Italic", "I", "em", () => {
        if (view) {
          toggleMark(schema.marks.em)(view.state, view.dispatch);
          view.focus();
        }
      });
      buttonsContainer.appendChild(italicButton);
    }
    if (schema.marks.code) {
      const codeButton = createButton("Code", `<svg width="16" height="16" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
        <path d="M9.4 16.6L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4zm5.2 0l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4z" fill="#85889C"/>
      </svg>`, "code", () => {
         if (view) {
           toggleMark(schema.marks.code)(view.state, view.dispatch);
           view.focus();
         }
      });
      buttonsContainer.appendChild(codeButton);
    }
    // Add Link Button to the regular buttons
    if (schema.marks.link) {
      linkButton = createButton("Link", 
        `<svg width="16" height="16" viewBox="0 0 24 24" focusable="false" fill="currentColor"><path d="M6.2 12.3a1 1 0 0 1 1.4 1.4l-2 2a2 2 0 1 0 2.6 2.8l4.8-4.8a1 1 0 0 0 0-1.4 1 1 0 1 1 1.4-1.3 2.9 2.9 0 0 1 0 4L9.6 20a3.9 3.9 0 0 1-5.5-5.5l2-2Zm11.6-.6a1 1 0 0 1-1.4-1.4l2-2a2 2 0 1 0-2.6-2.8L11 10.3a1 1 0 0 0 0 1.4A1 1 0 1 1 9.6 13a2.9 2.9 0 0 1 0-4L14.4 4a3.9 3.9 0 0 1 5.5 5.5l-2 2Z" fill-rule="nonzero"></path></svg>`, 
        "link", 
        handleLinkButtonClick // Special handler
      );
      buttonsContainer.appendChild(linkButton);
    }
    
    // Add Comment Button
    if (schema.marks.comment) {
      const commentButton = createButton("Add Comment", 
        `<svg width="16" height="16" viewBox="0 0 24 24" fill="#85889C">
          <path d="M21.99 4c0-1.1-.89-2-2-2H4c-1.1 0-2 .9-2 2v12c0 1.1.89 2 2 2h14l4 4-.01-18z"/>
        </svg>`, 
        "comment", 
        handleCommentButtonClick
      );
      buttonsContainer.appendChild(commentButton);
    }
    menu.appendChild(buttonsContainer);

    // --- Link Input Container (initially hidden) ---
    linkInputContainer = document.createElement("div");
    linkInputContainer.style.display = "none"; // Hidden by default
    linkInputContainer.style.gap = "4px";
    linkInputContainer.style.alignItems = "center";

    linkInput = document.createElement("input");
    linkInput.type = "text";
    linkInput.placeholder = "Enter Link";
    linkInput.style.flexGrow = "1";
    linkInput.style.padding = "4px 6px";
    linkInput.style.border = "1px solid #3a3b44";
    linkInput.style.borderRadius = "3px";
    linkInput.style.background = "#1e1f29";
    linkInput.style.color = "#bfc0cc";
    linkInput.style.fontSize = "13px";
    linkInput.style.outlineWidth = "2px";
    linkInput.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
            e.preventDefault();
            handleLinkDoneClick();
        }
        if (e.key === "Escape") {
            e.preventDefault();
            switchToButtonsMode(); // Revert without applying
            hideMenu();
        }
    });
    linkInputContainer.appendChild(linkInput);

    linkDoneButton = document.createElement("button");
    linkDoneButton.textContent = "Done";
    linkDoneButton.className = "floating-menu-button"; // Reuse some styling
    linkDoneButton.style.background = "#4CAF50";
    linkDoneButton.style.borderRadius = "4px";
    linkDoneButton.style.fontSize = "12px";
    linkDoneButton.style.color = "white";
    linkDoneButton.style.padding = "4px 8px";
    linkDoneButton.addEventListener("mousedown", (e) => {
      e.preventDefault();
      e.stopPropagation();
      handleLinkDoneClick();
    });
    linkInputContainer.appendChild(linkDoneButton);
    menu.appendChild(linkInputContainer);

    return menu;
  }

  // --- Mode Switching Functions ---
  function switchToLinkInputMode() {
    if (!buttonsContainer || !linkInputContainer || !linkInput || !view) return;
    
    const { state, dispatch } = view;
    const { selection } = state;
    const { $from, from, to } = selection;
    
    // Create and store pseudo-selection decoration
    pseudoSelectionDecoration = Decoration.inline(from, to, { class: 'pseudo-selection' });
    // Trigger a view update to render the decoration
    dispatch(state.tr); // Dispatch an empty transaction just to update decorations

    // Pre-fill input with existing link if present
    const existingMark = schema.marks.link.isInSet($from.marksAcross(selection.$to) || []); // Check across selection
    linkInput.value = existingMark?.attrs.href || "";

    buttonsContainer.style.display = "none";
    linkInputContainer.style.display = "flex";
    currentMode = "linkInput";
    linkInput.focus(); // Focus the input
    linkInput.select();
  }

  function switchToButtonsMode() {
    if (!buttonsContainer || !linkInputContainer) return;
    buttonsContainer.style.display = "flex";
    linkInputContainer.style.display = "none";
    currentMode = "buttons";
    
    // Clear pseudo-selection decoration
    if (pseudoSelectionDecoration && view) {
        pseudoSelectionDecoration = null;
        const { state, dispatch } = view;
        dispatch(state.tr); // Update view to remove decoration
    }
    
    if (view) {
      updateButtonStates(view); // Update button active states when switching back
      view.focus(); // Return focus to editor
    }
  }

  // --- Event Handlers for Link UI ---
  function handleLinkButtonClick() {
    switchToLinkInputMode();
  }

  function handleLinkDoneClick() {
    if (!linkInput || !view) return;
    const href = linkInput.value.trim();

    const { state, dispatch } = view;
    
    // Apply or remove the mark
    toggleMark(schema.marks.link, href ? { href } : null)(state, dispatch);
    
    switchToButtonsMode();
    hideMenu(); // Hide menu after action is done
  }

  // --- Event Handlers for Comment UI ---
  function handleCommentButtonClick() {
    if (!view) return;
    
    const { state } = view;
    const { selection } = state;
    
    if (selection.empty) {
      console.warn("No text selected for commenting");
      return;
    }

    // Get the selected text to show in the modal
    const selectedText = state.doc.textBetween(selection.from, selection.to, ' ');

    // Dispatch a custom event to open the comment modal
    const commentEvent = new CustomEvent('open-comment-modal', {
      detail: {
        selectedText,
        position: {
          from: selection.from,
          to: selection.to
        }
      }
    });
    document.dispatchEvent(commentEvent);

    hideMenu();
  }

  // Helper function to generate unique thread IDs
  function generateThreadId(): string {
    return 'thread_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);
  }

  // Helper to create a button (now includes markType for state updates)
  function createButton(title: string, labelOrHTML: string, markType: string, command: () => any) {
    const button = document.createElement("button");
    button.className = `floating-menu-button menu-${markType}`;
    button.title = title;
    button.innerHTML = labelOrHTML;
    button.dataset.markType = markType; // Store mark type for updates

    // Base styles (can be overridden by classes)
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
    button.style.height = "28px"; // Ensure consistent height

    // Add hover effect
    button.addEventListener("mouseenter", () => {
      if (currentMode === "buttons") button.style.background = "#3a3b44";
    });
    button.addEventListener("mouseleave", () => {
      if (currentMode === "buttons") {
         // Re-apply active style if needed, otherwise default
         const isActive = button.classList.contains('active');
         button.style.background = isActive ? "#3a3b44" : "#2a2b2f"; 
      }
    });

    // Add click handler
    button.addEventListener("mousedown", (e) => {
      e.preventDefault();
      e.stopPropagation();
      if (view && currentMode === "buttons") {
        // Execute the command (either toggleMark or custom handler)
        command(); 
      }
    });

    return button;
  }
  
  // Update button active states based on current selection
  function updateButtonStates(editorView: EditorView) {
    if (!menu || currentMode !== "buttons") return; // Only update in button mode

    const { state } = editorView;
    const { selection } = state;
    const { $from, $to } = selection;

    menu.querySelectorAll<HTMLButtonElement>('.floating-menu-button').forEach(button => {
      const markType = button.dataset.markType;
      if (!markType) return;

      const mark = schema.marks[markType];
      if (!mark) return;

      let isActive = false;
      if (!selection.empty) {
          // Check if mark is present across the entire selection range
          isActive = state.doc.rangeHasMark($from.pos, $to.pos, mark);
      } else {
          // For cursor position, check marks at the cursor
          isActive = !!mark.isInSet($from.marks());
      }

      button.classList.toggle('active', isActive);
      // Update background style based on active state for hover consistency
      button.style.background = isActive ? "#3a3b44" : "#2a2b2f";
    });
  }

  function hideMenu() {
    if (!menu || !isMenuVisible) return;
    menu.style.opacity = "0";
    menu.style.transform = "translateY(8px)";
    
    // Clear pseudo-selection decoration immediately when starting to hide
    if (pseudoSelectionDecoration && view) {
        const { state, dispatch } = view;
        pseudoSelectionDecoration = null;
        dispatch(state.tr); 
    }

    setTimeout(() => {
      if (menu) menu.style.display = "none";
      // Reset to button mode state (decoration is already cleared)
      if (currentMode === "linkInput") {
         currentMode = "buttons"; // Reset mode state, UI handled by display none
         buttonsContainer?.style.setProperty('display', 'flex');
         linkInputContainer?.style.setProperty('display', 'none');
      }
    }, 150);
    isMenuVisible = false;
  }

  function showMenu() {
    if (!menu || isMenuVisible) return;
    menu.style.display = "flex";
    // Ensure correct UI is visible based on mode *before* showing
    if (currentMode === 'buttons') {
        buttonsContainer?.style.setProperty('display', 'flex');
        linkInputContainer?.style.setProperty('display', 'none');
    } else {
        buttonsContainer?.style.setProperty('display', 'none');
        linkInputContainer?.style.setProperty('display', 'flex');
    }
    // Force a reflow
    menu.getBoundingClientRect();
    menu.style.opacity = "1";
    menu.style.transform = "translateY(0)";
    isMenuVisible = true;
  }

  // Handle scroll events
  function handleScroll() {
    if (isMenuVisible) {
      hideMenu();
    }
  }

  // Position the menu near the selection
  function positionMenu(editorView: EditorView) {
    if (!menu) return;

    const { state } = editorView;
    const { selection } = state;
    const { from, to } = selection;

    if (selection.empty) {
      hideMenu();
      return;
    }

    const selectedText = state.doc.textBetween(from, to, ' ');

    if (!selectedText.trim()) {
      hideMenu();
      return;
    }

    if (currentMode === "buttons") {
      updateButtonStates(editorView);
    }

    // Determine the scroll container: prioritize '.scrollbar-thin' parent, fallback to '.editor-main'
    let scrollContainer = editorView.dom.parentElement;
    if (!scrollContainer || !scrollContainer.classList.contains('scrollbar-thin')) {
        scrollContainer = editorView.dom.closest<HTMLElement>('.editor-main');
    }
    
    if (!scrollContainer) {
      console.warn("Floating menu: Could not find scroll container.");
      hideMenu();
      return;
    }

    // Since we're using position: fixed, we don't need offset parent calculations
    const startCoords = editorView.coordsAtPos(from);
    const endCoords = editorView.coordsAtPos(to);
    
    const scrollContainerRect = scrollContainer.getBoundingClientRect();

    // Temporarily display menu to get accurate dimensions if it's currently hidden
    const wasMenuHidden = menu.style.display === 'none';
    if (wasMenuHidden) {
        menu.style.visibility = 'hidden'; // Avoid flicker
        menu.style.display = 'flex';
    }
    const menuRect = menu.getBoundingClientRect();
    if (wasMenuHidden) {
        menu.style.display = 'none';
        menu.style.visibility = 'visible';
    }
    
    const menuHeight = menuRect.height || 36; // Fallback height
    const menuWidth = menuRect.width || 150;  // Fallback width

    // If selection is completely outside the scroll container's visible area, hide menu
    if (endCoords.bottom < scrollContainerRect.top || startCoords.top > scrollContainerRect.bottom) {
        hideMenu();
        return;
    }

    const M_MARGIN = 10; // Desired gap (8-12px)

    let targetTopWindow: number;

    // Calculate available space relative to the scroll container
    const spaceAboveSelection = startCoords.top - scrollContainerRect.top;
    const spaceBelowSelection = scrollContainerRect.bottom - endCoords.bottom;

    // --- Vertical Placement ---
    // Primary: Place above selection if enough space within container above selection
    if (spaceAboveSelection >= menuHeight + M_MARGIN) {
      targetTopWindow = startCoords.top - menuHeight - M_MARGIN;
    } 
    // Fallback: Place below selection if enough space within container below selection
    else if (spaceBelowSelection >= menuHeight + M_MARGIN) {
      targetTopWindow = endCoords.bottom + M_MARGIN;
    } 
    // Constrained: Not enough ideal space above or below.
    // Decide based on more available relative space, or if one side can fit at least half.
    else {
      const canFitAtLeastHalfAbove = spaceAboveSelection >= menuHeight / 2 + M_MARGIN;
      const canFitAtLeastHalfBelow = spaceBelowSelection >= menuHeight / 2 + M_MARGIN;

      if (canFitAtLeastHalfAbove && (!canFitAtLeastHalfBelow || spaceAboveSelection > spaceBelowSelection)) {
        // Prefer above if it has more space or only it can fit half
        targetTopWindow = startCoords.top - menuHeight - M_MARGIN;
      } else if (canFitAtLeastHalfBelow) {
        // Prefer below if it has more space or only it can fit half (or if above wasn't preferred)
        targetTopWindow = endCoords.bottom + M_MARGIN;
      } else {
        // Very constrained. Default to attempting below, then clamp.
        // This handles cases where selection is very large or container very small.
        targetTopWindow = endCoords.bottom + M_MARGIN;
      }
    }

    // --- Horizontal Placement (Center with selection) ---
    const selectionCenterX = (startCoords.left + endCoords.right) / 2;
    let targetLeftWindow = selectionCenterX - menuWidth / 2;

    // --- Apply Container Constraints & Edge Handling ---
    // Adjust horizontal position to stay within scroll container
    targetLeftWindow = Math.max(targetLeftWindow, scrollContainerRect.left + M_MARGIN);
    targetLeftWindow = Math.min(targetLeftWindow, scrollContainerRect.right - menuWidth - M_MARGIN);

    // Adjust vertical position to stay within scroll container (final clamping)
    targetTopWindow = Math.max(targetTopWindow, scrollContainerRect.top + M_MARGIN);
    targetTopWindow = Math.min(targetTopWindow, scrollContainerRect.bottom - menuHeight - M_MARGIN);
    
    // Additional viewport constraints to ensure menu stays on screen
    targetTopWindow = Math.max(targetTopWindow, M_MARGIN);
    targetTopWindow = Math.min(targetTopWindow, window.innerHeight - menuHeight - M_MARGIN);
    targetLeftWindow = Math.max(targetLeftWindow, M_MARGIN);
    targetLeftWindow = Math.min(targetLeftWindow, window.innerWidth - menuWidth - M_MARGIN);

    // Since we're using position: fixed, use window coordinates directly
    const finalTop = targetTopWindow;
    const finalLeft = targetLeftWindow;

    menu.style.top = `${finalTop}px`;
    menu.style.left = `${finalLeft}px`;

    showMenu();
  }

  // Handle clicks outside the menu
  function handleClickOutside(event: MouseEvent) {
    if (!menu || !isMenuVisible || !view) return; // Check view as well
    
    const target = event.target as Node;

    // Find the main editor menu element (assuming it has a class like 'editor-fixed-menu')
    // Adjust selector based on your actual DOM structure for the fixed menu container
    const fixedMenu = view.dom.closest('.editor-container')?.querySelector('.editor-fixed-menu'); 

    // Check if the click target is inside the floating menu OR inside a button within the fixed menu
    const isClickInsideFloatingMenu = menu.contains(target);
    // Check if the target is inside the fixed menu *and* is a button or inside a button
    const isClickInsideFixedMenuButton = fixedMenu?.contains(target) && !!(target as HTMLElement).closest('button'); 

    // Check if the click is inside the editor content area itself (excluding the floating menu)
    const isClickInsideEditor = view.dom.contains(target) && !isClickInsideFloatingMenu;

    // Hide ONLY if the click is NOT inside the floating menu,
    // NOT inside a fixed menu button,
    // AND NOT inside the editor content area.
    if (!isClickInsideFloatingMenu && !isClickInsideFixedMenuButton && !isClickInsideEditor) {
      hideMenu();
    }
  }

  return new Plugin({
    key: floatingMenuKey,
    view(editorView) {
      view = editorView;
      menu = createMenu();
      
      // Find the best container for the menu - prefer editor containers over body
      let menuContainer = document.body; // fallback
      
      // Try to find a better container in this order of preference:
      const editorContainer = editorView.dom.closest('.editor-container') || 
                             editorView.dom.closest('.editor-main') || 
                             editorView.dom.closest('[data-editor]');
      
      if (editorContainer) {
        menuContainer = editorContainer as HTMLElement;
      } else if (editorView.dom.parentNode) {
        menuContainer = editorView.dom.parentNode as HTMLElement;
      }
      
      menuContainer.appendChild(menu);

      // Add event listeners
      document.addEventListener("mousedown", handleClickOutside, true); // Use capture phase
      editorView.dom.addEventListener("scroll", handleScroll);
      
      // Find the actual scrolling container - look for the element with scrollbar-thin class
      // which is the direct parent of the ProseMirror editor
      const actualScrollContainer = editorView.dom.parentElement;
      if (actualScrollContainer && actualScrollContainer.classList.contains('scrollbar-thin')) {
        actualScrollContainer.addEventListener("scroll", handleScroll);
      }
      
      // Also add to .editor-main as fallback
      const editorMainContainer = editorView.dom.closest(".editor-main");
      if (editorMainContainer) {
        editorMainContainer.addEventListener("scroll", handleScroll);
      }

      return {
        update(view, prevState) {
          const { state } = view;
          const { selection } = state;
          const prevSelection = prevState?.selection;

          // Don't update if selection hasn't changed or menu isn't visible and selection is empty
          if (selection.eq(prevSelection || selection) && (isMenuVisible || selection.empty)) {
              // If selection hasn't changed but it's not empty, 
              // maybe still update button states (e.g., if marks changed programmatically)
              if (!selection.empty && currentMode === 'buttons') {
                  updateButtonStates(view);
              }
              return;
          }
          
          if (!selection.empty) {
            positionMenu(view);
          } else if (isMenuVisible) {
            hideMenu();
          }
        },
        destroy() {
          document.removeEventListener("mousedown", handleClickOutside, true);
          editorView.dom.removeEventListener("scroll", handleScroll);
          
          // Remove from actual scroll container
          const actualScrollContainer = editorView.dom.parentElement;
          if (actualScrollContainer && actualScrollContainer.classList.contains('scrollbar-thin')) {
            actualScrollContainer.removeEventListener("scroll", handleScroll);
          }
          
          // Remove from .editor-main
          const editorMainContainer = editorView.dom.closest(".editor-main");
          if (editorMainContainer) {
            editorMainContainer.removeEventListener("scroll", handleScroll);
          }
          
          if (menu && menu.parentNode) {
            menu.parentNode.removeChild(menu);
          }
          menu = null;
          view = null;
          // Clear references
          buttonsContainer = null;
          linkInputContainer = null;
          linkInput = null;
          linkDoneButton = null;
          linkButton = null;
        }
      };
    },
    // Add props to handle decorations
    props: {
      decorations(state) {
        if (pseudoSelectionDecoration) {
          // If we have a pseudo-selection, return it in a DecorationSet
          return DecorationSet.create(state.doc, [pseudoSelectionDecoration]);
        } else {
          // Otherwise, return an empty set or null
          return null; 
        }
      },
      // Handle clicks inside the pseudo-selection slightly differently
      // This might prevent accidentally clearing the input mode if clicking within the highlighted area
      handleClickOn(view, pos, node, nodePos, event) {
         if (currentMode === 'linkInput') {
             // If in link input mode, prevent the click from propagating 
             // and potentially causing the menu to hide via handleClickOutside
             event.stopPropagation();
             return true; // Indicate we handled the click
         }
         return false; // Default behavior
      },
      // Prevent editor from losing selection visually when clicking the menu
      handleDOMEvents: {
          mousedown: (view, event) => {
              // If the click is inside the floating menu, prevent ProseMirror's default
              // mousedown handling which might interfere with our selection preservation.
              if (menu?.contains(event.target as Node)) {
                  event.preventDefault(); 
                  return true; // We handled it
              }
              return false; // Let ProseMirror handle other clicks
          }
      }
    }
  });
} 