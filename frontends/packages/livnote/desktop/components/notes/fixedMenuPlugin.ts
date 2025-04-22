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
} from "./setup/menuItems";

// Add styles to document
const style = document.createElement("style");
style.id = "prosemirror-menu-styles"; // Add an ID to avoid duplicate styles
style.textContent = dropdownStyle + activeItemStyle + alignmentStyle;

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
			// Create the menu container
			const menuNode = document.createElement("div");
			menuNode.className = "editor-fixed-menu";

			// Add menu items
			addHistoryItems(menuNode, schema, editorView);
			addBlockFormatDropdown(menuNode, schema, editorView);
			addTextSizeControls(menuNode, schema, editorView);
			addFormattingItems(menuNode, schema, editorView);
			addListItems(menuNode, schema, editorView);
			addAlignmentButtons(menuNode, schema, editorView);
			addIndentButtons(menuNode, schema, editorView);

			// Insert the menu at the top of the editor
			const editorContainer = editorView.dom.closest(".editor-container");
			if (editorContainer) {
				editorContainer.insertBefore(menuNode, editorContainer.firstChild);
			} else if (editorView.dom.parentNode) {
				editorView.dom.parentNode.insertBefore(menuNode, editorView.dom);
			}

			// Return the plugin view
			return {
				update(view) {
					// Update active states for menu items when the editor state changes
					updateButtonStates(menuNode, view);
					updateAlignmentButtonStates(menuNode, view.state);
				},
				destroy() {
					if (menuNode.parentNode) {
						menuNode.parentNode.removeChild(menuNode);
					}
				},
			};
		},
	});
}
