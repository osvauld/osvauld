import { Plugin, PluginKey } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { Schema, Mark } from "prosemirror-model";
import { toggleMark } from "prosemirror-commands";
import { Decoration, DecorationSet } from "prosemirror-view";
import { getSearchState } from "prosemirror-search";

export const floatingMenuKey = new PluginKey("floating-menu");

type MenuMode = "buttons" | "linkInput";

// CSS-in-JS Styles
const FLOATING_MENU_STYLES = `
  .floating-menu {
    position: fixed;
    z-index: 50;
    background-color: #16171f;
    border: 1px solid #2a2b2f;
    border-radius: 10px;
    padding: 6px;
    display: none;
    gap: 4px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    opacity: 0;
    transform: translateY(8px);
    transition: opacity 0.15s ease, transform 0.15s ease;
  }

  .floating-menu.visible {
    opacity: 1;
    transform: translateY(0);
  }

  .floating-menu-buttons {
    display: flex;
    gap: 4px;
  }

  .floating-menu-button {
    background-color: #2a2b2f;
    color: #bfc0cc;
    border: none;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.875rem;
    font-weight: 500;
    min-width: 30px;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 28px;
    transition: background-color 0.15s ease;
  }

  .floating-menu-button:hover {
    background-color: #2a2b2f;
  }

  .floating-menu-button.active {
    background-color: #2a2b2f;
  }

  .floating-menu-link-input {
    display: none;
    gap: 4px;
    align-items: center;
  }

  .floating-menu-input {
    flex-grow: 1;
    padding: 4px 8px;
    border: 1px solid #2a2b2f;
    border-radius: 4px;
    background-color: #16171f;
    color: #bfc0cc;
    font-size: 0.875rem;
    outline: none;
  }

  .floating-menu-input:focus {
    border-color: #3a3b44;
  }

  .floating-menu-done-button {
    background-color: #2a2b2f;
    border-radius: 4px;
    font-size: 0.875rem;
    font-weight: 500;
    color: #bfc0cc;
    padding: 4px 8px;
    border: none;
    cursor: pointer;
    transition: background-color 0.15s ease;
  }

  .floating-menu-done-button:hover {
    background-color: #3a3b44;
  }

  .pseudo-selection {
    background-color: rgba(74, 175, 80, 0.2);
    border-radius: 2px;
  }
`;

