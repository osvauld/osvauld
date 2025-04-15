import { EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import { toggleMark, setBlockType, wrapIn } from "prosemirror-commands";
import { wrapInList } from "prosemirror-schema-list";
import { undo, redo } from "prosemirror-history";
import { indentRight, indentLeft } from "./indentUtils";
import { setTextAlign } from "./alignmentUtils";
import { createHeadingSubmenu, hideDropdowns } from "./dropdownUtils";

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

  // // Paragraph
  // if (schema.nodes.paragraph) {
  //   const paragraphButton = document.createElement("button");
  //   paragraphButton.className = "editor-general-button menu-paragraph";
  //   paragraphButton.title = "Paragraph";
  //   paragraphButton.dataset.nodeType = "paragraph";
  //   paragraphButton.innerHTML = `
  //     <svg width="24px" height="24px" viewBox="0 0 128 128" xmlns="http://www.w3.org/2000/svg">
  //       <text x="50%" y="50%" font-family="Arial" font-size="100" font-weight="light" fill="#85889C" dominant-baseline="central" text-anchor="middle">P</text>
  //     </svg>
  //   `;
  //   paragraphButton.addEventListener("click", () => {
  //     setBlockType(schema.nodes.paragraph)(view.state, view.dispatch);
  //     view.focus();
  //   });
  //   group.appendChild(paragraphButton);
  // }

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
        <path d="M0 512h128v-128h-128v128zM0 256h128v-128h-128v128zM0 768h128v-128h-128v128zM256 512h512v-128h-512v128zM256 256h512v-128h-512v128zM256 768h512v-128h-512v128z" fill="#85889C"/>
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
        <path d="M320 512h448v-128h-448v128zM320 768h448v-128h-448v128zM320 128v128h448v-128h-448zM79 384h78v-256h-36l-85 23v50l43-2v185zM189 590c0-36-12-78-96-78-33 0-64 6-83 16l1 66c21-10 42-15 67-15s32 11 32 28c0 26-30 58-110 112v50h192v-67l-91 2c49-30 87-66 87-113l1-1z" fill="#85889C"/>
      </svg>
    `;
    orderedListButton.addEventListener("click", () => {
      wrapInList(schema.nodes.ordered_list)(view.state, view.dispatch);
      view.focus();
    });
    group.appendChild(orderedListButton);
  }

  // Blockquote
  if (schema.nodes.blockquote) {
    const blockquoteButton = document.createElement("button");
    blockquoteButton.className = "editor-general-button";
    blockquoteButton.title = "Blockquote";
    blockquoteButton.dataset.nodeType = "blockquote";
    blockquoteButton.innerHTML = `
      <svg width="24" height="24" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" fill="currentColor">
        <path d="M0 448v256h256v-256h-128c0 0 0-128 128-128v-128c0 0-256 0-256 256zM640 320v-128c0 0-256 0-256 256v256h256v-256h-128c0 0 0-128 128-128z" fill="#85889C"/>
      </svg>
    `;
    blockquoteButton.addEventListener("click", () => {
      wrapIn(schema.nodes.blockquote)(view.state, view.dispatch);
      view.focus();
    });
    group.appendChild(blockquoteButton);
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

export function addFormatDropdown(container: HTMLElement, schema: Schema, view: EditorView) {
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

  // Add headings submenu
  const headingsSubmenu = createHeadingSubmenu(schema, view);
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
      // Force a reflow to make sure the browser applies the style change
      dropdownMenu.getBoundingClientRect();
    }
  });

  // Show submenu on hover
  headingsItem.addEventListener("mouseenter", () => {
    const submenu = headingsItem.querySelector(".submenu");
    if (submenu instanceof HTMLElement) {
      submenu.style.display = "block";
    }
  });

  // Hide submenu when leaving headings item
  headingsItem.addEventListener("mouseleave", () => {
    const submenu = headingsItem.querySelector(".submenu");
    if (submenu instanceof HTMLElement) {
      submenu.style.display = "none";
    }
  });

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