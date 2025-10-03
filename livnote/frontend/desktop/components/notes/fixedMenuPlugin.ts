import { Plugin } from "prosemirror-state";
import { Schema } from "prosemirror-model";
import { EditorView } from "prosemirror-view";
import { updateButtonStates } from "./setup/buttonUtils";
import {
  updateAlignmentButtonStates,
  alignmentStyle,
} from "./setup/alignmentUtils";
import {
  dropdownStyle,
  activeItemStyle,
  hideDropdowns,
} from "./setup/dropdownUtils";
import {
  addHistoryItems,
  addFormattingItems,
  addListItems,
  addIndentButtons,
  addAlignmentButtons,
  addBlockFormatDropdown,
  addTextSizeControls,
  addSecondaryFormattingItems,
  addBlockStyleItems,
  addTextColorPicker,
  addFontFamilyDropdown,
  updateBlockFormatButton,
  updateFontSizeDisplay,
  updateIndentButtonsState,
  updateSecondaryFormatButtons,
  updateTextColorButton,
  updateFontFamilyButton,
} from "./setup/menuItems";
import { addMathItems } from "./setup/mathMenuItems";

// Helper function to manage style injection and removal
function manageStyles(action: "create" | "destroy") {
  const styleId = "prosemirror-menu-styles";
  const existingStyle = document.getElementById(styleId);

  if (action === "create") {
    if (existingStyle) return; // Don't add if it already exists

    const style = document.createElement("style");
    style.id = styleId;
    // Combine all necessary styles
    style.textContent =
      dropdownStyle +
      activeItemStyle +
      alignmentStyle +
      `
      .editor-fixed-menu {
        height: auto;
      }
      
      .secondary-menu {
        width: 100%;
        padding: 5px;
        box-sizing: border-box;
        display: flex;
        align-items: center;
        overflow: visible;
        visibility: hidden;
        max-height: 0;
        flex-wrap: wrap;
      }
      
      @supports not (gap: 2px) {
        .secondary-menu {
          margin: -2px;
        }
        .secondary-menu > * {
          margin: 2px;
        }
      }
      
      .secondary-menu.visible {
        max-height: 100px;
        padding: 10px 0px 0px 0px;
        visibility: visible;
      }
      
      .secondary-menu.visible::before {
        content: "";
        margin-bottom: 10px;
        display: block;
        width: 100%;
        border-top: 1px solid #2a2b2f;
      }
      
      .menu-button {
        background: none;
        border: 1px solid transparent;
        padding: 4px;
        margin: 2px;
        cursor: pointer;
        border-radius: 3px;
      }
      
      .menu-button:hover {
        background-color: rgb(58, 59, 68);
      }
      
      .menu-button.active {
      }
      
      .dropdown-menu.color-picker-dropdown {
        grid-template-columns: repeat(3, 1fr);
        gap: 8px;
        padding: 12px;
        background: #16171f;
        border: 1px solid #2a2b2f;
        border-radius: 4px;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
        position: absolute;
        z-index: 100;
        width: 120px;
        min-width: auto;
      }
      
      @supports not (gap: 8px) {
        .color-picker-dropdown {
          grid-gap: 8px;
        }
        .color-picker-dropdown > * {
          margin: 4px;
        }
      }
      
      .color-swatch {
        width: 24px;
        height: 24px;
        border-radius: 4px;
        cursor: pointer;
        transition: transform 0.1s ease;
      }
      
      .color-swatch:hover {
        transform: scale(1.1);
      }
      
      .remove-color-button {
        grid-column: 1 / -1;
        margin-top: 8px;
        text-align: center;
        padding: 6px;
        background: #2a2b2f;
        border: none;
        border-radius: 4px;
        color: #bfc0cc;
        cursor: pointer;
      }
      
      .remove-color-button:hover {
        background: #3a3b44;
      }
      
      .text-color-button:disabled {
        opacity: 0.5;
      }
      
      .ProseMirror pre {
        background-color: #2a2b2f;
        color: #f0f0f0;
        font-family: 'Courier New', Courier, monospace;
        padding: 10px;
        border-radius: 4px;
        margin: 1em 0;
        white-space: pre-wrap;
        word-wrap: break-word;
      }
      
      .font-family-dropdown-button {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 6px 12px;
        border: 1px solid #3a3b44;
        border-radius: 4px;
        color: #bfc0cc;
        cursor: pointer;
        font-size: 14px;
        min-width: 100px;
        transition: background-color 0.1s ease;
      }
      
      @supports not (gap: 8px) {
        .font-family-dropdown-button > *:not(:last-child) {
          margin-right: 8px;
        }
      }
      
      .font-family-dropdown-button:hover {
        background: #3a3b44;
      }
      
      .font-family-dropdown-button:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }
      
      .font-family-dropdown-button .font-name {
        flex: 1;
        text-align: left;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }
      
      .font-family-dropdown {
        background: #16171f;
        border: 1px solid #2a2b2f;
        border-radius: 4px;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
        position: absolute;
        z-index: 100;
        width: 200px;
        max-height: 300px;
        overflow-y: auto;
        padding: 4px 0;
      }
      
      .font-family-item {
        padding: 8px 16px;
        cursor: pointer;
        transition: background-color 0.1s ease;
        color: #bfc0cc;
        border-bottom: 1px solid #2a2b2f;
      }
      
      .font-family-item:hover {
        background: #2a2b2f;
      }
      
      .font-family-item:last-of-type {
        border-bottom: none;
      }
      
      .remove-font-button {
        padding: 8px 16px;
        background: #2a2b2f;
        border: none;
        border-top: 1px solid #3a3b44;
        color: #bfc0cc;
        cursor: pointer;
        width: 100%;
        text-align: center;
        margin-top: 4px;
      }
      
      .remove-font-button:hover {
        background: #3a3b44;
      }
    `;
    document.head.appendChild(style);
  } else if (action === "destroy") {
    if (existingStyle) {
      existingStyle.remove();
    }
  }
}

