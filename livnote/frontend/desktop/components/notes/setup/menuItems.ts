import { EditorView } from "prosemirror-view";
import { Schema, Node as ProsemirrorNode } from "prosemirror-model";
import {
  wrapIn,
  setBlockType,
  chainCommands,
  toggleMark,
  exitCode,
  joinUp,
  joinDown,
  lift,
  selectParentNode,
} from "prosemirror-commands";
import { wrapInList, liftListItem } from "prosemirror-schema-list";
import { undo, redo } from "prosemirror-history";
import { indentRight, indentLeft } from "./indentUtils";
import { setTextAlign } from "./alignmentUtils";
import { createHeadingSubmenu, hideDropdowns } from "./dropdownUtils";
import { emit } from "@tauri-apps/api/event";
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { readFile } from '@tauri-apps/plugin-fs';

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

  // --- Function to update button state ---
  const updateIndentButtonsState = () => {
    const { state } = view;
    const { $from } = state.selection;
    const node = $from.parent;

    // Check 1: Node has indent attribute > 0
    const hasIndent = node.attrs.indent && node.attrs.indent > 0;

    // Check 2: liftListItem command is applicable
    // We pass undefined for dispatch because we only want to check applicability
    const canLiftList = schema.nodes.list_item && liftListItem(schema.nodes.list_item)(state, undefined);

    const canIndentLeft = hasIndent || canLiftList;

    // Update button appearance and state
    indentLeftButton.disabled = !canIndentLeft;
    indentLeftButton.style.opacity = canIndentLeft ? '1' : '0.5';
  };

  // --- Initial setup and event listeners ---
  updateIndentButtonsState(); // Set initial state

  // Update state on selection change
  view.dom.addEventListener("keyup", updateIndentButtonsState);
  view.dom.addEventListener("mouseup", updateIndentButtonsState);

  // Wrap view.dispatch to update state after transactions
  const originalDispatch = view.dispatch;
  view.dispatch = (tr) => {
    originalDispatch(tr); // Apply the transaction first
    // Update the display if the document changed or the selection moved
    if (tr.docChanged || tr.selectionSet) {
      updateIndentButtonsState();
    }
  };

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
  paragraphItem.innerHTML = `<span style="font-size: 1em;">Paragraph</span>`;
  paragraphItem.addEventListener("click", () => {
    const { state, dispatch } = view;
    // Apply the block type change first
    setBlockType(schema.nodes.paragraph)(state, dispatch);

    // After the block type changes, get the new state and remove the fontSize mark
    const newState = view.state;
    const { $from } = newState.selection;
    const nodeStart = $from.start();
    const nodeEnd = $from.end();

    const tr = newState.tr;
    if (schema.marks.fontSize) { // Check if fontSize mark exists
      tr.removeMark(nodeStart, nodeEnd, schema.marks.fontSize);
    }
    // Only dispatch if the mark removal actually changed something
    if (tr.docChanged) {
      view.dispatch(tr);
    }

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

    // Style the dropdown items to match the actual heading styles
    let headingStyle = '';
    switch (heading.level) {
      case 1:
        headingStyle = 'font-size: 2em; font-weight: bold;';
        break;
      case 2:
        headingStyle = 'font-size: 1.5em; font-weight: bold;';
        break;
      case 3:
        headingStyle = 'font-size: 1.17em; font-weight: bold;';
        break;
      case 4:
        headingStyle = 'font-size: 1.1em; font-weight: bold;';
        break;
      case 5:
        headingStyle = 'font-size: 1.05em; font-weight: bold;';
        break;
      case 6:
        headingStyle = 'font-size: 1em; font-weight: bold;';
        break;
    }

    headingItem.innerHTML = `<span style="${headingStyle}">${heading.text}</span>`;

    headingItem.addEventListener("click", () => {
      const { state, dispatch } = view;
      // Apply the block type change first
      setBlockType(schema.nodes.heading, { level: heading.level })(state, dispatch);

      // After the block type changes, get the new state and remove the fontSize mark
      const newState = view.state;
      const { $from } = newState.selection;
      const nodeStart = $from.start();
      const nodeEnd = $from.end();

      const tr = newState.tr;
      if (schema.marks.fontSize) { // Check if fontSize mark exists
        tr.removeMark(nodeStart, nodeEnd, schema.marks.fontSize);
      }
      // Only dispatch if the mark removal actually changed something
      if (tr.docChanged) {
        view.dispatch(tr);
      }

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
  fontSizeInput.value = "18px"; // Default changed to 18px to match update logic

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
    const { $from } = state.selection;

    // Determine the start and end positions based on selection
    const { from, to, empty } = state.selection;
    // If selection is empty, apply to the whole node content
    const [markStart, markEnd] = empty ? [$from.start(), $from.end()] : [from, to];

    // Apply the mark to the determined range
    const tr = state.tr;
    if (schema.marks.fontSize) {
      // Remove any existing fontSize mark from the range first
      tr.removeMark(markStart, markEnd, schema.marks.fontSize);
      // Add the new mark to the range
      tr.addMark(markStart, markEnd, schema.marks.fontSize.create({ size: newSize }));
      dispatch(tr);
    }
    view.focus();
  };


  // --- Font size adjustment handlers ---
  decreaseButton.addEventListener("click", (e) => {
    e.stopPropagation();
    const currentSize = parseInt(fontSizeInput.value) || 18; // Use 18 as base default
    if (currentSize > 8) {
      const newSize = `${currentSize - 1}px`;
      fontSizeInput.value = newSize;
      applyFontSize(newSize);
    }
  });

  increaseButton.addEventListener("click", (e) => {
    e.stopPropagation();
    const currentSize = parseInt(fontSizeInput.value) || 18; // Use 18 as base default
    if (currentSize < 72) {
      const newSize = `${currentSize + 1}px`;
      fontSizeInput.value = newSize;
      applyFontSize(newSize);
    }
  });

  fontSizeInput.addEventListener("change", () => {
    let size = parseInt(fontSizeInput.value);
    if (isNaN(size)) size = 18; // Default to 18 if input is invalid
    size = Math.min(72, Math.max(8, size)); // Clamp between 8 and 72
    const newSize = `${size}px`;
    fontSizeInput.value = newSize; // Update input to clamped value
    applyFontSize(newSize);
  });

  // --- Function to update display based on selection ---
  const updateFontSizeDisplay = () => {
    // Check if the input element still exists in the DOM
    if (!fontSizeInput || !fontSizeInput.isConnected) {
      return; // Avoid errors if the menu is removed
    }

    const { state } = view;
    const { selection } = state;
    const { $from } = selection;

    // 1. Check for explicit fontSize mark at cursor position
    const marks = $from.marks();
    if (schema.marks.fontSize) {
      const fontSizeMark = schema.marks.fontSize.isInSet(marks);
      if (fontSizeMark && fontSizeMark.attrs.size) {
        // Use the explicit mark's size if it exists
        fontSizeInput.value = fontSizeMark.attrs.size;
        return;
      }
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

  // Remove the previous keyup/mouseup listeners
  // view.dom.removeEventListener("keyup", updateFontSizeDisplay);
  // view.dom.removeEventListener("mouseup", updateFontSizeDisplay);

  // Wrap the view's dispatch function to update on any relevant transaction
  const originalDispatch = view.dispatch;
  view.dispatch = (tr) => {
    originalDispatch(tr); // Apply the transaction first
    // Update the display if the document changed or the selection moved
    if (tr.docChanged || tr.selectionSet) {
      updateFontSizeDisplay();
    }
  };


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

  // --- Image Upload Button ---
  if (schema.nodes.image) {
    const imageButton = document.createElement("button");
    imageButton.className = "editor-general-button menu-image";
    imageButton.title = "Insert image";
    imageButton.innerHTML = `
      <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
        <path d="M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2zM8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z" fill="#85889C"/>
      </svg>
    `;
    imageButton.addEventListener("click", async () => {
      try {
        const selectedPath = await openDialog({
          multiple: false,
          filters: [{
            name: 'Images',
            extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg']
          }]
        });

        // Handle both string and string[] return types
        const path = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;

        if (typeof path === 'string') {
          const binaryData = await readFile(path);

          // Function to convert Blob to Base64 Data URL using FileReader wrapped in a Promise
          const blobToBase64 = (blob: Blob): Promise<string> => {
            return new Promise((resolve, reject) => {
              const reader = new FileReader();
              reader.onloadend = () => resolve(reader.result as string);
              reader.onerror = (error) => reject(error);
              reader.readAsDataURL(blob);
            });
          };

          // Create Blob and convert to Data URL
          const blob = new Blob([binaryData]);
          const dataUrl = await blobToBase64(blob);

          // Extract filename from path
          const filename = path.split('/').pop() || path.split('\\').pop() || 'uploaded-image';

          // Detect MIME type from data URL or filename
          const mimeMatch = dataUrl.match(/^data:([^;]+);/);
          let mimeType = mimeMatch ? mimeMatch[1] : 'image/png';

          // If MIME type is generic, try to detect from filename
          if (mimeType === 'application/octet-stream' || !mimeType.startsWith('image/')) {
            const ext = filename.toLowerCase().split('.').pop();
            const mimeMap: { [key: string]: string } = {
              'jpg': 'image/jpeg',
              'jpeg': 'image/jpeg',
              'png': 'image/png',
              'gif': 'image/gif',
              'bmp': 'image/bmp',
              'webp': 'image/webp',
              'svg': 'image/svg+xml'
            };
            mimeType = mimeMap[ext || ''] || 'image/png';
          }

          // Access the image storage service from the editor
          // We need to pass it through or access it via a global/shared method
          const editorContainer = view.dom.closest('.editor-container');
          if (!editorContainer) {
            console.error('Editor container not found');
            return;
          }

          // Emit an event to request image storage
          const storeImageEvent = new CustomEvent('store-image-request', {
            detail: {
              dataUrl,
              mimeType,
              filename,
              callback: (imageId: string, metadata: any) => {
                // Create the image node with YJS reference
                const imageNode = schema.nodes.image.create({
                  src: `yjs-image:${imageId}`,
                  alt: filename,
                  title: filename,
                  width: metadata?.width,
                  height: metadata?.height
                });

                // Insert the image node at the current selection
                const { state, dispatch } = view;
                const transaction = state.tr.replaceSelectionWith(imageNode);
                dispatch(transaction);
                view.focus();
              }
            }
          });

          document.dispatchEvent(storeImageEvent);
        }
      } catch (error) {
        console.error("Error selecting or processing image:", error);
        // Optionally: Show an error message to the user
      }
    });
    group.appendChild(imageButton);
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
        // Add opacity effect when no text is selected
        underlineButton.style.opacity = empty ? '0.5' : '1';
        underlineButton.disabled = empty;
      }
    }

    // Update Strikethrough Button
    if (schema.marks.strikethrough) {
      const strikethroughButton = group.querySelector(".menu-strikethrough") as HTMLButtonElement;
      if (strikethroughButton) {
        strikethroughButton.classList.toggle("is-active", !!schema.marks.strikethrough.isInSet($from.marks()));
        // Add opacity effect when no text is selected
        strikethroughButton.style.opacity = empty ? '0.5' : '1';
        strikethroughButton.disabled = empty;
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
      chainCommands(lift, wrapIn(schema.nodes.blockquote))(view.state, view.dispatch);
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
      const { $from } = view.state.selection;
      const command = $from.parent.type === schema.nodes.code_block
        ? setBlockType(schema.nodes.paragraph)
        : setBlockType(schema.nodes.code_block);
      command(view.state, view.dispatch);
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
      dropdownMenu.style.display = "block";
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

export function addFontFamilyDropdown(container: HTMLElement, schema: Schema, view: EditorView) {
  if (!schema.marks.fontFamily) return; // Don't add if mark is not defined

  const group = document.createElement("div");
  group.className = "editor-menu-group";

  const dropdownContainer = document.createElement("div");
  dropdownContainer.className = "dropdown-container";

  // Main font family button
  const fontButton = document.createElement("button");
  fontButton.className = "font-family-dropdown-button";
  fontButton.title = "Font family";
  fontButton.innerHTML = `
    <span class="font-name">Arial</span>
    <svg width="12" height="12" viewBox="0 0 24 24" focusable="false">
      <path d="M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z" fill="#85889C"></path>
    </svg>
  `;

  // Dropdown menu for font families
  const dropdownMenu = document.createElement("div");
  dropdownMenu.className = "dropdown-menu font-family-dropdown";
  dropdownMenu.style.display = "none";

  // Common system fonts list - simplified and cleaned up
  const fontFamilies = [
    { name: "Arial", value: "Arial, sans-serif" },
    { name: "Arial Black", value: "'Arial Black', sans-serif" },
    { name: "Calibri", value: "Calibri, sans-serif" },
    { name: "Cambria", value: "Cambria, serif" },
    { name: "Candara", value: "Candara, sans-serif" },
    { name: "Comic Sans MS", value: "'Comic Sans MS', cursive" },
    { name: "Consolas", value: "Consolas, monospace" },
    { name: "Constantia", value: "Constantia, serif" },
    { name: "Corbel", value: "Corbel, sans-serif" },
    { name: "Courier New", value: "'Courier New', monospace" },
    { name: "Georgia", value: "Georgia, serif" },
    { name: "Helvetica", value: "Helvetica, sans-serif" },
    { name: "Helvetica Neue", value: "'Helvetica Neue', sans-serif" },
    { name: "Impact", value: "Impact, sans-serif" },
    { name: "Lucida Grande", value: "'Lucida Grande', sans-serif" },
    { name: "Segoe UI", value: "'Segoe UI', sans-serif" },
    { name: "Tahoma", value: "Tahoma, sans-serif" },
    { name: "Times New Roman", value: "'Times New Roman', serif" },
    { name: "Trebuchet MS", value: "'Trebuchet MS', sans-serif" },
    { name: "Verdana", value: "Verdana, sans-serif" }
  ];

  // Create font family options
  fontFamilies.forEach(font => {
    const fontItem = document.createElement("div");
    fontItem.className = "dropdown-item font-family-item";
    fontItem.style.fontFamily = font.value;
    fontItem.textContent = font.name;
    fontItem.dataset.fontFamily = font.value;
    fontItem.dataset.fontName = font.name;

    fontItem.addEventListener("click", (e) => {
      e.stopPropagation();
      const { state, dispatch } = view;
      const { from, to, empty } = state.selection;

      const tr = state.tr;

      if (empty) {
        // No text selected - apply font family to cursor position for future typing
        tr.addStoredMark(schema.marks.fontFamily.create({ family: font.value }));
      } else {
        // Text is selected - apply font family to selected text
        tr.removeMark(from, to, schema.marks.fontFamily);
        tr.addMark(from, to, schema.marks.fontFamily.create({ family: font.value }));
      }

      dispatch(tr);

      // Update button text
      const fontNameSpan = fontButton.querySelector('.font-name');
      if (fontNameSpan) {
        fontNameSpan.textContent = font.name;
      }

      hideDropdowns();
      view.focus();
    });
    dropdownMenu.appendChild(fontItem);
  });



  // Add "Remove Font" button
  const removeFontButton = document.createElement("button");
  removeFontButton.className = "dropdown-item remove-font-button";
  removeFontButton.textContent = "Remove Font";
  removeFontButton.addEventListener("click", (e) => {
    e.stopPropagation();
    const { state, dispatch } = view;
    const { from, to, empty } = state.selection;

    const tr = state.tr;

    if (empty) {
      // No text selected - remove font family from cursor position for future typing
      tr.removeStoredMark(schema.marks.fontFamily);
    } else {
      // Text is selected - remove font family from selected text
      tr.removeMark(from, to, schema.marks.fontFamily);
    }

    dispatch(tr);

    // Reset button text to default
    const fontNameSpan = fontButton.querySelector('.font-name');
    if (fontNameSpan) {
      fontNameSpan.textContent = "Arial";
    }

    hideDropdowns();
    view.focus();
  });
  dropdownMenu.appendChild(removeFontButton);

  // Function to update button state based on selection
  const updateButtonState = () => {
    const { state } = view;
    const { selection } = state;
    const { empty, $from } = selection;

    // Enable button regardless of selection state
    fontButton.disabled = false;
    fontButton.style.opacity = '1';

    if (empty) {
      // No text selected - check for stored marks (for future typing)
      const storedMarks = state.storedMarks || $from.marks();
      const fontFamilyMark = schema.marks.fontFamily.isInSet(storedMarks);

      const fontNameSpan = fontButton.querySelector('.font-name');
      if (fontNameSpan) {
        if (fontFamilyMark) {
          // Find the display name for this font family
          const fontFamilyValue = fontFamilyMark.attrs.family;
          
          // Try exact match first
          let fontMatch = fontFamilies.find(f => f.value === fontFamilyValue);
          
          // If no exact match, try to match the font name without fallbacks
          if (!fontMatch) {
            const fontName = fontFamilyValue.replace(/['"]/g, '').split(',')[0].trim();
            fontMatch = fontFamilies.find(f => f.name === fontName);
          }
          
          // If still no match, try partial matching
          if (!fontMatch) {
            const fontName = fontFamilyValue.replace(/['"]/g, '').split(',')[0].trim();
            fontMatch = fontFamilies.find(f => 
              f.name.toLowerCase() === fontName.toLowerCase() ||
              f.value.toLowerCase().includes(fontName.toLowerCase())
            );
          }
          
          fontNameSpan.textContent = fontMatch ? fontMatch.name : fontFamilyValue.split(',')[0].replace(/['"]/g, '').trim();
        } else {
          fontNameSpan.textContent = "Arial";
        }
      }
      return;
    }

    // Check for fontFamily mark in selection
    const marks = $from.marksAcross(selection.$to);
    let commonFont: string | null = null;
    let first = true;

    if (marks) {
      for (const mark of marks) {
        if (mark.type === schema.marks.fontFamily) {
          const markFont = mark.attrs.family;
          if (first) {
            commonFont = markFont;
            first = false;
          } else if (commonFont !== markFont) {
            commonFont = null; // Multiple fonts in selection
            break;
          }
        }
      }
    }

    // If no fontFamily mark found across selection, check at cursor position
    if (commonFont === null && !first) {
      // Multiple fonts in selection
      const fontNameSpan = fontButton.querySelector('.font-name');
      if (fontNameSpan) fontNameSpan.textContent = "Mixed";
    } else {
      // Use the common font or check at cursor position
      const markAtCursor = schema.marks.fontFamily.isInSet($from.marks());
      const finalFont = commonFont ?? (markAtCursor ? markAtCursor.attrs.family : null);

      const fontNameSpan = fontButton.querySelector('.font-name');
      if (fontNameSpan) {
        if (finalFont) {
          // Find the display name for this font family
          const fontFamilyValue = finalFont;
          
          // Try exact match first
          let fontMatch = fontFamilies.find(f => f.value === fontFamilyValue);
          
          // If no exact match, try to match the font name without fallbacks
          if (!fontMatch) {
            const fontName = fontFamilyValue.replace(/['"]/g, '').split(',')[0].trim();
            fontMatch = fontFamilies.find(f => f.name === fontName);
          }
          
          // If still no match, try partial matching
          if (!fontMatch) {
            const fontName = fontFamilyValue.replace(/['"]/g, '').split(',')[0].trim();
            fontMatch = fontFamilies.find(f => 
              f.name.toLowerCase() === fontName.toLowerCase() ||
              f.value.toLowerCase().includes(fontName.toLowerCase())
            );
          }
          
          fontNameSpan.textContent = fontMatch ? fontMatch.name : fontFamilyValue.split(',')[0].replace(/['"]/g, '').trim();
        } else {
          fontNameSpan.textContent = "Arial";
        }
      }
    }
  };

  // Toggle dropdown on click
  fontButton.addEventListener("click", (e) => {
    e.stopPropagation();
    // Remove the check for empty selection - allow dropdown to open always

    const isVisible = dropdownMenu.style.display === "block";
    hideDropdowns(); // Hide other dropdowns first
    if (!isVisible) {
      updateButtonState(); // Ensure button state is current before showing
      dropdownMenu.style.display = "block";
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
  dropdownContainer.appendChild(fontButton);
  dropdownContainer.appendChild(dropdownMenu);
  group.appendChild(dropdownContainer);
  container.appendChild(group);

  // Initial button state
  updateButtonState();
}
