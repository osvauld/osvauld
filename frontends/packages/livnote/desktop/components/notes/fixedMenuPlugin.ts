import { Plugin } from "prosemirror-state";
import { MenuItem } from "prosemirror-menu";
import { EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import {
	toggleMark,
	lift,
	joinUp,
	setBlockType,
	wrapIn,
} from "prosemirror-commands";
import { wrapInList } from "prosemirror-schema-list";
import { undo, redo } from "prosemirror-history";

// Helper function to check if a mark is active
function markActive(state, type) {
	// Simplified approach that checks if a mark is active at the current selection
	try {
		const { from, $from, to, empty } = state.selection;

		if (empty) {
			// When cursor is at a single position
			return (
				type.isInSet($from.marks()) ||
				(state.storedMarks && type.isInSet(state.storedMarks))
			);
		} else {
			// When there's a text selection
			return state.doc.rangeHasMark(from, to, type);
		}
	} catch (e) {
		// In case of any errors, default to not active
		console.error("Error checking mark active state:", e);
		return false;
	}
}

// Helper function to check if node type is active at selection
function nodeActive(state, type, attrs = {}) {
	const { selection } = state;
	const { $from, $to, node } = selection;

	if (node) {
		return node.hasMarkup(type, attrs);
	}

	let active = false;
	if ($from.depth > 0) {
		// Check if the current parent node is of this type
		const parent = $from.node($from.depth);
		if (parent.type === type) {
			// For heading, check also the level attribute
			if (type.name === "heading" && attrs.level !== undefined) {
				active = parent.attrs.level === attrs.level;
			} else {
				active = true;
			}
		}
	}

	return active;
}

function addFormattingItems(container, schema, view) {
	const group = document.createElement("div");
	group.className = "editor-menu-group";

	// Bold
	if (schema.marks.strong) {
		const boldButton = document.createElement("button");
		boldButton.className = "editor-general-button menu-bold";
		boldButton.title = "Bold";
		boldButton.dataset.markType = "strong";
		boldButton.innerHTML = `
		<svg width="24" height="24" focusable="false">
		  <path d="M7.8 19c-.3 0-.5 0-.6-.2l-.2-.5V5.7c0-.2 0-.4.2-.5l.6-.2h5c1.5 0 2.7.3 3.5 1 .7.6 1.1 1.4 1.1 2.5a3 3 0 0 1-.6 1.9c-.4.6-1 1-1.6 1.2.4.1.9.3 1.3.6s.8.7 1 1.2c.4.4.5 1 .5 1.6 0 1.3-.4 2.3-1.3 3-.8.7-2.1 1-3.8 1H7.8Zm5-8.3c.6 0 1.2-.1 1.6-.5.4-.3.6-.7.6-1.3 0-1.1-.8-1.7-2.3-1.7H9.3v3.5h3.4Zm.5 6c.7 0 1.3-.1 1.7-.4.4-.4.6-.9.6-1.5s-.2-1-.7-1.4c-.4-.3-1-.4-2-.4H9.4v3.8h4Z" fill-rule="evenodd" fill="#85889C">
		  </path>
		</svg>
	  `;
		boldButton.addEventListener("click", () => {
			toggleMark(schema.marks.strong)(view.state, view.dispatch);
			view.focus();
		});

		// Ensure it's not active by default
		if (view.state && view.state.selection) {
			const isActive = markActive(view.state, schema.marks.strong);
			boldButton.classList.toggle("editor-menuitem-active", isActive);
		}
		group.appendChild(boldButton);
	}

	// Italic
	if (schema.marks.em) {
		const italicButton = document.createElement("button");
		italicButton.className = "editor-general-button menu-italic";
		italicButton.title = "Italic";
		italicButton.dataset.markType = "em";
		italicButton.innerHTML = `
		<svg width="24" height="24" focusable="false">
		  <path d="m16.7 4.7-.1.9h-.3c-.6 0-1 0-1.4.3-.3.3-.4.6-.5 1.1l-2.1 9.8v.6c0 .5.4.8 1.4.8h.2l-.2.8H8l.2-.8h.2c1.1 0 1.8-.5 2-1.5l2-9.8.1-.5c0-.6-.4-.8-1.4-.8h-.3l.2-.9h5.8Z" fill-rule="evenodd" fill="#85889C">
		  </path>
		</svg>
	  `;
		italicButton.addEventListener("click", () => {
			toggleMark(schema.marks.em)(view.state, view.dispatch);
			view.focus();
		});
		group.appendChild(italicButton);
	}

	// Code
	// if (schema.marks.code) {
	// 	const codeButton = createButton("</>", "Code", () => {
	// 		toggleMark(schema.marks.code)(view.state, view.dispatch);
	// 		view.focus();
	// 	});
	// 	codeButton.classList.add("menu-code");
	// 	codeButton.dataset.markType = "code";
	// 	group.appendChild(codeButton);
	// }

	if (group.children.length > 0) {
		container.appendChild(group);
	}
}

// Add block formatting buttons (headings, paragraph)
function addBlockItems(container, schema, view) {
	const group = document.createElement("div");
	group.className = "editor-menu-group";

	// // Paragraph
	// if (schema.nodes.paragraph) {
	// 	const paragraphButton = createButton("¶", "Paragraph", () => {
	// 		setBlockType(schema.nodes.paragraph)(view.state, view.dispatch);
	// 		view.focus();
	// 	});
	// 	paragraphButton.dataset.nodeType = "paragraph";
	// 	group.appendChild(paragraphButton);
	// }

	// Headings
	if (schema.nodes.heading) {
		for (let level = 1; level <= 3; level++) {
			const headingButton = createButton(
				`H${level}`,
				`Heading ${level}`,
				() => {
					setBlockType(schema.nodes.heading, { level })(
						view.state,
						view.dispatch,
					);
					view.focus();
				},
			);
			headingButton.dataset.nodeType = "heading";
			headingButton.dataset.level = level.toString();
			group.appendChild(headingButton);
		}
	}

	if (group.children.length > 0) {
		container.appendChild(group);
	}
}

// Add list and quote buttons
function addListItems(container, schema, view) {
	const group = document.createElement("div");
	group.className = "editor-menu-group";

	// Bullet list
	if (schema.nodes.bullet_list) {
		const bulletListButton = createButton("• List", "Bullet list", () => {
			wrapInList(schema.nodes.bullet_list)(view.state, view.dispatch);
			view.focus();
		});
		bulletListButton.dataset.nodeType = "bullet_list";
		group.appendChild(bulletListButton);
	}

	// Ordered list
	if (schema.nodes.ordered_list) {
		const orderedListButton = createButton("1. List", "Ordered list", () => {
			wrapInList(schema.nodes.ordered_list)(view.state, view.dispatch);
			view.focus();
		});
		orderedListButton.dataset.nodeType = "ordered_list";
		group.appendChild(orderedListButton);
	}

	// Blockquote
	if (schema.nodes.blockquote) {
		const blockquoteButton = createButton("Quote", "Blockquote", () => {
			wrapIn(schema.nodes.blockquote)(view.state, view.dispatch);
			view.focus();
		});
		blockquoteButton.dataset.nodeType = "blockquote";
		group.appendChild(blockquoteButton);
	}

	// Lift (outdent)
	const liftButton = createButton(
		"↑ Lift",
		"Lift out of enclosing block",
		() => {
			lift(view.state, view.dispatch);
			view.focus();
		},
	);
	group.appendChild(liftButton);

	// Join with the block above
	const joinButton = createButton("↕ Join", "Join with above block", () => {
		joinUp(view.state, view.dispatch);
		view.focus();
	});
	group.appendChild(joinButton);

	if (group.children.length > 0) {
		container.appendChild(group);
	}
}

function addHistoryItems(container, schema, view) {
	const group = document.createElement("div");
	group.className = "editor-menu-group";

	// Undo button with SVG
	const undoButton = document.createElement("button");
	undoButton.className = "editor-general-button"; // Assuming you have this class
	undoButton.title = "Undo last change";
	undoButton.innerHTML = `
	  <svg width="24" height="24" focusable="false"><path d="M6.4 8H12c3.7 0 6.2 2 6.8 5.1.6 2.7-.4 5.6-2.3 6.8a1 1 0 0 1-1-1.8c1.1-.6 1.8-2.7 1.4-4.6-.5-2.1-2.1-3.5-4.9-3.5H6.4l3.3 3.3a1 1 0 1 1-1.4 1.4l-5-5a1 1 0 0 1 0-1.4l5-5a1 1 0 0 1 1.4 1.4L6.4 8Z" fill-rule="nonzero" fill="#85889C"></path></svg>
	`;
	undoButton.addEventListener("click", () => {
		undo(view.state, view.dispatch);
		view.focus();
	});
	group.appendChild(undoButton);

	// Redo button with SVG
	const redoButton = document.createElement("button");
	redoButton.className = "editor-general-button"; // Assuming you have this class
	redoButton.title = "Redo last undone change";
	redoButton.innerHTML = `
	<svg width="24" height="24" focusable="false"><path d="M17.6 10H12c-2.8 0-4.4 1.4-4.9 3.5-.4 2 .3 4 1.4 4.6a1 1 0 1 1-1 1.8c-2-1.2-2.9-4.1-2.3-6.8.6-3 3-5.1 6.8-5.1h5.6l-3.3-3.3a1 1 0 1 1 1.4-1.4l5 5a1 1 0 0 1 0 1.4l-5 5a1 1 0 0 1-1.4-1.4l3.3-3.3Z" fill-rule="nonzero" fill="#85889C"></path></svg>
	`;
	redoButton.addEventListener("click", () => {
		redo(view.state, view.dispatch);
		view.focus();
	});
	group.appendChild(redoButton);

	if (group.children.length > 0) {
		container.appendChild(group);
	}
}

// Create a button with text, title and click handler
function createButton(text, title, onClick) {
	const button = document.createElement("button");
	button.className = "editor-menuitem";
	button.textContent = text;
	button.title = title;
	button.type = "button";
	button.addEventListener("click", onClick);

	// Ensure it's not active by default - explicitly remove the active class
	button.classList.remove("editor-menuitem-active");

	return button;
}

// Update active/disabled states for menu items
// function updateButtonStates(menuNode, view) {
// 	const { state } = view;
// 	const { schema } = state.doc.type;

// 	// Update mark buttons (bold, italic, code)
// 	menuNode.querySelectorAll("[data-mark-type]").forEach((button) => {
// 		try {
// 			const markName = button.dataset.markType;
// 			if (!markName || !schema.marks[markName]) return;

// 			const markType = schema.marks[markName];
// 			// First remove active class to ensure clean state
// 			button.classList.remove("editor-menuitem-active");

// 			// Only add active class if actually active
// 			const isActive = markActive(state, markType);
// 			if (isActive) {
// 				button.classList.add("editor-menuitem-active");
// 			}
// 		} catch (e) {
// 			console.error("Error updating mark button state:", e);
// 		}
// 	});

// 	// Update node type buttons (headings, paragraph)
// 	menuNode.querySelectorAll("[data-node-type]").forEach((button) => {
// 		const nodeName = button.dataset.nodeType;
// 		const nodeType = schema.nodes[nodeName];

// 		if (nodeName === "heading" && button.dataset.level) {
// 			const level = parseInt(button.dataset.level);
// 			const isActive = nodeActive(state, nodeType, { level });
// 			button.classList.toggle("editor-menuitem-active", isActive);
// 		} else {
// 			// For paragraph and other block nodes
// 			const isActive = nodeActive(state, nodeType);
// 			button.classList.toggle("editor-menuitem-active", isActive);
// 		}
// 	});
// }

function updateButtonStates(menuNode, view) {
	const { state } = view;
	const { schema } = state.doc.type;

	// Update mark buttons (bold, italic, code)
	menuNode.querySelectorAll("[data-mark-type]").forEach((button) => {
		try {
			const markName = button.dataset.markType;
			if (!markName || !schema.marks[markName]) return;

			const markType = schema.marks[markName];
			// First remove active class to ensure clean state
			button.classList.remove("editor-menuitem-active");

			// Only add active class if actually active
			const isActive = markActive(state, markType);
			if (isActive) {
				button.classList.add("editor-menuitem-active");
			}
		} catch (e) {
			console.error("Error updating mark button state:", e);
		}
	});

	// Update node type buttons (headings, paragraph)
	menuNode.querySelectorAll("[data-node-type]").forEach((button) => {
		const nodeName = button.dataset.nodeType;
		const nodeType = schema.nodes[nodeName];

		if (nodeName === "heading" && button.dataset.level) {
			const level = parseInt(button.dataset.level);
			const isActive = nodeActive(state, nodeType, { level });
			button.classList.toggle("editor-menuitem-active", isActive);
		} else {
			// For paragraph and other block nodes
			const isActive = nodeActive(state, nodeType);
			button.classList.toggle("editor-menuitem-active", isActive);
		}
	});

	// Also update active states in dropdown menus
	const currentHeadingLevel = getCurrentHeadingLevel(state, schema);
	if (currentHeadingLevel) {
		const submenuItems = menuNode.querySelectorAll(".submenu-item");
		submenuItems.forEach((item) => {
			item.classList.remove("active-menuitem");
			if (item.textContent?.includes(`Heading ${currentHeadingLevel}`)) {
				item.classList.add("active-menuitem");
			}
		});
	}
}

function getCurrentHeadingLevel(state, schema) {
	const { selection } = state;
	const { $from } = selection;

	if ($from.depth > 0) {
		const parent = $from.node($from.depth);
		if (parent.type === schema.nodes.heading) {
			return parent.attrs.level;
		}
	}
	return null;
}

// Add CSS for active dropdown items
const activeItemStyle = document.createElement("style");
activeItemStyle.textContent = `
  .active-menuitem {
    background-color: #3a3b44;
    box-shadow: 2px 0 0 0 rgb(124 145 249 / 1) inset;
  }
`;
document.head.appendChild(activeItemStyle);

// Add dropdown format menu
function addFormatDropdown(container, schema, view) {
	const group = document.createElement("div");
	group.className = "editor-menu-group";

	// Create the main dropdown button
	const dropdownContainer = document.createElement("div");
	dropdownContainer.className = "dropdown-container";

	const formatButton = document.createElement("button");
	formatButton.className = "format-dropdown-button";
	formatButton.innerHTML = `
	  <span>Formats</span>
	  <svg width="12" height="12" viewBox="0 0 24 24" focusable="false">
		<path d="M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z" fill="#85889C"></path>
	  </svg>
	`;

	// Create dropdown menu
	const dropdownMenu = document.createElement("div");
	dropdownMenu.className = "dropdown-menu";
	dropdownMenu.style.display = "none";

	// Add menu items
	const headingsItem = document.createElement("div");
	headingsItem.className = "dropdown-item has-submenu";
	headingsItem.innerHTML = `
	  <span>Headings</span>
	  <svg width="12" height="12" viewBox="0 0 24 24" focusable="false">
		<path d="M10 6 8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z" fill="#85889C"></path>
	  </svg>
	`;

	// Create submenu for headings
	const headingsSubmenu = document.createElement("div");
	headingsSubmenu.className = "submenu";

	// Add heading options (H1-H6)
	const headingLevels = [
		{ level: 1, text: "Level 1" },
		{ level: 2, text: "Level 2" },
		{ level: 3, text: "Level 3" },
		{ level: 4, text: "Level 4" },
		{ level: 5, text: "Level 5" },
		{ level: 6, text: "Level 6" },
	];

	headingLevels.forEach((heading) => {
		const headingOption = document.createElement("div");
		headingOption.className = "submenu-item";
		headingOption.textContent = heading.text;
		headingOption.addEventListener("click", (e) => {
			e.stopPropagation();
			setBlockType(schema.nodes.heading, { level: heading.level })(
				view.state,
				view.dispatch,
			);
			view.focus();
			hideDropdowns();
		});
		headingsSubmenu.appendChild(headingOption);
	});

	headingsItem.appendChild(headingsSubmenu);
	dropdownMenu.appendChild(headingsItem);

	// Add other menu items
	const items = [
		{ text: "Inline", hasSubmenu: true },
		{ text: "Blocks", hasSubmenu: true },
		{ text: "Alignment", hasSubmenu: true },
	];

	items.forEach((item) => {
		const menuItem = document.createElement("div");
		menuItem.className = "dropdown-item";
		if (item.hasSubmenu) {
			menuItem.classList.add("has-submenu");
			menuItem.innerHTML = `
		  <span>${item.text}</span>
		  <svg width="12" height="12" viewBox="0 0 24 24" focusable="false">
			<path d="M10 6 8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z" fill="#85889C"></path>
		  </svg>
		`;
		} else {
			menuItem.textContent = item.text;
		}
		dropdownMenu.appendChild(menuItem);
	});

	// Toggle dropdown on click
	formatButton.addEventListener("click", () => {
		const isVisible = dropdownMenu.style.display === "block";
		hideDropdowns();
		if (!isVisible) {
			dropdownMenu.style.display = "block";
		}
	});

	// Hide all dropdowns when clicking elsewhere
	function hideDropdowns() {
		const dropdowns = document.querySelectorAll(".dropdown-menu, .submenu");
		dropdowns.forEach((dropdown) => {
			dropdown.style.display = "none";
		});
	}

	// Show submenu on hover
	headingsItem.addEventListener("mouseenter", () => {
		const submenu = headingsItem.querySelector(".submenu");
		if (submenu) {
			submenu.style.display = "block";
		}
	});

	// Close dropdown when clicking outside
	document.addEventListener("click", (e) => {
		if (!dropdownContainer.contains(e.target)) {
			hideDropdowns();
		}
	});

	// Add everything to the DOM
	dropdownContainer.appendChild(formatButton);
	dropdownContainer.appendChild(dropdownMenu);
	group.appendChild(dropdownContainer);
	container.appendChild(group);
}

// Add this to your CSS
const style = document.createElement("style");
style.textContent = `
	.dropdown-container {
	  position: relative;
	  display: inline-block;
	}
	
	.format-dropdown-button {
	  display: flex;
	  align-items: center;
	  gap: 5px;
	
	  color: #bfc0cc;
	  border: none;
	  padding: 6px 12px;
	  border-radius: 4px;
	  cursor: pointer;
	  font-size: 14px;
	}
	
	.format-dropdown-button:hover {
	    background: #2a2b2f;
	}
	
	.dropdown-menu {
	  position: absolute;
	  top: 110%;
	  left: 0px;
	  min-width: 160px;
	  background: #2a2b2f;
	  font-size: 14px;
	  z-index: 100;
	}
	
	.dropdown-item {
	  padding: 8px 12px;
	  cursor: pointer;
	  color: #bfc0cc;
	  display: flex;
	  justify-content: space-between;
	  align-items: center;
	}
	
	.dropdown-item:hover {
	  background: #3a3b44;
	}
	
	.has-submenu {
	  position: relative;
	}
	
	.submenu {
	  position: absolute;
	  left: 100%;
	  top: 0;
	  min-width: 160px;
	  background: #2a2b2f;
	  font-size: 14px;
	  display: none;
	  z-index: 101;
	}
	
	.submenu-item {
	  padding: 8px 12px;
	  cursor: pointer;
	  color: #bfc0cc;
	}
	
	.submenu-item:hover {
	  background: #3a3b44;
	}
  `;
document.head.appendChild(style);

// Create a custom menu plugin
export function fixedMenuPlugin(schema: Schema) {
	return new Plugin({
		view(editorView) {
			// Create the menu container
			const menuNode = document.createElement("div");
			menuNode.className = "editor-fixed-menu";

			// Add menu items
			addHistoryItems(menuNode, schema, editorView);
			addFormattingItems(menuNode, schema, editorView);
			addFormatDropdown(menuNode, schema, editorView);
			// addBlockItems(menuNode, schema, editorView);
			addListItems(menuNode, schema, editorView);

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