export function fixedMenuPlugin(schema: Schema) {
  return new Plugin({
    view(editorView: EditorView) {
      manageStyles("create");

      // --- Centralized State & Cleanup ---
      const originalDispatch = editorView.dispatch;
      const listeners: {
        target: EventTarget;
        type: string;
        handler: EventListener;
      }[] = [];

      const registerListener = (
        target: EventTarget,
        type: string,
        handler: EventListener,
      ) => {
        target.addEventListener(type, handler);
        listeners.push({ target, type, handler });
      };

      // --- UI Creation ---
      const menuNode = document.createElement("div");
      menuNode.className = "editor-fixed-menu";
      const secondaryMenuNode = document.createElement("div");
      secondaryMenuNode.className = "secondary-menu";

      // Add menu items (now just UI factories)
      addHistoryItems(menuNode, schema, editorView);
      const blockFormatDropdown = addBlockFormatDropdown(menuNode, schema, editorView);
      const textSizeControls = addTextSizeControls(menuNode, schema, editorView);
      addFormattingItems(menuNode, schema, editorView);
      addListItems(menuNode, schema, editorView);
      addAlignmentButtons(menuNode, schema, editorView);
      // Add secondary menu items
      const indentButtons = addIndentButtons(secondaryMenuNode, schema, editorView);
      const secondaryFormatButtons = addSecondaryFormattingItems(secondaryMenuNode, schema, editorView);
      addBlockStyleItems(secondaryMenuNode, schema, editorView);
      const textColorPicker = addTextColorPicker(secondaryMenuNode, schema, editorView);
      addMathItems(secondaryMenuNode, schema, editorView);
      const fontFamilyDropdown = addFontFamilyDropdown(secondaryMenuNode, schema, editorView);


      // --- "More Options" Button ---
      const moreOptionsButton = document.createElement("button");
      moreOptionsButton.className = "menu-button more-options-button";
      moreOptionsButton.innerHTML = `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M5 10.5C5.82843 10.5 6.5 11.1716 6.5 12C6.5 12.8284 5.82843 13.5 5 13.5C4.17157 13.5 3.5 12.8284 3.5 12C3.5 11.1716 4.17157 10.5 5 10.5ZM12 10.5C12.8284 10.5 13.5 11.1716 13.5 12C13.5 12.8284 12.8284 13.5 12 13.5C11.1716 13.5 10.5 12.8284 10.5 12C10.5 11.1716 11.1716 10.5 12 10.5ZM19 10.5C19.8284 10.5 20.5 11.1716 20.5 12C20.5 12.8284 19.8284 13.5 19 13.5C18.1716 13.5 17.5 12.8284 17.5 12C17.5 11.1716 18.1716 10.5 19 10.5Z" fill="#85889C"/></svg>`;
      moreOptionsButton.title = "More options";
      moreOptionsButton.onclick = (e) => {
        e.preventDefault();
        secondaryMenuNode.classList.toggle("visible");
      };
      menuNode.appendChild(moreOptionsButton);

      menuNode.appendChild(secondaryMenuNode);

      // --- DOM Insertion ---
      const editorContainer = editorView.dom.closest(".editor-container");
      if (editorContainer) {
        editorContainer.insertBefore(menuNode, editorContainer.firstChild);
      } else if (editorView.dom.parentNode) {
        editorView.dom.parentNode.insertBefore(menuNode, editorView.dom);
      }

      // --- Centralized Update Logic ---
      const updateAllButtonStates = (view: EditorView) => {
        // General updates
        updateButtonStates(menuNode, view);
        updateAlignmentButtonStates(menuNode, view.state);

        // Specific component updates
        updateBlockFormatButton(blockFormatDropdown, schema, view);
        updateFontSizeDisplay(textSizeControls, schema, view);
        updateIndentButtonsState(indentButtons, schema, view);
        updateSecondaryFormatButtons(secondaryFormatButtons, schema, view);
        updateTextColorButton(textColorPicker, schema, view);
        updateFontFamilyButton(fontFamilyDropdown, schema, view);

        // Update secondary menu only if visible
        if (secondaryMenuNode.classList.contains("visible")) {
          updateButtonStates(secondaryMenuNode, view);
          updateAlignmentButtonStates(secondaryMenuNode, view.state);
        }
      };

      // --- Centralized Event Listeners ---
      const handleDocumentClick = (e: MouseEvent) => {
        const target = e.target as Node;
        if (!target.closest(".dropdown-container")) {
          hideDropdowns();
        }
      };
      registerListener(document, "click", handleDocumentClick as EventListener);

      const handleSelectionChange = () => updateAllButtonStates(editorView);
      registerListener(editorView.dom, "keyup", handleSelectionChange);
      registerListener(editorView.dom, "mouseup", handleSelectionChange);

      // --- Centralized Dispatch Wrapper ---
      (editorView as any).dispatch = (tr: any) => {
        originalDispatch(tr);
        if (tr.docChanged || tr.selectionSet) {
          updateAllButtonStates(editorView);
        }
      };

      // Initial state update
      updateAllButtonStates(editorView);

      return {
        update(view: EditorView) {
          // The dispatch wrapper handles most updates.
          // We might still call this on view updates that don't involve a transaction.
          updateAllButtonStates(view);
        },
        destroy() {
          // 1. Remove the menu DOM element
          if (menuNode.parentNode) {
            menuNode.parentNode.removeChild(menuNode);
          }

          // 2. Restore the original dispatch function
          (editorView as any).dispatch = originalDispatch;

          // 3. Remove all registered event listeners
          listeners.forEach(({ target, type, handler }) => {
            target.removeEventListener(type, handler);
          });

          // 4. Remove the injected styles
          manageStyles("destroy");
        },
      };
    },
  });
}
