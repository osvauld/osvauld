import { EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import { toggleMark, setBlockType, wrapIn } from "prosemirror-commands";
import { wrapInList } from "prosemirror-schema-list";
import { undo, redo } from "prosemirror-history";
import { indentRight, indentLeft } from "./indentUtils";
import { setTextAlign } from "./alignmentUtils";
import { createHeadingSubmenu, hideDropdowns } from "./dropdownUtils";
import { emit } from "@tauri-apps/api/event";

export function addHistoryItems(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";

  // Undo button with SVG
  const undoButton = document.createElement("button");
  undoButton.className = "editor-general-button";
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
  redoButton.className = "editor-general-button";
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

export function addFormattingItems(container: HTMLElement, schema: Schema, view: EditorView) {
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

  if (group.children.length > 0) {
    container.appendChild(group);
  }
}

export function addListItems(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";

  // Bullet list
  if (schema.nodes.bullet_list) {
    const bulletListButton = document.createElement("button");
    bulletListButton.className = "editor-general-button";
    bulletListButton.title = "Bullet list";
    bulletListButton.dataset.nodeType = "bullet_list";
    bulletListButton.innerHTML = `
     <svg width="24" height="24" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" fill="currentColor">
    <g transform="translate(140, 60)">
        <path d="M0 512h128v-128h-128v128zM0 256h128v-128h-128v128zM0 768h128v-128h-128v128zM256 512h512v-128h-512v128zM256 256h512v-128h-512v128zM256 768h512v-128h-512v128z" fill="#85889C"/>
</g>
      </svg>
    `;
    bulletListButton.addEventListener("click", () => {
      wrapInList(schema.nodes.bullet_list)(view.state, view.dispatch);
      view.focus();
    });
    group.appendChild(bulletListButton);
  }

  // Ordered list
  if (schema.nodes.ordered_list) {
    const orderedListButton = document.createElement("button");
    orderedListButton.className = "editor-general-button";
    orderedListButton.title = "Ordered list";
    orderedListButton.dataset.nodeType = "ordered_list";
    orderedListButton.innerHTML = `
      <svg width="24" height="24" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" fill="currentColor">
  <g transform="translate(140, 60)">
        <path d="M320 512h448v-128h-448v128zM320 768h448v-128h-448v128zM320 128v128h448v-128h-448zM79 384h78v-256h-36l-85 23v50l43-2v185zM189 590c0-36-12-78-96-78-33 0-64 6-83 16l1 66c21-10 42-15 67-15s32 11 32 28c0 26-30 58-110 112v50h192v-67l-91 2c49-30 87-66 87-113l1-1z" fill="#85889C"/>
</g>
      </svg>
    `;
    orderedListButton.addEventListener("click", () => {
      wrapInList(schema.nodes.ordered_list)(view.state, view.dispatch);
      view.focus();
    });
    group.appendChild(orderedListButton);
  }

  if (group.children.length > 0) {
    container.appendChild(group);
  }
}

export function addIndentButtons(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";

  // Indent right button
  const indentRightButton = document.createElement("button");
  indentRightButton.className = "editor-general-button";
  indentRightButton.title = "Indent right";
  indentRightButton.innerHTML = `
     <svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
      <g fill="none" stroke="#85889C" stroke-width="1.2">
        <line x1="3" y1="6" x2="21" y2="6" />
        <line x1="8" y1="10" x2="21" y2="10" />
        <line x1="8" y1="14" x2="21" y2="14" />
        <line x1="3" y1="18" x2="21" y2="18" />
        <path d="M6.5 13L3.5 10M6.5 11L3.5 14" />
      </g>
    </svg>

  `;
  indentRightButton.addEventListener("click", () => {
    indentRight(view);
  });
  group.appendChild(indentRightButton);

  // Indent left button
  const indentLeftButton = document.createElement("button");
  indentLeftButton.className = "editor-general-button";
  indentLeftButton.title = "Indent left";
  indentLeftButton.innerHTML = `
   
       <svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
      <g fill="none" stroke="#85889C" stroke-width="1.2">
        <line x1="3" y1="6" x2="21" y2="6" />
        <line x1="8" y1="10" x2="21" y2="10" />
        <line x1="8" y1="14" x2="21" y2="14" />
        <line x1="3" y1="18" x2="21" y2="18" />
        <path d="M3.5 13L6.5 10M3.5 11L6.5 14" />
      </g>
    </svg>
  `;
  indentLeftButton.addEventListener("click", () => {
    indentLeft(view);
  });
  group.appendChild(indentLeftButton);

  if (group.children.length > 0) {
    container.appendChild(group);
  }
}

export function addAlignmentButtons(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";

  // Align left button
  const alignLeftButton = document.createElement("button");
  alignLeftButton.className = "editor-general-button";
  alignLeftButton.title = "Align left";
  alignLeftButton.dataset.alignment = "left";
  alignLeftButton.innerHTML = `
    <svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
      <g fill="none" stroke="#85889C" stroke-width="1.2">
        <line x1="3" y1="6" x2="21" y2="6" />
        <line x1="3" y1="10" x2="15" y2="10" />
        <line x1="3" y1="14" x2="21" y2="14" />
        <line x1="3" y1="18" x2="15" y2="18" />
      </g>
    </svg>
  `;
  alignLeftButton.addEventListener("click", () => {
    setTextAlign(view, "left");
  });
  group.appendChild(alignLeftButton);

  // Align center button
  const alignCenterButton = document.createElement("button");
  alignCenterButton.className = "editor-general-button";
  alignCenterButton.title = "Align center";
  alignCenterButton.dataset.alignment = "center";
  alignCenterButton.innerHTML = `
    <svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
      <g fill="none" stroke="#85889C" stroke-width="1.2">
        <line x1="3" y1="6" x2="21" y2="6" />
        <line x1="6" y1="10" x2="18" y2="10" />
        <line x1="3" y1="14" x2="21" y2="14" />
        <line x1="6" y1="18" x2="18" y2="18" />
      </g>
    </svg>
  `;
  alignCenterButton.addEventListener("click", () => {
    setTextAlign(view, "center");
  });
  group.appendChild(alignCenterButton);

  // Align right button
  const alignRightButton = document.createElement("button");
  alignRightButton.className = "editor-general-button";
  alignRightButton.title = "Align right";
  alignRightButton.dataset.alignment = "right";
  alignRightButton.innerHTML = `
    <svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
      <g fill="none" stroke="#85889C" stroke-width="1.2">
        <line x1="3" y1="6" x2="21" y2="6" />
        <line x1="9" y1="10" x2="21" y2="10" />
        <line x1="3" y1="14" x2="21" y2="14" />
        <line x1="9" y1="18" x2="21" y2="18" />
      </g>
    </svg>
  `;
  alignRightButton.addEventListener("click", () => {
    setTextAlign(view, "right");
  });
  group.appendChild(alignRightButton);

  if (group.children.length > 0) {
    container.appendChild(group);
  }
}

export function addBlockFormatDropdown(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";

  // Create the main dropdown button
  const dropdownContainer = document.createElement("div");
  dropdownContainer.className = "dropdown-container";

  const formatButton = document.createElement("button");
  formatButton.className = "format-dropdown-button";
  formatButton.innerHTML = `
    <span>Paragraph</span>
    <svg width="12" height="12" viewBox="0 0 24 24" focusable="false">
      <path d="M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z" fill="#85889C"></path>
    </svg>
  `;

  // Create dropdown menu
  const dropdownMenu = document.createElement("div");
  dropdownMenu.className = "dropdown-menu";
  dropdownMenu.style.display = "none";


    // Add paragraph option
    const paragraphItem = document.createElement("div");
    paragraphItem.className = "dropdown-item";
    paragraphItem.innerHTML = `<span>Paragraph</span>`;
    paragraphItem.addEventListener("click", () => {
      setBlockType(schema.nodes.paragraph)(view.state, view.dispatch);
      view.focus();
      hideDropdowns();
      formatButton.innerHTML = `
        <span>Paragraph</span>
        <svg width="12" height="12" viewBox="0 0 24 24" focusable="false">
          <path d="M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z" fill="#85889C"></path>
        </svg>
      `;
    });
    dropdownMenu.appendChild(paragraphItem);

  // Add heading options
  const headings = [
    { level: 1, text: "Heading 1" },
    { level: 2, text: "Heading 2" },
    { level: 3, text: "Heading 3" },
    { level: 4, text: "Heading 4" },
    { level: 5, text: "Heading 5" },
    { level: 6, text: "Heading 6" }
  ];

  headings.forEach(heading => {
    const headingItem = document.createElement("div");
    headingItem.className = "dropdown-item";
    headingItem.innerHTML = `<span>${heading.text}</span>`;
    headingItem.addEventListener("click", () => {
      setBlockType(schema.nodes.heading, { level: heading.level })(view.state, view.dispatch);
      view.focus();
      hideDropdowns();
      formatButton.innerHTML = `
        <span>${heading.text}</span>
        <svg width="12" height="12" viewBox="0 0 24 24" focusable="false">
          <path d="M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z" fill="#85889C"></path>
        </svg>
      `;
    });
    dropdownMenu.appendChild(headingItem);
  });



  // Function to update button text based on current block type
  const updateButtonText = () => {
    const { $from } = view.state.selection;
    const node = $from.node();
    if (node.type === schema.nodes.heading) {
      formatButton.innerHTML = `
        <span>Heading ${node.attrs.level}</span>
        <svg width="12" height="12" viewBox="0 0 24 24" focusable="false">
          <path d="M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z" fill="#85889C"></path>
        </svg>
      `;
    } else {
      formatButton.innerHTML = `
        <span>Paragraph</span>
        <svg width="12" height="12" viewBox="0 0 24 24" focusable="false">
          <path d="M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z" fill="#85889C"></path>
        </svg>
      `;
    }
  };

  // Toggle dropdown on click
  formatButton.addEventListener("click", (e) => {
    e.stopPropagation(); // Keep this to prevent event bubbling
    const isVisible = dropdownMenu.style.display === "block";
    hideDropdowns();
    if (!isVisible) {
      updateButtonText();
      dropdownMenu.style.display = "block";
      dropdownMenu.getBoundingClientRect(); // Force reflow
    }
  });

  // Update button text when selection changes
  view.dom.addEventListener("keyup", updateButtonText);
  view.dom.addEventListener("mouseup", updateButtonText);

  // Close dropdown when clicking outside
  document.addEventListener("click", (e) => {
    if (e.target instanceof Node && !dropdownContainer.contains(e.target)) {
      hideDropdowns();
    }
  });

  // Stop event propagation when clicking on the dropdown menu
  dropdownMenu.addEventListener("click", (e) => {
    e.stopPropagation();
  });

  // Add everything to the DOM
  dropdownContainer.appendChild(formatButton);
  dropdownContainer.appendChild(dropdownMenu);
  group.appendChild(dropdownContainer);
  container.appendChild(group);
}

// Add new function for text size controls
export function addTextSizeControls(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";

  // Create font size controls
  const fontSizeControls = document.createElement("div");
  fontSizeControls.className = "font-size-controls";
  
  const fontSizeInput = document.createElement("input");
  fontSizeInput.type = "text";
  fontSizeInput.className = "font-size-input";
  fontSizeInput.value = "16px"; 

  const decreaseButton = document.createElement("button");
  decreaseButton.className = "size-adjust-button";
  decreaseButton.innerHTML = `
    <svg width="16" height="16" viewBox="0 0 24 24" focusable="false">
      <path d="M19 13H5v-2h14v2z" fill="currentColor"/>
    </svg>
  `;
  
  const increaseButton = document.createElement("button");
  increaseButton.className = "size-adjust-button";
  increaseButton.innerHTML = `
    <svg width="16" height="16" viewBox="0 0 24 24" focusable="false">
      <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z" fill="currentColor"/>
    </svg>
  `;

  // --- Helper function to apply font size mark ---
  const applyFontSize = (newSize: string) => {
    const { state, dispatch } = view;
    const { $from } = state.selection; // Get the resolved position for the start of the selection

    // Determine the start and end positions of the node containing the cursor
    const nodeStart = $from.start(); // Get the start position of the node
    const nodeEnd = $from.end();   // Get the end position of the node

    // Apply the mark to the entire node range
    const tr = state.tr;
    // Remove any existing fontSize mark from the node range first
    tr.removeMark(nodeStart, nodeEnd, schema.marks.fontSize);
    // Add the new mark to the node range
    tr.addMark(nodeStart, nodeEnd, schema.marks.fontSize.create({ size: newSize }));
    
    dispatch(tr);
    view.focus();
  };

  // --- Font size adjustment handlers ---
  decreaseButton.addEventListener("click", (e) => {
    e.stopPropagation();
    const currentSize = parseInt(fontSizeInput.value) || 16;
    if (currentSize > 8) {
      const newSize = `${currentSize - 1}px`;
      fontSizeInput.value = newSize;
      applyFontSize(newSize);
    }
  });

  increaseButton.addEventListener("click", (e) => {
    e.stopPropagation();
    const currentSize = parseInt(fontSizeInput.value) || 16;
    if (currentSize < 72) {
      const newSize = `${currentSize + 1}px`;
      fontSizeInput.value = newSize;
      applyFontSize(newSize);
    }
  });

  fontSizeInput.addEventListener("change", () => {
    let size = parseInt(fontSizeInput.value);
    if (isNaN(size)) size = 16; // Default to 16 if input is invalid
    size = Math.min(72, Math.max(8, size)); // Clamp between 8 and 72
    const newSize = `${size}px`;
    fontSizeInput.value = newSize; // Update input to clamped value
    applyFontSize(newSize);
  });

  // --- Function to update display based on selection ---
  const updateFontSizeDisplay = () => {
    const { state } = view;
    const { selection } = state;
    const { $from } = selection;

    // 1. Check for explicit fontSize mark at cursor position
    const marks = $from.marks();
    const fontSizeMark = schema.marks.fontSize.isInSet(marks);

    if (fontSizeMark && fontSizeMark.attrs.size) {
      // Use the explicit mark's size if it exists
      fontSizeInput.value = fontSizeMark.attrs.size;
      return;
    }

    // 2. If no explicit mark, check if we're in a heading node
    const node = $from.parent;
    if (node.type === schema.nodes.heading) {
      // Use the default size for the heading level (based on 18px base)
      const headingLevel = node.attrs.level as 1 | 2 | 3 | 4 | 5 | 6;
      // Map heading levels to font sizes in pixels (converted from em values in CSS, base 18px)
      const headingSizes: Record<1 | 2 | 3 | 4 | 5 | 6, string> = {
        1: "36px",  // 2em    (2 * 18px)
        2: "27px",  // 1.5em  (1.5 * 18px)
        3: "21px",  // 1.17em (1.17 * 18px ≈ 21.06)
        4: "20px",  // 1.1em  (1.1 * 18px = 19.8)
        5: "19px",  // 1.05em (1.05 * 18px = 18.9)
        6: "18px"   // 1em    (1 * 18px)
      };
      fontSizeInput.value = headingSizes[headingLevel];
      return;
    }

    // 3. If no mark and not a heading, default to base size (now 18px)
    fontSizeInput.value = "18px";
  };

  // --- Initial setup and event listeners for updates ---
  updateFontSizeDisplay(); // Set initial value

  // Update display when selection changes
  view.dom.addEventListener("keyup", updateFontSizeDisplay);
  view.dom.addEventListener("mouseup", updateFontSizeDisplay);
  // Consider adding 'focus' if needed, though mouseup/keyup cover most cases

  // Append controls to the DOM
  fontSizeControls.appendChild(decreaseButton);
  fontSizeControls.appendChild(fontSizeInput);
  fontSizeControls.appendChild(increaseButton);
  group.appendChild(fontSizeControls);
  container.appendChild(group);
}

// Function to add underline and strikethrough buttons
export function addSecondaryFormattingItems(container: HTMLElement, schema: Schema, view: EditorView) {
	const group = document.createElement("div");
	group.className = "editor-menu-group";

	// Underline button
	if (schema.marks.underline) {
		const underlineButton = document.createElement("button");
		underlineButton.className = "editor-general-button menu-underline";
		underlineButton.title = "Underline";
		underlineButton.dataset.markType = "underline";
		underlineButton.innerHTML = `
     <svg width="24" height="24" focusable="false"><path d="M16 5c.6 0 1 .4 1 1v5.5a4 4 0 0 1-.4 1.8l-1 1.4a5.3 5.3 0 0 1-5.5 1 5 5 0 0 1-1.6-1c-.5-.4-.8-.9-1.1-1.4a4 4 0 0 1-.4-1.8V6c0-.6.4-1 1-1s1 .4 1 1v5.5c0 .3 0 .6.2 1l.6.7a3.3 3.3 0 0 0 2.2.8 3.4 3.4 0 0 0 2.2-.8c.3-.2.4-.5.6-.8l.2-.9V6c0-.6.4-1 1-1ZM8 17h8c.6 0 1 .4 1 1s-.4 1-1 1H8a1 1 0 0 1 0-2Z" fill-rule="evenodd" fill="#85889C"></path></svg>
    `;
		underlineButton.addEventListener("click", () => {
			toggleMark(schema.marks.underline)(view.state, view.dispatch);
			view.focus();
		});
		group.appendChild(underlineButton);
	}

	// Strikethrough button
	if (schema.marks.strikethrough) {
		const strikethroughButton = document.createElement("button");
		strikethroughButton.className = "editor-general-button menu-strikethrough";
		strikethroughButton.title = "Strikethrough";
		strikethroughButton.dataset.markType = "strikethrough";
		strikethroughButton.innerHTML = `
      <svg width="24" height="24" focusable="false"><g fill-rule="evenodd"><path d="M15.6 8.5c-.5-.7-1-1.1-1.3-1.3-.6-.4-1.3-.6-2-.6-2.7 0-2.8 1.7-2.8 2.1 0 1.6 1.8 2 3.2 2.3 4.4.9 4.6 2.8 4.6 3.9 0 1.4-.7 4.1-5 4.1A6.2 6.2 0 0 1 7 16.4l1.5-1.1c.4.6 1.6 2 3.7 2 1.6 0 2.5-.4 3-1.2.4-.8.3-2-.8-2.6-.7-.4-1.6-.7-2.9-1-1-.2-3.9-.8-3.9-3.6C7.6 6 10.3 5 12.4 5c2.9 0 4.2 1.6 4.7 2.4l-1.5 1.1Z" fill="#85889C"></path><path d="M5 11h14a1 1 0 0 1 0 2H5a1 1 0 0 1 0-2Z" fill-rule="nonzero" fill="#85889C"></path></g></svg>
    `;
		strikethroughButton.addEventListener("click", () => {
			toggleMark(schema.marks.strikethrough)(view.state, view.dispatch);
			view.focus();
		});
		group.appendChild(strikethroughButton);
	}

  // Link button
  if (schema.marks.link) {
    const linkButton = document.createElement("button");
    linkButton.className = "editor-general-button menu-link";
    linkButton.title = "Add link";
    linkButton.innerHTML = `
      <svg width="24" height="24" viewBox="0 0 24 24" focusable="false">
        <path d="M10.59 13.41c.44.44 1.16.44 1.6 0l3.82-3.82a4.003 4.003 0 0 0-5.66-5.66l-1.41 1.41a1 1 0 0 0 1.41 1.41l1.06-1.06c1.17-.88 2.77-.62 3.64.24.88.88.62 2.47-.24 3.64L13.4 12a1 1 0 0 0 0 1.41l.01.01zm2.82-1.41a1 1 0 0 0-1.41 0L10.6 13.4c-1.17.88-2.77.62-3.64-.24-.88-.88-.62-2.47.24-3.64l1.06-1.06a1 1 0 0 0-1.41-1.41L5.4 8.46a4.003 4.003 0 0 0 5.66 5.66l3.82-3.82a1 1 0 0 0-1.41-1.41l-.01-.01z" fill="#85889C"/>
      </svg>
    `;

    linkButton.addEventListener("click", (e) => {
      e.preventDefault();
      console.log("Link button clicked"); // Log: Button click
      const { state, dispatch } = view;
      const { selection } = state;
      const { $from } = selection;

      // Check if link mark is active
      const isLinkActive = state.doc.rangeHasMark(selection.$anchor.pos, selection.$head.pos, schema.marks.link);
      console.log("Is link active?", isLinkActive); // Log: Link active status

      if (isLinkActive) {
        // If link is active, remove it
        console.log("Removing link mark"); // Log: Removing link
        toggleMark(schema.marks.link)(state, dispatch);
        view.focus();
      } else {
        // If link is not active, emit an event to request the modal
        console.log("Requesting link modal"); // Log: Requesting modal
        const existingHref = schema.marks.link.isInSet($from.marks())?.attrs.href || "";
        
        // Emit event with selection details
        void emit('request-link-modal', { 
          from: selection.from, 
          to: selection.to, 
          existingHref 
        });
        
        // Focus remains in the editor for now
        view.focus(); 
      }
    });
    group.appendChild(linkButton);
  }
  

  // Function to update button active state
  const updateButtonActiveState = () => {
    const { state } = view;
    const { selection } = state;
    const { $from, empty } = selection;

    // Update Underline Button
    if (schema.marks.underline) {
      const underlineButton = group.querySelector(".menu-underline") as HTMLButtonElement;
      if (underlineButton) {
        underlineButton.classList.toggle("is-active", !!schema.marks.underline.isInSet($from.marks()));
      }
    }

    // Update Strikethrough Button
    if (schema.marks.strikethrough) {
      const strikethroughButton = group.querySelector(".menu-strikethrough") as HTMLButtonElement;
      if (strikethroughButton) {
        strikethroughButton.classList.toggle("is-active", !!schema.marks.strikethrough.isInSet($from.marks()));
      }
    }

    // Update Link Button
    if (schema.marks.link) {
      const linkButton = group.querySelector(".menu-link") as HTMLButtonElement;
      if (linkButton) {
        linkButton.disabled = empty;
        linkButton.style.opacity = empty ? "0.5" : "1";
        // Check if link mark is active at cursor/selection
        const {$anchor, $head} = selection;
        const isLinkActive = state.doc.rangeHasMark($anchor.pos, $head.pos, schema.marks.link);
        linkButton.classList.toggle("is-active", isLinkActive);
      }
    }
  };

  // Initial state update
  updateButtonActiveState();

  // Add event listeners to update state
  view.dom.addEventListener("keyup", updateButtonActiveState);
  view.dom.addEventListener("mouseup", updateButtonActiveState);
  const originalDispatch = view.dispatch;
  view.dispatch = (tr) => {
    originalDispatch(tr);
    if (tr.docChanged || tr.selectionSet) {
      updateButtonActiveState();
    }
  };

  if (group.children.length > 0) {
    container.appendChild(group);
  }
}

// Function to add blockquote and code block buttons
export function addBlockStyleItems(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";

  // Blockquote button
  if (schema.nodes.blockquote) {
    const blockquoteButton = document.createElement("button");
    blockquoteButton.className = "editor-general-button";
    blockquoteButton.title = "Blockquote";
    blockquoteButton.dataset.nodeType = "blockquote";
    blockquoteButton.innerHTML = `
     <svg width="24" height="24" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" fill="currentColor">
  <g transform="translate(170, 60)">
    <path d="M0 448v256h256v-256h-128c0 0 0-128 128-128v-128c0 0-256 0-256 256zM640 320v-128c0 0-256 0-256 256v256h256v-256h-128c0 0 0-128 128-128z" fill="#85889C"/>
  </g>
</svg>

    `;
    blockquoteButton.addEventListener("click", () => {
      wrapIn(schema.nodes.blockquote)(view.state, view.dispatch);
      view.focus();
    });
    group.appendChild(blockquoteButton);
  }

  // Code block button
  if (schema.nodes.code_block) {
    const codeBlockButton = document.createElement("button");
    codeBlockButton.className = "editor-general-button";
    codeBlockButton.title = "Code block";
    codeBlockButton.dataset.nodeType = "code_block";
    codeBlockButton.innerHTML = `
      <svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
        <path d="M9.4 16.6L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4zm5.2 0l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4z" fill="#85889C"/>
      </svg>
    `;
    codeBlockButton.addEventListener("click", () => {
      setBlockType(schema.nodes.code_block)(view.state, view.dispatch);
      view.focus();
    });
    group.appendChild(codeBlockButton);
  }

  if (group.children.length > 0) {
    container.appendChild(group);
  }
}

// Function to add text color picker
export function addTextColorPicker(container: HTMLElement, schema: Schema, view: EditorView) {
  if (!schema.marks.textColor) return; // Don't add if mark is not defined

  const group = document.createElement("div");
  group.className = "editor-menu-group";

  const dropdownContainer = document.createElement("div");
  dropdownContainer.className = "dropdown-container";

  // Main color picker button
  const colorButton = document.createElement("button");
  colorButton.className = "editor-general-button text-color-button";
  colorButton.title = "Text color";
  colorButton.innerHTML = `
    <svg width="24" height="24" viewBox="0 0 24 24" focusable="false">
      <path d="M8.7 16h-.8a.5.5 0 0 1-.5-.6l2.7-9c.1-.3.3-.4.5-.4h2.8c.2 0 .4.1.5.4l2.7 9a.5.5 0 0 1-.5.6h-.8a.5.5 0 0 1-.4-.4l-.7-2.2c0-.3-.3-.4-.5-.4h-3.4c-.2 0-.4.1-.5.4l-.7 2.2c0 .3-.2.4-.4.4Zm2.6-7.6-.6 2a.5.5 0 0 0 .5.6h1.6a.5.5 0 0 0 .5-.6l-.6-2c0-.3-.3-.4-.5-.4h-.4c-.2 0-.4.1-.5.4Z"  fill="#85889C"></path>
      <rect x="4" y="19" width="16" height="1.5" rx="1" ry="1" class="color-indicator" fill="#000" />
    </svg>
  `;
  const colorIndicator = colorButton.querySelector('.color-indicator') as SVGRectElement | null;

  // Dropdown menu for colors
  const dropdownMenu = document.createElement("div");
  dropdownMenu.className = "dropdown-menu color-picker-dropdown";
  dropdownMenu.style.display = "none";

  const colors = [
    "#000000", "#FF0000", "#FFA500", // Black, Red, Orange
    "#FFFF00", "#008000", "#0000FF", // Yellow, Green, Blue
    "#4B0082", "#EE82EE", "#FFFFFF"  // Indigo, Violet, White (adjust white if needed for visibility)
    // Add a 'remove color' option?
  ];

  // Create color swatches
  colors.forEach(color => {
    const swatch = document.createElement("div");
    swatch.className = "color-swatch";
    swatch.style.backgroundColor = color;
    if (color === "#FFFFFF") { // Add border for white swatch
        swatch.style.border = "1px solid #ccc";
    }
    swatch.dataset.color = color;
    swatch.addEventListener("click", (e) => {
      e.stopPropagation();
      const { state, dispatch } = view;
      const { from, to, empty } = state.selection;

      if (empty) {
        // Optionally apply to the current word or do nothing
        // For now, we only apply if text is selected
        console.warn("No text selected to apply color.");
        hideDropdowns(); // Hide dropdown even if no action taken
        return;
      }

      const tr = state.tr;
      // Remove existing textColor mark from the selection
      tr.removeMark(from, to, schema.marks.textColor);
      // Add the new textColor mark
      tr.addMark(from, to, schema.marks.textColor.create({ color }));
      dispatch(tr);

      // Update button indicator color
      if (colorIndicator) {
        colorIndicator.setAttribute('fill', color);
      }

      hideDropdowns();
      view.focus();
    });
    dropdownMenu.appendChild(swatch);
  });

  // Add "Remove Color" button
  const removeColorButton = document.createElement("button");
  removeColorButton.className = "dropdown-item remove-color-button";
  removeColorButton.textContent = "Remove Color";
  removeColorButton.addEventListener("click", (e) => {
      e.stopPropagation();
      const { state, dispatch } = view;
      const { from, to, empty } = state.selection;

      if (empty) {
        console.warn("No text selected to remove color.");
        hideDropdowns();
        return;
      }

      const tr = state.tr;
      // Remove existing textColor mark from the selection
      tr.removeMark(from, to, schema.marks.textColor);
      dispatch(tr);

       // Reset button indicator color to default (e.g., black)
      if (colorIndicator) {
        colorIndicator.setAttribute('fill', '#000000'); // Or a different default
      }

      hideDropdowns();
      view.focus();
  });
  dropdownMenu.appendChild(removeColorButton);


  // Function to update button state based on selection
  const updateButtonState = () => {
    const { state } = view;
    const { selection } = state;
    const { empty, $from } = selection;

    // Disable button if selection is empty
    colorButton.disabled = empty;
    colorButton.style.opacity = empty ? '0.5' : '1';

    if (empty) {
       if (colorIndicator) colorIndicator.setAttribute('fill', '#fff'); // Reset to default if empty
       return;
    }

    // Update indicator color based on the mark at the start of selection
    const marks = $from.marksAcross(selection.$to); // Get marks spanning the selection
    let commonColor: string | null = null;
    let first = true;

    if (marks) {
       for (const mark of marks) {
           if (mark.type === schema.marks.textColor) {
               const markColor = mark.attrs.color;
               if (first) {
                   commonColor = markColor;
                   first = false;
               } else if (commonColor !== markColor) {
                   commonColor = null; // Multiple colors in selection
                   break;
               }
           }
       }
    }


    // If no textColor mark found across selection, check at cursor pos ($from)
    if (commonColor === null && !first) { // 'first' is false if we entered the loop but found different colors
       // Indicate multiple colors (optional, e.g., a gradient or default black)
       if (colorIndicator) colorIndicator.setAttribute('fill', '#fff');
    } else {
         // Use the common color or the color at $from if no marks span the whole selection or selection is a cursor
        const markAtCursor = schema.marks.textColor.isInSet($from.marks());
        const finalColor = commonColor ?? (markAtCursor ? markAtCursor.attrs.color : null);

        if (colorIndicator) {
            colorIndicator.setAttribute('fill', finalColor || '#fff'); // Use found color or default to black
        }
    }
  };

  // Toggle dropdown on click
  colorButton.addEventListener("click", (e) => {
    e.stopPropagation();
    if (view.state.selection.empty) return; // Don't open if nothing selected

    const isVisible = dropdownMenu.style.display === "block";
    hideDropdowns(); // Hide other dropdowns first
    if (!isVisible) {
      updateButtonState(); // Ensure button state is current before showing
      // Set display to grid to enable grid layout defined in CSS
      dropdownMenu.style.display = "grid"; 
      dropdownMenu.style.position = "absolute"; // Ensure it positions correctly
      dropdownMenu.style.top = `${colorButton.offsetTop + colorButton.offsetHeight}px`;
      dropdownMenu.style.left = `${colorButton.offsetLeft}px`;
      dropdownMenu.getBoundingClientRect(); // Force reflow
    }
  });

  // Update button state when selection or marks change
  view.dom.addEventListener("keyup", updateButtonState);
  view.dom.addEventListener("mouseup", updateButtonState);
  // Listen for transactions as marks can change programmatically
  const originalDispatch = view.dispatch;
  view.dispatch = (tr) => {
      originalDispatch(tr);
      if (tr.docChanged || tr.selectionSet) {
          updateButtonState();
      }
  };

  // Close dropdown when clicking outside
  document.addEventListener("click", (e) => {
    if (e.target instanceof Node && !dropdownContainer.contains(e.target)) {
      hideDropdowns();
    }
  });

  // Stop event propagation when clicking on the dropdown menu
  dropdownMenu.addEventListener("click", (e) => {
    e.stopPropagation();
  });

  // Add elements to DOM
  dropdownContainer.appendChild(colorButton);
  dropdownContainer.appendChild(dropdownMenu);
  group.appendChild(dropdownContainer);
  container.appendChild(group);

  // Initial button state
  updateButtonState();
}