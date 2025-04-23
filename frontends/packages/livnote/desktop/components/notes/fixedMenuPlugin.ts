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
    padding: 8px;
    margin: 2px;
    cursor: pointer;
    border-radius: 3px;
  }
  .menu-button:hover {
    /* background-color: #e0e0e0; */ /* Revert to default hover state */
    /* border-color: #bbb; */
  }
  .menu-button.active {
    /* background-color: #d0d0d0; */ /* Revert to default active state */
    /* border-color: #aaa; */
  }
  .more-options-button {
    font-weight: bold;
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
			moreOptionsButton.innerHTML = "&#8942;"; // Ellipsis character
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