// Style injection function
function injectStyles() {
  // Check if styles are already injected
  if (document.querySelector("#floating-menu-styles")) {
    return;
  }

  const styleElement = document.createElement("style");
  styleElement.id = "floating-menu-styles";
  styleElement.textContent = FLOATING_MENU_STYLES;
  document.head.appendChild(styleElement);
}

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

  // Inject styles when plugin is created
  injectStyles();

  // Create the menu element and its internal structure
  function createMenu() {
    if (menu) return menu;

    menu = document.createElement("div");
    menu.className = "floating-menu";

    // --- Buttons Container ---
    buttonsContainer = document.createElement("div");
    buttonsContainer.className = "floating-menu-buttons";

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
      const italicButton = createButton("Italic", `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" >
        <path d="m16.7 4.7-.1.9h-.3c-.6 0-1 0-1.4.3-.3.3-.4.6-.5 1.1l-2.1 9.8v.6c0 .5.4.8 1.4.8h.2l-.2.8H8l.2-.8h.2c1.1 0 1.8-.5 2-1.5l2-9.8.1-.5c0-.6-.4-.8-1.4-.8h-.3l.2-.9h5.8Z" fill-rule="evenodd" fill="currentColor">
        </path>
      </svg>`, "em", () => {
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
        `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg">
<path fill-rule="evenodd" clip-rule="evenodd" d="M6.77 21.7C6.925 21.765 7.09 21.795 7.25 21.795L7.255 21.79C7.575 21.79 7.895 21.665 8.135 21.425L11.56 18H19.25C20.765 18 22 16.765 22 15.25V5.75C22 4.235 20.765 3 19.25 3H4.75C3.235 3 2 4.235 2 5.75V15.25C2 16.765 3.235 18 4.75 18H6V20.545C6 21.055 6.3 21.505 6.77 21.7ZM3.5 5.75C3.5 5.06 4.06 4.5 4.75 4.5H19.25C19.94 4.5 20.5 5.06 20.5 5.75V15.25C20.5 15.94 19.94 16.5 19.25 16.5H10.94L7.5 19.94V16.5H4.75C4.06 16.5 3.5 15.94 3.5 15.25V5.75ZM17.5 8H6.5V9.5H17.5V8ZM13.5 11.5H6.5V13H13.5V11.5Z"/>
</svg>
`, 
        "comment", 
        handleCommentButtonClick
      );
      buttonsContainer.appendChild(commentButton);
    }
    menu.appendChild(buttonsContainer);

    // --- Link Input Container (initially hidden) ---
    linkInputContainer = document.createElement("div");
    linkInputContainer.className = "floating-menu-link-input";

    linkInput = document.createElement("input");
    linkInput.type = "text";
    linkInput.placeholder = "Enter Link";
    linkInput.className = "floating-menu-input";
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
    linkDoneButton.className = "floating-menu-done-button";
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
    if (!buttonsContainer || !linkInputContainer || !linkInput || !linkDoneButton || !view) return;
    
    const { state, dispatch } = view;
    const { selection } = state;
    const { $from, from, to } = selection;
    
    // Create and store pseudo-selection decoration
    pseudoSelectionDecoration = Decoration.inline(from, to, { class: 'pseudo-selection' });
    // Trigger a view update to render the decoration
    dispatch(state.tr); // Dispatch an empty transaction just to update decorations

    // Pre-fill input with existing link if present
    let existingMark: Mark | null = null;
    
    if (!selection.empty) {
      // For non-empty selections, check if the entire range has the link mark
      state.doc.nodesBetween(from, to, (node, pos) => {
        if (!existingMark && node.isText) {
          const linkMark = schema.marks.link.isInSet(node.marks);
          if (linkMark) {
            existingMark = linkMark;
            return false; // Stop iteration once we find a link mark
          }
        }
      });
    } else {
      // For cursor position, check stored marks or marks at the position
      existingMark = schema.marks.link.isInSet($from.marks()) || null;
    }
    
    const hasExistingLink = !!existingMark;
    linkInput.value = existingMark?.attrs.href || "";

    // Configure input and button based on whether this is an existing link
    if (hasExistingLink) {
      // Read-only mode for existing links
      linkInput.readOnly = true;
      linkInput.placeholder = "Link URL (read-only)";
      linkDoneButton.style.display = "none";
    } else {
      // Editable mode for new links
      linkInput.readOnly = false;
      linkInput.placeholder = "Enter Link";
      linkDoneButton.style.display = "block";
    }

    buttonsContainer.style.display = "none";
    linkInputContainer.style.display = "flex";
    currentMode = "linkInput";
    linkInput.focus(); // Focus the input
    if (hasExistingLink) {
      linkInput.select(); // Select text for easy copying
    }
  }

  function switchToButtonsMode() {
    if (!buttonsContainer || !linkInputContainer || !linkInput || !linkDoneButton) return;
    
    // Reset link input to default editable state
    linkInput.readOnly = false;
    linkInput.placeholder = "Enter Link";
    linkDoneButton.style.display = "block";
    
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
    });
  }

  function hideMenu() {
    if (!menu || !isMenuVisible) return;
    menu.classList.remove("visible");
    
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
         if (buttonsContainer) buttonsContainer.style.display = "flex";
         if (linkInputContainer) linkInputContainer.style.display = "none";
      }
    }, 150);
    isMenuVisible = false;
  }

  function showMenu() {
    if (!menu || isMenuVisible) return;
    menu.style.display = "flex";
    // Ensure correct UI is visible based on mode *before* showing
    if (currentMode === 'buttons') {
        if (buttonsContainer) buttonsContainer.style.display = "flex";
        if (linkInputContainer) linkInputContainer.style.display = "none";
    } else {
        if (buttonsContainer) buttonsContainer.style.display = "none";
        if (linkInputContainer) linkInputContainer.style.display = "flex";
    }
    // Force a reflow
    menu.getBoundingClientRect();
    menu.classList.add("visible");
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

    // Don't show menu during search navigation
    const searchQuery = getSearchState(state);
    if (searchQuery && searchQuery.query && searchQuery.query.search) {
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