import { Plugin } from "prosemirror-state";
import { Schema } from "prosemirror-model";
import { EditorView } from "prosemirror-view";
import { updateButtonStates } from "./setup/buttonUtils";
import { updateAlignmentButtonStates } from "./setup/alignmentUtils";
import { dropdownStyle, activeItemStyle } from "./setup/dropdownUtils";
import { alignmentStyle } from "./setup/alignmentUtils";
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
} from "./setup/menuItems";

// Add styles to document
const style = document.createElement("style");
style.id = "prosemirror-menu-styles"; // Add an ID to avoid duplicate styles
style.textContent = dropdownStyle + activeItemStyle + alignmentStyle;

// CSS for the secondary menu and More Options button
style.textContent += `
  .editor-fixed-menu {
    height: auto;
  }
  .secondary-menu {
    width: 100%;
    padding: 5px;
    box-sizing: border-box; /* Include padding and border in width */
    display: flex;
    align-items: center;
    overflow: visible; /* Hide content during animation */
    visibility: hidden;
    max-height: 0; /* Start hidden */
    transition: max-height 0.2s ease-in;
    flex-wrap: wrap;
  }
  
  /* Fallback for browsers that don't support gap in flexbox */
  @supports not (gap: 2px) {
    .secondary-menu {
      margin: -2px;
    }
    
    .secondary-menu > * {
      margin: 2px;
    }
  }
  .secondary-menu.visible {
    max-height: 100px; /* Adjust as needed */
    padding: 10px 0px 0px 0px; /* Restore padding when visible */
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
    /* Basic styling for buttons - adjust as needed */
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
    /* background-color: #d0d0d0; */ /* Revert to default active state */
    /* border-color: #aaa; */
  }

  /* Styles for the Text Color Picker */
  .color-picker-dropdown {
    display: grid; 
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    padding: 12px;
    background: #16171f;
    border: 1px solid #2a2b2f;
    border-radius: 4px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    position: absolute;
    z-index: 100;
    width: auto;
  }
  
  /* Fallback for browsers that don't support gap in grid */
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

  /* Styles for Code Blocks */
  .ProseMirror pre {
    background-color: #2a2b2f; /* Slightly different background */
    color: #f0f0f0;           /* Light text color */
    font-family: 'Courier New', Courier, monospace; /* Monospace font */
    padding: 10px;            /* Padding inside the block */
    border-radius: 4px;       /* Rounded corners */
    margin: 1em 0;            /* Margin top/bottom */
    white-space: pre-wrap;    /* Wrap long lines */
    word-wrap: break-word;    /* Break long words */
  }

  /* Styles for Font Family Dropdown */
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
  
  /* Fallback for browsers that don't support gap in flexbox */
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

// Remove any existing style element with the same ID to avoid duplicates
const existingStyle = document.getElementById("prosemirror-menu-styles");
if (existingStyle) {
	existingStyle.remove();
}

document.head.appendChild(style);

// Create a custom menu plugin
export function fixedMenuPlugin(schema: Schema) {
	return new Plugin({
		view(editorView: EditorView) {
			// Create the main menu container
			const menuNode = document.createElement("div");
			menuNode.className = "editor-fixed-menu";

			// Create the secondary menu container
			const secondaryMenuNode = document.createElement("div");
			secondaryMenuNode.className = "secondary-menu";

			// Add menu items to the main menu
			addHistoryItems(menuNode, schema, editorView);
			addBlockFormatDropdown(menuNode, schema, editorView);
			addTextSizeControls(menuNode, schema, editorView);
			addFormattingItems(menuNode, schema, editorView); // Keep in main for now
			addListItems(menuNode, schema, editorView);
			addAlignmentButtons(menuNode, schema, editorView);
			

			// Add "More Options" button
			const moreOptionsButton = document.createElement("button");
			moreOptionsButton.className = "menu-button more-options-button";
			moreOptionsButton.innerHTML = `
<svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
<path d="M5 10.5C5.82843 10.5 6.5 11.1716 6.5 12C6.5 12.8284 5.82843 13.5 5 13.5C4.17157 13.5 3.5 12.8284 3.5 12C3.5 11.1716 4.17157 10.5 5 10.5ZM12 10.5C12.8284 10.5 13.5 11.1716 13.5 12C13.5 12.8284 12.8284 13.5 12 13.5C11.1716 13.5 10.5 12.8284 10.5 12C10.5 11.1716 11.1716 10.5 12 10.5ZM19 10.5C19.8284 10.5 20.5 11.1716 20.5 12C20.5 12.8284 19.8284 13.5 19 13.5C18.1716 13.5 17.5 12.8284 17.5 12C17.5 11.1716 18.1716 10.5 19 10.5Z" fill="#85889C"/>
</svg>
`;
			moreOptionsButton.title = "More options";
			moreOptionsButton.onclick = (e) => {
				e.preventDefault();
				secondaryMenuNode.classList.toggle("visible");
				if (secondaryMenuNode.classList.contains("visible")) {
					updateButtonStates(secondaryMenuNode, editorView);
					updateAlignmentButtonStates(secondaryMenuNode, editorView.state);
				}
			};
			menuNode.appendChild(moreOptionsButton);

			// Add placeholder items to the secondary menu
			addIndentButtons(secondaryMenuNode, schema, editorView);
			addSecondaryFormattingItems(secondaryMenuNode, schema, editorView);
			addBlockStyleItems(secondaryMenuNode, schema, editorView);
			addTextColorPicker(secondaryMenuNode, schema, editorView);
			addFontFamilyDropdown(secondaryMenuNode, schema, editorView);
			// --- Add other secondary menu items here in the future ---

			// Insert the menus into the DOM
			menuNode.appendChild(secondaryMenuNode); // Append secondary menu INSIDE main menu

			const editorContainer = editorView.dom.closest(".editor-container");
			if (editorContainer) {
				// Insert main menu first, then secondary menu right after
				editorContainer.insertBefore(menuNode, editorContainer.firstChild);
			} else if (editorView.dom.parentNode) {
				// Fallback if no .editor-container found
				editorView.dom.parentNode.insertBefore(menuNode, editorView.dom);
			}

			// Return the plugin view
			return {
				update(view) {
					// Update active states for main menu items
					updateButtonStates(menuNode, view);
					updateAlignmentButtonStates(menuNode, view.state);

					// Update secondary menu items only if visible
					if (secondaryMenuNode.classList.contains("visible")) {
						updateButtonStates(secondaryMenuNode, view);
						updateAlignmentButtonStates(secondaryMenuNode, view.state);
					}
				},
				destroy() {
					if (menuNode.parentNode) {
						menuNode.parentNode.removeChild(menuNode);
					}
					// Secondary menu is removed when menuNode is removed
				},
			};
		},
	});
}
