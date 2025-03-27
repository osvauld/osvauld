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
			addBlockItems(menuNode, schema, editorView);
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

// Add text formatting buttons (bold, italic, code)
function addFormattingItems(container, schema, view) {
	const group = document.createElement("div");
	group.className = "editor-menu-group";

	// Bold
	if (schema.marks.strong) {
		const boldButton = createButton("B", "Bold", () => {
			toggleMark(schema.marks.strong)(view.state, view.dispatch);
			view.focus();
		});
		boldButton.classList.add("menu-bold");
		boldButton.dataset.markType = "strong";

		// Ensure it's not active by default
		if (view.state && view.state.selection) {
			const isActive = markActive(view.state, schema.marks.strong);
			boldButton.classList.toggle("editor-menuitem-active", isActive);
		}

		group.appendChild(boldButton);
	}

	// Italic
	if (schema.marks.em) {
		const italicButton = createButton("I", "Italic", () => {
			toggleMark(schema.marks.em)(view.state, view.dispatch);
			view.focus();
		});
		italicButton.classList.add("menu-italic");
		italicButton.dataset.markType = "em";
		group.appendChild(italicButton);
	}

	// Code
	if (schema.marks.code) {
		const codeButton = createButton("</>", "Code", () => {
			toggleMark(schema.marks.code)(view.state, view.dispatch);
			view.focus();
		});
		codeButton.classList.add("menu-code");
		codeButton.dataset.markType = "code";
		group.appendChild(codeButton);
	}

	if (group.children.length > 0) {
		container.appendChild(group);
	}
}

// Add block formatting buttons (headings, paragraph)
function addBlockItems(container, schema, view) {
	const group = document.createElement("div");
	group.className = "editor-menu-group";

	// Paragraph
	if (schema.nodes.paragraph) {
		const paragraphButton = createButton("¶", "Paragraph", () => {
			setBlockType(schema.nodes.paragraph)(view.state, view.dispatch);
			view.focus();
		});
		paragraphButton.dataset.nodeType = "paragraph";
		group.appendChild(paragraphButton);
	}

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

// Add history buttons (undo/redo)
function addHistoryItems(container, schema, view) {
	const group = document.createElement("div");
	group.className = "editor-menu-group";

	// Undo
	const undoButton = createButton("↶ Undo", "Undo last change", () => {
		undo(view.state, view.dispatch);
		view.focus();
	});
	group.appendChild(undoButton);

	// Redo
	const redoButton = createButton("↷ Redo", "Redo last undone change", () => {
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
}
