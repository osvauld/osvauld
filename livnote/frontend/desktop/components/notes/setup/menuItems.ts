import { EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import {
  wrapIn,
  setBlockType,
  chainCommands,
  toggleMark,
} from "prosemirror-commands";
import { wrapInList, liftListItem } from "prosemirror-schema-list";
import { undo, redo } from "y-prosemirror";
import { indentRight, indentLeft } from "./indentUtils";
import { setTextAlign } from "./alignmentUtils";
import { hideDropdowns } from "./dropdownUtils";
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { readFile } from '@tauri-apps/plugin-fs';

// Helper to create a button with a standard structure
function createButton(options: {
  className: string;
  title: string;
  innerHTML: string;
  onClick: (e: MouseEvent) => void;
  dataset?: Record<string, string>;
}): HTMLButtonElement {
  const button = document.createElement("button");
  button.className = options.className;
  button.title = options.title;
  button.innerHTML = options.innerHTML;
  button.addEventListener("click", (e) => {
    e.preventDefault();
    options.onClick(e);
  });
  if (options.dataset) {
    Object.assign(button.dataset, options.dataset);
  }
  return button;
}

// --- History Items (Undo/Redo) ---
export function addHistoryItems(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  group.appendChild(
    createButton({
      className: "editor-general-button", title: "Undo last change",
      innerHTML: `<svg width="24" height="24" focusable="false"><path d="M6.4 8H12c3.7 0 6.2 2 6.8 5.1.6 2.7-.4 5.6-2.3 6.8a1 1 0 0 1-1-1.8c1.1-.6 1.8-2.7 1.4-4.6-.5-2.1-2.1-3.5-4.9-3.5H6.4l3.3 3.3a1 1 0 1 1-1.4 1.4l-5-5a1 1 0 0 1 0-1.4l5-5a1 1 0 0 1 1.4 1.4L6.4 8Z" fill-rule="nonzero" fill="#85889C"></path></svg>`,
      onClick: () => { undo(view.state, view.dispatch); view.focus(); },
    })
  );
  group.appendChild(
    createButton({
      className: "editor-general-button", title: "Redo last undone change",
      innerHTML: `<svg width="24" height="24" focusable="false"><path d="M17.6 10H12c-2.8 0-4.4 1.4-4.9 3.5-.4 2 .3 4 1.4 4.6a1 1 0 1 1-1 1.8c-2-1.2-2.9-4.1-2.3-6.8.6-3 3-5.1 6.8-5.1h5.6l-3.3-3.3a1 1 0 1 1 1.4-1.4l5 5a1 1 0 0 1 0 1.4l-5 5a1 1 0 0 1-1.4-1.4l3.3-3.3Z" fill-rule="nonzero" fill="#85889C"></path></svg>`,
      onClick: () => { redo(view.state, view.dispatch); view.focus(); },
    })
  );
  container.appendChild(group);
}

// --- Basic Formatting (Bold, Italic) ---
export function addFormattingItems(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  if (schema.marks.strong) {
    group.appendChild(createButton({
      className: "editor-general-button menu-bold", title: "Bold", dataset: { markType: "strong" },
      innerHTML: `<svg width="24" height="24" focusable="false"><path d="M7.8 19c-.3 0-.5 0-.6-.2l-.2-.5V5.7c0-.2 0-.4.2-.5l.6-.2h5c1.5 0 2.7.3 3.5 1 .7.6 1.1 1.4 1.1 2.5a3 3 0 0 1-.6 1.9c-.4.6-1 1-1.6 1.2.4.1.9.3 1.3.6s.8.7 1 1.2c.4.4.5 1 .5 1.6 0 1.3-.4 2.3-1.3 3-.8.7-2.1 1-3.8 1H7.8Zm5-8.3c.6 0 1.2-.1 1.6-.5.4-.3.6-.7.6-1.3 0-1.1-.8-1.7-2.3-1.7H9.3v3.5h3.4Zm.5 6c.7 0 1.3-.1 1.7-.4.4-.4.6-.9.6-1.5s-.2-1-.7-1.4c-.4-.3-1-.4-2-.4H9.4v3.8h4Z" fill-rule="evenodd" fill="#85889C"></path></svg>`,
      onClick: () => { toggleMark(schema.marks.strong)(view.state, view.dispatch); view.focus(); },
    }));
  }
  if (schema.marks.em) {
    group.appendChild(createButton({
      className: "editor-general-button menu-italic", title: "Italic", dataset: { markType: "em" },
      innerHTML: `<svg width="24" height="24" focusable="false"><path d="m16.7 4.7-.1.9h-.3c-.6 0-1 0-1.4.3-.3.3-.4.6-.5 1.1l-2.1 9.8v.6c0 .5.4.8 1.4.8h.2l-.2.8H8l.2-.8h.2c1.1 0 1.8-.5 2-1.5l2-9.8.1-.5c0-.6-.4-.8-1.4-.8h-.3l.2-.9h5.8Z" fill-rule="evenodd" fill="#85889C"></path></svg>`,
      onClick: () => { toggleMark(schema.marks.em)(view.state, view.dispatch); view.focus(); },
    }));
  }
  container.appendChild(group);
}

// --- List Items ---
export function addListItems(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  if (schema.nodes.bullet_list) {
    group.appendChild(createButton({
      className: "editor-general-button", title: "Bullet list", dataset: { nodeType: "bullet_list" },
      innerHTML: `<svg width="24" height="24" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" fill="currentColor"><g transform="translate(140, 60)"><path d="M0 512h128v-128h-128v128zM0 256h128v-128h-128v128zM0 768h128v-128h-128v128zM256 512h512v-128h-512v128zM256 256h512v-128h-512v128zM256 768h512v-128h-512v128z" fill="#85889C"/></g></svg>`,
      onClick: () => { wrapInList(schema.nodes.bullet_list)(view.state, view.dispatch); view.focus(); },
    }));
  }
  if (schema.nodes.ordered_list) {
    group.appendChild(createButton({
      className: "editor-general-button", title: "Ordered list", dataset: { nodeType: "ordered_list" },
      innerHTML: `<svg width="24" height="24" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" fill="currentColor"><g transform="translate(140, 60)"><path d="M320 512h448v-128h-448v128zM320 768h448v-128h-448v128zM320 128v128h448v-128h-448zM79 384h78v-256h-36l-85 23v50l43-2v185zM189 590c0-36-12-78-96-78-33 0-64 6-83 16l1 66c21-10 42-15 67-15s32 11 32 28c0 26-30 58-110 112v50h192v-67l-91 2c49-30 87-66 87-113l1-1z" fill="#85889C"/></g></svg>`,
      onClick: () => { wrapInList(schema.nodes.ordered_list)(view.state, view.dispatch); view.focus(); },
    }));
  }
  container.appendChild(group);
}

// --- Indent Buttons ---
export function addIndentButtons(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  const indentRightBtn = createButton({
    className: "editor-general-button", title: "Indent right",
    innerHTML: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="#85889C" stroke-width="1.2"><line x1="3" y1="6" x2="21" y2="6" /><line x1="8" y1="10" x2="21" y2="10" /><line x1="8" y1="14" x2="21" y2="14" /><line x1="3" y1="18" x2="21" y2="18" /><path d="M6.5 13L3.5 10M6.5 11L3.5 14" /></g></svg>`,
    onClick: () => indentRight(view)
  });
  const indentLeftBtn = createButton({
    className: "editor-general-button", title: "Indent left",
    innerHTML: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="#85889C" stroke-width="1.2"><line x1="3" y1="6" x2="21" y2="6" /><line x1="8" y1="10" x2="21" y2="10" /><line x1="8" y1="14" x2="21" y2="14" /><line x1="3" y1="18" x2="21" y2="18" /><path d="M3.5 13L6.5 10M3.5 11L6.5 14" /></g></svg>`,
    onClick: () => indentLeft(view)
  });
  group.appendChild(indentRightBtn);
  group.appendChild(indentLeftBtn);
  container.appendChild(group);
  return { indentLeftBtn, indentRightBtn };
}

export function updateIndentButtonsState(elements: { indentLeftBtn: HTMLButtonElement; indentRightBtn: HTMLButtonElement; }, schema: Schema, view: EditorView) {
  const { state } = view;
  const { $from } = state.selection;
  const node = $from.parent;
  const hasIndent = node.attrs.indent && node.attrs.indent > 0;
  const canLiftList = schema.nodes.list_item && liftListItem(schema.nodes.list_item)(state, undefined);
  const canIndentLeft = hasIndent || canLiftList;
  elements.indentLeftBtn.disabled = !canIndentLeft;
  elements.indentLeftBtn.style.opacity = canIndentLeft ? '1' : '0.5';
}

// --- Alignment Buttons ---
export function addAlignmentButtons(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  group.appendChild(createButton({
    className: "editor-general-button", title: "Align left", dataset: { alignment: "left" },
    innerHTML: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="#85889C" stroke-width="1.2"><line x1="3" y1="6" x2="21" y2="6" /><line x1="3" y1="10" x2="15" y2="10" /><line x1="3" y1="14" x2="21" y2="14" /><line x1="3" y1="18" x2="15" y2="18" /></g></svg>`,
    onClick: () => setTextAlign(view, "left")
  }));
  group.appendChild(createButton({
    className: "editor-general-button", title: "Align center", dataset: { alignment: "center" },
    innerHTML: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="#85889C" stroke-width="1.2"><line x1="3" y1="6" x2="21" y2="6" /><line x1="6" y1="10" x2="18" y2="10" /><line x1="3" y1="14" x2="21" y2="14" /><line x1="6" y1="18" x2="18" y2="18" /></g></svg>`,
    onClick: () => setTextAlign(view, "center")
  }));
  group.appendChild(createButton({
    className: "editor-general-button", title: "Align right", dataset: { alignment: "right" },
    innerHTML: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="#85889C" stroke-width="1.2"><line x1="3" y1="6" x2="21" y2="6" /><line x1="9" y1="10" x2="21" y2="10" /><line x1="3" y1="14" x2="21" y2="14" /><line x1="9" y1="18" x2="21" y2="18" /></g></svg>`,
    onClick: () => setTextAlign(view, "right")
  }));
  container.appendChild(group);
}

// --- Block Format Dropdown ---
export function addBlockFormatDropdown(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  const dropdownContainer = document.createElement("div");
  dropdownContainer.className = "dropdown-container";

  const formatButton = document.createElement("button");
  formatButton.className = "format-dropdown-button";
  const dropdownMenu = document.createElement("div");
  dropdownMenu.className = "dropdown-menu";
  dropdownMenu.style.display = "none";

  const paragraphItem = document.createElement("div");
  paragraphItem.className = "dropdown-item";
  paragraphItem.innerHTML = `<span style="font-size: 1em;">Paragraph</span>`;
  paragraphItem.addEventListener("click", () => {
    setBlockType(schema.nodes.paragraph)(view.state, view.dispatch);
    view.focus();
    hideDropdowns();
  });
  dropdownMenu.appendChild(paragraphItem);

  const headings = [{ level: 1, text: "Heading 1" }, { level: 2, text: "Heading 2" }, { level: 3, text: "Heading 3" }, { level: 4, text: "Heading 4" }, { level: 5, text: "Heading 5" }, { level: 6, text: "Heading 6" }];
  headings.forEach(heading => {
    const headingItem = document.createElement("div");
    headingItem.className = "dropdown-item";
    const styles: Record<number, string> = { 1: '2em', 2: '1.5em', 3: '1.17em', 4: '1.1em', 5: '1.05em', 6: '1em' };
    headingItem.innerHTML = `<span style="font-size: ${styles[heading.level]}; font-weight: bold;">${heading.text}</span>`;
    headingItem.addEventListener("click", () => {
      setBlockType(schema.nodes.heading, { level: heading.level })(view.state, view.dispatch);
      view.focus();
      hideDropdowns();
    });
    dropdownMenu.appendChild(headingItem);
  });

  formatButton.addEventListener("click", (e) => {
    e.stopPropagation();
    const isVisible = dropdownMenu.style.display === "block";
    hideDropdowns();
    if (!isVisible) {
      dropdownMenu.style.display = "block";
    }
  });

  dropdownContainer.appendChild(formatButton);
  dropdownContainer.appendChild(dropdownMenu);
  group.appendChild(dropdownContainer);
  container.appendChild(group);
  return { formatButton };
}

export function updateBlockFormatButton(elements: { formatButton: HTMLButtonElement }, schema: Schema, view: EditorView) {
  const { $from } = view.state.selection;
  const node = $from.parent;
  let currentBlock = "Paragraph";
  if (node.type === schema.nodes.heading) {
    currentBlock = `Heading ${node.attrs.level}`;
  }
  elements.formatButton.innerHTML = `
        <span>${currentBlock}</span>
        <svg width="12" height="12" viewBox="0 0 24 24" focusable="false"><path d="M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z" fill="#85889C"></path></svg>
    `;
}

// --- Text Size Controls ---
export function addTextSizeControls(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  const fontSizeControls = document.createElement("div");
  fontSizeControls.className = "font-size-controls";

  const fontSizeInput = document.createElement("input");
  fontSizeInput.type = "text";
  fontSizeInput.className = "font-size-input";

  const applyFontSize = (newSize: string) => {
    const { state, dispatch } = view;
    const { from, to, empty, $from } = state.selection;
    const [markStart, markEnd] = empty ? [$from.start(), $from.end()] : [from, to];
    if (schema.marks.fontSize) {
      const tr = state.tr;
      tr.removeMark(markStart, markEnd, schema.marks.fontSize);
      tr.addMark(markStart, markEnd, schema.marks.fontSize.create({ size: newSize }));
      dispatch(tr);
    }
    view.focus();
  };

  const decreaseButton = createButton({
    className: "size-adjust-button", title: "Decrease font size",
    innerHTML: `<svg width="16" height="16" viewBox="0 0 24 24" focusable="false"><path d="M19 13H5v-2h14v2z" fill="currentColor"/></svg>`,
    onClick: () => {
      const currentSize = parseInt(fontSizeInput.value) || 16;
      if (currentSize > 8) {
        const newSize = `${currentSize - 1}px`;
        fontSizeInput.value = newSize;
        applyFontSize(newSize);
      }
    }
  });

  const increaseButton = createButton({
    className: "size-adjust-button", title: "Increase font size",
    innerHTML: `<svg width="16" height="16" viewBox="0 0 24 24" focusable="false"><path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z" fill="currentColor"/></svg>`,
    onClick: () => {
      const currentSize = parseInt(fontSizeInput.value) || 16;
      if (currentSize < 72) {
        const newSize = `${currentSize + 1}px`;
        fontSizeInput.value = newSize;
        applyFontSize(newSize);
      }
    }
  });

  fontSizeInput.addEventListener("change", () => {
    let size = parseInt(fontSizeInput.value);
    if (isNaN(size)) size = 16;
    size = Math.min(72, Math.max(8, size));
    const newSize = `${size}px`;
    fontSizeInput.value = newSize;
    applyFontSize(newSize);
  });

  fontSizeControls.appendChild(decreaseButton);
  fontSizeControls.appendChild(fontSizeInput);
  fontSizeControls.appendChild(increaseButton);
  group.appendChild(fontSizeControls);
  container.appendChild(group);

  return { fontSizeInput };
}

export function updateFontSizeDisplay(elements: { fontSizeInput: HTMLInputElement }, schema: Schema, view: EditorView) {
  if (!elements.fontSizeInput.isConnected) return;

  const { state } = view;
  const { $from } = state.selection;

  if (schema.marks.fontSize) {
    const fontSizeMark = schema.marks.fontSize.isInSet($from.marks());
    if (fontSizeMark && fontSizeMark.attrs.size) {
      elements.fontSizeInput.value = fontSizeMark.attrs.size;
      return;
    }
  }

  const node = $from.parent;
  if (node.type === schema.nodes.heading) {
    const level = node.attrs.level as number;
    const headingSizes: Record<number, string> = { 1: "36px", 2: "27px", 3: "21px", 4: "20px", 5: "19px", 6: "16px" };
    elements.fontSizeInput.value = headingSizes[level] || "18px";
    return;
  }

  elements.fontSizeInput.value = "16px";
}

// --- Secondary Formatting (Underline, Strikethrough, Image) ---
export function addSecondaryFormattingItems(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  const buttons: { underlineButton?: HTMLButtonElement, strikethroughButton?: HTMLButtonElement } = {};

  if (schema.marks.underline) {
    buttons.underlineButton = createButton({
      className: "editor-general-button menu-underline", title: "Underline", dataset: { markType: "underline" },
      innerHTML: `<svg width="24" height="24" focusable="false"><path d="M16 5c.6 0 1 .4 1 1v5.5a4 4 0 0 1-.4 1.8l-1 1.4a5.3 5.3 0 0 1-5.5 1 5 5 0 0 1-1.6-1c-.5-.4-.8-.9-1.1-1.4a4 4 0 0 1-.4-1.8V6c0-.6.4-1 1-1s1 .4 1 1v5.5c0 .3 0 .6.2 1l.6.7a3.3 3.3 0 0 0 2.2.8 3.4 3.4 0 0 0 2.2-.8c.3-.2.4-.5.6-.8l.2-.9V6c0-.6.4-1 1-1ZM8 17h8c.6 0 1 .4 1 1s-.4 1-1 1H8a1 1 0 0 1 0-2Z" fill-rule="evenodd" fill="#85889C"></path></svg>`,
      onClick: () => { toggleMark(schema.marks.underline)(view.state, view.dispatch); view.focus(); }
    });
    group.appendChild(buttons.underlineButton);
  }
  if (schema.marks.strikethrough) {
    buttons.strikethroughButton = createButton({
      className: "editor-general-button menu-strikethrough", title: "Strikethrough", dataset: { markType: "strikethrough" },
      innerHTML: `<svg width="24" height="24" focusable="false"><g fill-rule="evenodd"><path d="M15.6 8.5c-.5-.7-1-1.1-1.3-1.3-.6-.4-1.3-.6-2-.6-2.7 0-2.8 1.7-2.8 2.1 0 1.6 1.8 2 3.2 2.3 4.4.9 4.6 2.8 4.6 3.9 0 1.4-.7 4.1-5 4.1A6.2 6.2 0 0 1 7 16.4l1.5-1.1c.4.6 1.6 2 3.7 2 1.6 0 2.5-.4 3-1.2.4-.8.3-2-.8-2.6-.7-.4-1.6-.7-2.9-1-1-.2-3.9-.8-3.9-3.6C7.6 6 10.3 5 12.4 5c2.9 0 4.2 1.6 4.7 2.4l-1.5 1.1Z" fill="#85889C"></path><path d="M5 11h14a1 1 0 0 1 0 2H5a1 1 0 0 1 0-2Z" fill-rule="nonzero" fill="#85889C"></path></g></svg>`,
      onClick: () => { toggleMark(schema.marks.strikethrough)(view.state, view.dispatch); view.focus(); }
    });
    group.appendChild(buttons.strikethroughButton);
  }
  if (schema.nodes.image) {
    const imageButton = createButton({
      className: "editor-general-button menu-image", title: "Insert image",
      innerHTML: `<svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2zM8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z" fill="#85889C"/></svg>`,
      onClick: async () => {
        // This complex async logic is self-contained and doesn't create persistent listeners, so it's safe here.
        try {
          const selectedPath = await openDialog({
            multiple: false,
            filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'] }]
          });
          const path = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;

          if (typeof path === 'string') {
            const binaryData = await readFile(path);
            const blobToBase64 = (blob: Blob): Promise<string> => new Promise((resolve, reject) => {
              const reader = new FileReader();
              reader.onloadend = () => resolve(reader.result as string);
              reader.onerror = reject;
              reader.readAsDataURL(blob);
            });

            const dataUrl = await blobToBase64(new Blob([binaryData]));
            const filename = path.split('/').pop() || 'image';
            const imageNode = schema.nodes.image.create({ src: dataUrl, alt: filename, title: filename });
            const { state, dispatch } = view;
            dispatch(state.tr.replaceSelectionWith(imageNode));
            view.focus();
          }
        } catch (error) {
          console.error("Error selecting image:", error);
        }
      }
    });
    group.appendChild(imageButton);
  }
  container.appendChild(group);
  return buttons;
}

export function updateSecondaryFormatButtons(elements: { underlineButton?: HTMLButtonElement, strikethroughButton?: HTMLButtonElement }, schema: Schema, view: EditorView) {
  const { state } = view;
  const { selection } = state;
  const { empty } = selection;
  if (elements.underlineButton) {
    elements.underlineButton.disabled = empty;
    elements.underlineButton.style.opacity = empty ? '0.5' : '1';
  }
  if (elements.strikethroughButton) {
    elements.strikethroughButton.disabled = empty;
    elements.strikethroughButton.style.opacity = empty ? '0.5' : '1';
  }
}

// --- Block Styles (Blockquote, Code Block) ---
export function addBlockStyleItems(container: HTMLElement, schema: Schema, view: EditorView) {
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  if (schema.nodes.blockquote) {
    group.appendChild(createButton({
      className: "editor-general-button", title: "Blockquote", dataset: { nodeType: "blockquote" },
      innerHTML: `<svg width="24" height="24" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" fill="currentColor"><g transform="translate(170, 60)"><path d="M0 448v256h256v-256h-128c0 0 0-128 128-128v-128c0 0-256 0-256 256zM640 320v-128c0 0-256 0-256 256v256h256v-256h-128c0 0 0-128 128-128z" fill="#85889C"/></g></svg>`,
      onClick: () => { chainCommands(liftListItem(schema.nodes.list_item), wrapIn(schema.nodes.blockquote))(view.state, view.dispatch); view.focus(); }
    }));
  }
  if (schema.nodes.code_block) {
    group.appendChild(createButton({
      className: "editor-general-button", title: "Code block", dataset: { nodeType: "code_block" },
      innerHTML: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path d="M9.4 16.6L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4zm5.2 0l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4z" fill="#85889C"/></svg>`,
      onClick: () => { setBlockType(schema.nodes.code_block)(view.state, view.dispatch); view.focus(); }
    }));
  }
  container.appendChild(group);
}

// --- Text Color Picker ---
export function addTextColorPicker(container: HTMLElement, schema: Schema, view: EditorView) {
  if (!schema.marks.textColor) return {};
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  const dropdownContainer = document.createElement("div");
  dropdownContainer.className = "dropdown-container";

  const colorButton = createButton({
    className: "editor-general-button text-color-button", title: "Text color",
    innerHTML: `<svg width="24" height="24" viewBox="0 0 24 24" focusable="false"><path d="M8.7 16h-.8a.5.5 0 0 1-.5-.6l2.7-9c.1-.3.3-.4.5-.4h2.8c.2 0 .4.1.5.4l2.7 9a.5.5 0 0 1-.5.6h-.8a.5.5 0 0 1-.4-.4l-.7-2.2c0-.3-.3-.4-.5-.4h-3.4c-.2 0-.4.1-.5.4l-.7 2.2c0 .3-.2.4-.4.4Zm2.6-7.6-.6 2a.5.5 0 0 0 .5.6h1.6a.5.5 0 0 0 .5-.6l-.6-2c0-.3-.3-.4-.5-.4h-.4c-.2 0-.4.1-.5.4Z" fill="#85889C"></path><rect x="4" y="19" width="16" height="1.5" rx="1" ry="1" class="color-indicator" fill="#000" /></svg>`,
    onClick: (e) => {
      e.stopPropagation();
      if (view.state.selection.empty) return;
      const dropdownMenu = dropdownContainer.querySelector(".dropdown-menu") as HTMLElement;
      if (dropdownMenu) {
        const isVisible = dropdownMenu.style.display === "grid";
        hideDropdowns();
        if (!isVisible) dropdownMenu.style.display = "grid";
      }
    }
  });
  const colorIndicator = colorButton.querySelector('.color-indicator') as SVGRectElement;
  const dropdownMenu = document.createElement("div");
  dropdownMenu.className = "dropdown-menu color-picker-dropdown";
  dropdownMenu.style.display = "none";

  const applyColor = (color: string | null) => {
    const { state, dispatch } = view;
    const { from, to } = state.selection;
    const tr = state.tr.removeMark(from, to, schema.marks.textColor);
    if (color) {
      tr.addMark(from, to, schema.marks.textColor.create({ color }));
    }
    dispatch(tr);
    hideDropdowns();
    view.focus();
  };

  const colors = ["#000000", "#FF0000", "#FFA500", "#FFFF00", "#008000", "#0000FF", "#4B0082", "#EE82EE", "#FFFFFF"];
  colors.forEach(color => {
    const swatch = document.createElement("div");
    swatch.className = "color-swatch";
    swatch.style.backgroundColor = color;
    if (color === "#FFFFFF") swatch.style.border = "1px solid #ccc";
    swatch.addEventListener("click", e => { e.stopPropagation(); applyColor(color); });
    dropdownMenu.appendChild(swatch);
  });
  const removeColorButton = document.createElement("button");
  removeColorButton.className = "remove-color-button";
  removeColorButton.textContent = "Remove Color";
  removeColorButton.addEventListener("click", e => { e.stopPropagation(); applyColor(null); });
  dropdownMenu.appendChild(removeColorButton);

  dropdownContainer.appendChild(colorButton);
  dropdownContainer.appendChild(dropdownMenu);
  group.appendChild(dropdownContainer);
  container.appendChild(group);

  return { colorButton, colorIndicator };
}

export function updateTextColorButton(elements: { colorButton?: HTMLButtonElement, colorIndicator?: SVGRectElement }, schema: Schema, view: EditorView) {
  if (!elements.colorButton || !elements.colorIndicator) return;
  const { state } = view;
  const { selection } = state;
  elements.colorButton.disabled = selection.empty;
  elements.colorButton.style.opacity = selection.empty ? '0.5' : '1';
  const mark = schema.marks.textColor.isInSet(selection.$from.marks());
  elements.colorIndicator.setAttribute('fill', mark ? mark.attrs.color : '#000000');
}

// --- Font Family Dropdown ---
export function addFontFamilyDropdown(container: HTMLElement, schema: Schema, view: EditorView) {
  if (!schema.marks.fontFamily) return {};
  const group = document.createElement("div");
  group.className = "editor-menu-group";
  const dropdownContainer = document.createElement("div");
  dropdownContainer.className = "dropdown-container";

  const fontButton = createButton({
    className: "font-family-dropdown-button", title: "Font family",
    innerHTML: `<span class="font-name">Arial</span><svg width="12" height="12" viewBox="0 0 24 24" focusable="false"><path d="M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z" fill="#85889C"></path></svg>`,
    onClick: e => {
      e.stopPropagation();
      const dropdownMenu = dropdownContainer.querySelector('.dropdown-menu') as HTMLElement;
      if (dropdownMenu) {
        const isVisible = dropdownMenu.style.display === "block";
        hideDropdowns();
        if (!isVisible) dropdownMenu.style.display = "block";
      }
    }
  });
  const fontNameSpan = fontButton.querySelector('.font-name') as HTMLSpanElement;
  const dropdownMenu = document.createElement("div");
  dropdownMenu.className = "dropdown-menu font-family-dropdown";
  dropdownMenu.style.display = "none";

  const applyFont = (family: string | null) => {
    const { state, dispatch } = view;
    const { from, to, empty } = state.selection;
    const tr = state.tr;
    if (empty) {
      if (family) tr.addStoredMark(schema.marks.fontFamily.create({ family }));
      else tr.removeStoredMark(schema.marks.fontFamily);
    } else {
      tr.removeMark(from, to, schema.marks.fontFamily);
      if (family) tr.addMark(from, to, schema.marks.fontFamily.create({ family }));
    }
    dispatch(tr);
    hideDropdowns();
    view.focus();
  }

  const fontFamilies = [{ name: "Arial", value: "Arial, sans-serif" }, { name: "Comic Sans MS", value: "'Comic Sans MS', cursive" }, { name: "Courier New", value: "'Courier New', monospace" }, { name: "Georgia", value: "Georgia, serif" }, { name: "Impact", value: "Impact, sans-serif" }, { name: "Times New Roman", value: "'Times New Roman', serif" }, { name: "Verdana", value: "Verdana, sans-serif" }];
  fontFamilies.forEach(font => {
    const item = document.createElement("div");
    item.className = "font-family-item";
    item.style.fontFamily = font.value;
    item.textContent = font.name;
    item.addEventListener("click", e => { e.stopPropagation(); applyFont(font.value); });
    dropdownMenu.appendChild(item);
  });
  const removeFontButton = document.createElement("button");
  removeFontButton.className = "remove-font-button";
  removeFontButton.textContent = "Remove Font";
  removeFontButton.addEventListener("click", e => { e.stopPropagation(); applyFont(null); });
  dropdownMenu.appendChild(removeFontButton);

  dropdownContainer.appendChild(fontButton);
  dropdownContainer.appendChild(dropdownMenu);
  group.appendChild(dropdownContainer);
  container.appendChild(group);

  return { fontButton, fontNameSpan };
}

export function updateFontFamilyButton(elements: { fontButton?: HTMLButtonElement, fontNameSpan?: HTMLSpanElement }, schema: Schema, view: EditorView) {
  if (!elements.fontButton || !elements.fontNameSpan) return;
  const { state } = view;
  const { selection } = state;
  const marks = selection.empty ? (state.storedMarks || selection.$from.marks()) : selection.$from.marks();
  const mark = schema.marks.fontFamily.isInSet(marks);
  const fontFamilies = [{ name: "Arial", value: "Arial, sans-serif" }, { name: "Comic Sans MS", value: "'Comic Sans MS', cursive" }, { name: "Courier New", value: "'Courier New', monospace" }, { name: "Georgia", value: "Georgia, serif" }, { name: "Impact", value: "Impact, sans-serif" }, { name: "Times New Roman", value: "'Times New Roman', serif" }, { name: "Verdana", value: "Verdana, sans-serif" }];

  if (mark) {
    const font = fontFamilies.find(f => f.value === mark.attrs.family);
    elements.fontNameSpan.textContent = font ? font.name : "Default";
  } else {
    elements.fontNameSpan.textContent = "Arial";
  }
}
