import {
  Plugin,
  PluginKey,
  EditorState,
  Selection,
  TextSelection,
} from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { setBlockType, wrapIn, toggleMark } from "prosemirror-commands";
import { Schema } from "prosemirror-model";
import { wrapInList } from "prosemirror-schema-list";
import type { SlashCommandItem } from "../../types/editor.types";

export const slashCommandKey = new PluginKey("slash-command");

const OPEN_REGEX = /(?:^|\s)\/$/;
const CLOSE_REGEX = /(?:^|\s)(?:\/(\w+))$/;

function isCursorSelection(selection: Selection): selection is TextSelection {
  return selection instanceof TextSelection && selection.empty;
}

export function slashCommandPlugin(schema: Schema) {
  let menu: HTMLElement | null = null;
  let lastState: EditorState | null = null;
  let scrollHandler: (() => void) | null = null;

  const commands = getCommands(schema);
  let isMenuOpen = false;
  function filterCommands(query: string = "") {
    return commands.filter((cmd) =>
      cmd.title.toLowerCase().includes(query.toLowerCase()),
    );
  }

  function createMenu() {
    if (menu) return menu;

    menu = document.createElement("div");
    menu.className = "slash-command-menu";
    menu.style.position = "absolute";
    menu.style.zIndex = "0";
    menu.style.background = "#16171f";
    menu.style.border = "1px solid #2a2b2f";
    menu.style.borderRadius = "4px";
    menu.style.boxShadow = "0 2px 8px rgba(0, 0, 0, 0.25)";
    menu.style.color = "white";
    menu.style.overflow = "hidden";
    menu.style.maxHeight = "300px";
    menu.style.overflowY = "auto";
    menu.style.width = "240px";
    menu.setAttribute("tabindex", "-1");
    return menu;
  }

  function renderMenu(
    view: EditorView,
    filteredCommands: SlashCommandItem[],
    query: string = "",
  ) {
    const menu = createMenu();
    menu.innerHTML = "";

    if (filteredCommands.length === 0) {
      const noResults = document.createElement("div");
      noResults.className = "slash-command-item no-results";
      noResults.textContent = "No commands found";
      noResults.style.padding = "8px 12px";
      noResults.style.color = "#85889C";
      menu.appendChild(noResults);
      return menu;
    }

    filteredCommands.forEach((cmd, index) => {
      const item = document.createElement("div");
      item.className = "slash-command-item";
      item.style.padding = "8px 12px";
      item.style.cursor = "pointer";
      item.style.display = "flex";
      item.style.alignItems = "center";
      item.style.gap = "8px";
      item.setAttribute("tabindex", "0");
      item.setAttribute("data-command-index", index.toString());
      // Hover state
      item.addEventListener("mouseenter", () => {
        item.style.background = "#2a2b2f";
      });

      item.addEventListener("mouseleave", () => {
        item.style.background = "transparent";
      });

      item.addEventListener("mousedown", (event) => {
        event.preventDefault();
        event.stopPropagation();

        const { state, dispatch } = view;
        const tr = state.tr;

        if (isCursorSelection(state.selection)) {
          const $cursor = state.selection.$cursor;

          if ($cursor && $cursor.nodeBefore) {
            const text = $cursor.nodeBefore.text;
            if (text) {
              const slashPos = text.lastIndexOf("/");
              if (slashPos > -1) {
                const from =
                  $cursor.pos - ($cursor.nodeBefore.text.length - slashPos);
                tr.delete(from, $cursor.pos);
              }
            }
          }
        }

        dispatch(tr);
        cmd.command(view.state, view.dispatch, view);
        closeMenu();
        view.focus();
      });
      if (filteredCommands.length > 0) {
        const firstItem = menu.querySelector(
          ".slash-command-item",
        ) as HTMLElement;
        if (firstItem) {
          firstItem.setAttribute("data-selected", "true");
          firstItem.style.background = "#2a2b2f";
        }
      }

      if (cmd.icon) {
        const icon = document.createElement("span");
        icon.className = "slash-command-icon";
        icon.innerHTML = cmd.icon;
        icon.style.width = "20px";
        icon.style.height = "20px";
        icon.style.display = "flex";
        icon.style.alignItems = "center";
        icon.style.justifyContent = "center";
        item.appendChild(icon);
      }

      const content = document.createElement("div");
      content.style.display = "flex";
      content.style.flexDirection = "column";

      const title = document.createElement("div");
      title.className = "slash-command-title";
      title.textContent = cmd.title;
      title.style.fontWeight = "500";
      content.appendChild(title);

      if (cmd.description) {
        const desc = document.createElement("div");
        desc.className = "slash-command-description";
        desc.textContent = cmd.description;
        desc.style.fontSize = "12px";
        desc.style.color = "#85889C";
        content.appendChild(desc);
      }

      item.appendChild(content);
      menu.appendChild(item);
    });

    return menu;
  }

  function positionMenu(view: EditorView) {
    if (!menu) return;

    const { state } = view;
    const { selection } = state;

    const scrollContainer = view.dom.closest<HTMLElement>('.editor-main');
    if (!scrollContainer) {
      console.warn("Slash menu: Could not find '.editor-main' scroll container. Positioning may be incorrect.");
      closeMenu(view);
      return;
    }
    const offsetParent = menu.offsetParent as HTMLElement || document.body;
    const coords = view.coordsAtPos(selection.from);

    const scrollContainerRect = scrollContainer.getBoundingClientRect();
    const offsetParentRect = offsetParent.getBoundingClientRect();
    const menuRect = menu.getBoundingClientRect();
    const cursorVisible = coords.top >= scrollContainerRect.top && coords.bottom <= scrollContainerRect.bottom;

    if (!cursorVisible) {
      closeMenu(view);
      return;
    }

    let targetTopWindow = coords.bottom + 10;
    let targetLeftWindow = coords.left;

    const spaceBelow = scrollContainerRect.bottom - coords.bottom;
    if (spaceBelow < menuRect.height + 10) {
      const spaceAbove = coords.top - scrollContainerRect.top;
      if (spaceAbove >= menuRect.height + 10) {
        targetTopWindow = coords.top - menuRect.height - 10;
      } else {
        targetTopWindow = scrollContainerRect.bottom - menuRect.height - 5;
      }
    }

    if (targetLeftWindow < scrollContainerRect.left) {
      targetLeftWindow = scrollContainerRect.left + 5;
    } else if (targetLeftWindow + menuRect.width > scrollContainerRect.right) {
      targetLeftWindow = scrollContainerRect.right - menuRect.width - 5;
    }

    const finalTop = targetTopWindow - offsetParentRect.top;
    const finalLeft = targetLeftWindow - offsetParentRect.left;

    menu.style.position = "absolute";
    menu.style.top = `${finalTop}px`;
    menu.style.left = `${finalLeft}px`;
  }

  function closeMenu(view?: EditorView) {
    if (menu && menu.parentNode) {
      menu.parentNode.removeChild(menu);
    }

    if (view) {
      const { state, dispatch } = view;
      const selection = state.selection;

      if (isCursorSelection(selection)) {
        const $cursor = selection.$cursor; // Access $cursor safely after check

        if ($cursor && $cursor.nodeBefore) {
          const text = $cursor.nodeBefore.text;
          if (text) {
            const slashPos = text.lastIndexOf("/");
            if (slashPos > -1) {
              const from =
                $cursor.pos - ($cursor.nodeBefore.text.length - slashPos);
              dispatch(state.tr.delete(from, $cursor.pos));
            }
          }
        }
      }
    }

    isMenuOpen = false;
  }

  return new Plugin({
    key: slashCommandKey,

    view(editorView) {
      const editorDom = editorView.dom;
      const scrollableContainer =
        editorDom.closest(".editor-main") || editorDom;

      scrollHandler = () => {
        if (isMenuOpen) {
          closeMenu(editorView);
        }
      };

      scrollableContainer.addEventListener("scroll", scrollHandler);
      return {
        update: (view, prevState) => {
          lastState = view.state;

          const { selection } = view.state;
          if (!selection.empty) {
            closeMenu();
            return;
          }

          if (!isCursorSelection(selection)) {
            closeMenu();
            return;
          }

          const $cursor = selection.$cursor;

          if (!$cursor) {
            closeMenu();
            return;
          }

          const textBefore = $cursor.nodeBefore?.text || "";

          if (OPEN_REGEX.test(textBefore)) {
            const filteredCommands = filterCommands();

            if (!isMenuOpen) {
              menu = renderMenu(view, filteredCommands);
              editorView.dom.parentNode?.appendChild(menu);
              positionMenu(view);
              view.focus();
              isMenuOpen = true;
            } else {
              menu = renderMenu(view, filteredCommands);
              if (menu.parentNode) {
                menu.parentNode.replaceChild(menu, menu);
              } else {
                editorView.dom.parentNode?.appendChild(menu);
              }
              positionMenu(view);
            }
          }
          else if (isMenuOpen && CLOSE_REGEX.test(textBefore)) {
            const match = CLOSE_REGEX.exec(textBefore);
            const query = match ? match[1] : "";

            const filteredCommands = filterCommands(query);
            renderMenu(view, filteredCommands, query);
            positionMenu(view);
          }
          else if (isMenuOpen) {
            closeMenu();
          }
        },

        destroy: () => {
          if (scrollHandler && scrollableContainer) {
            scrollableContainer.removeEventListener("scroll", scrollHandler);
            scrollHandler = null;
          }
          closeMenu();
        },
      };
    },

    props: {
      handleKeyDown(view, event) {

        if (!isMenuOpen) return false;

        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();

          event.stopPropagation();

          const items = menu?.querySelectorAll(".slash-command-item") || [];
          if (items.length === 0) return false;

          let currentIndex = Array.from(items).findIndex((item) =>
            item.hasAttribute("data-selected"),
          );

          if (currentIndex === -1) {
            currentIndex = event.key === "ArrowDown" ? -1 : items.length;
          }

          let nextIndex;
          if (event.key === "ArrowDown") {
            nextIndex = currentIndex < items.length - 1 ? currentIndex + 1 : 0;
          } else {
            nextIndex = currentIndex > 0 ? currentIndex - 1 : items.length - 1;
          }

          items.forEach((item) => {
            item.removeAttribute("data-selected");
            (item as HTMLElement).style.background = "transparent";
          });

          const nextItem = items[nextIndex] as HTMLElement;
          if (nextItem) {
            nextItem.setAttribute("data-selected", "true");
            nextItem.style.background = "#2a2b2f";
            nextItem.scrollIntoView({
              block: "center",
              behavior: "smooth",
            });
          }
          return true;
        } else if (event.key === "Enter") {
          event.preventDefault(); // Prevent default Enter behavior
          event.stopPropagation();
          const selectedItem = menu?.querySelector(
            ".slash-command-item[data-selected='true']",
          ) as HTMLElement;
          if (selectedItem && view) {

            const commandItem = selectedItem; // Use the element itself to trigger the click
            const mouseDownEvent = new MouseEvent("mousedown", {
              bubbles: true,
              cancelable: true,
              view: window,
            });
            if (commandItem) {
              commandItem.dispatchEvent(mouseDownEvent);
              return true;
            }
          }
          return false;
        } else if (event.key === "Escape") {
          closeMenu();
          return true;
        }

        return false;
      },
    },
  });
}

function getCommands(schema: Schema): SlashCommandItem[] {
  const commands: SlashCommandItem[] = [];

  if (schema.nodes.heading) {
    for (let i = 1; i <= 6; i++) {
      commands.push({
        title: `Heading ${i}`,
        description: `Level ${i} heading`,
        icon: `<strong>H${i}</strong>`,
        command: (state, dispatch, view) => {
          return setBlockType(schema.nodes.heading, { level: i })(
            state,
            dispatch,
            view,
          );
        },
      });
    }
  }

  if (schema.nodes.paragraph) {
    commands.push({
      title: "Paragraph",
      description: "Normal text",
      icon: `<span>¶</span>`,
      command: (state, dispatch, view) => {
        return setBlockType(schema.nodes.paragraph)(state, dispatch, view);
      },
    });
  }

  if (schema.nodes.bullet_list) {
    commands.push({
      title: "Bullet List",
      description: "Create a bulleted list",
      icon: `<span>•</span>`,
      command: (state, dispatch, view) => {
        if (dispatch && wrapInList(schema.nodes.bullet_list)(state, dispatch)) {
          if (view) view.focus();
          return true;
        }
        return false;
      },
    });
  }

  if (schema.nodes.ordered_list) {
    commands.push({
      title: "Numbered List",
      description: "Create a numbered list",
      icon: `<span>1.</span>`,
      command: (state, dispatch, view) => {
        if (
          dispatch &&
          wrapInList(schema.nodes.ordered_list)(state, dispatch)
        ) {
          if (view) view.focus();
          return true;
        }
        return false;
      },
    });
  }

  if (schema.nodes.blockquote) {
    commands.push({
      title: "Blockquote",
      description: "Create a blockquote",
      icon: `<span>"</span>`,
      command: (state, dispatch, view) => {
        return wrapIn(schema.nodes.blockquote)(state, dispatch, view);
      },
    });
  }

  if (schema.nodes.code_block) {
    commands.push({
      title: "Code Block",
      description: "Create a code block",
      icon: `<span>{}</span>`,
      command: (state, dispatch, view) => {
        return setBlockType(schema.nodes.code_block)(state, dispatch, view);
      },
    });
  }

  if (schema.marks.strong) {
    commands.push({
      title: "Bold",
      description: "Make text bold",
      icon: `<strong>B</strong>`,
      command: (state, dispatch, view) => {
        return toggleMark(schema.marks.strong)(state, dispatch, view);
      },
    });
  }

  if (schema.marks.em) {
    commands.push({
      title: "Italic",
      description: "Make text italic",
      icon: `<em>I</em>`,
      command: (state, dispatch, view) => {
        return toggleMark(schema.marks.em)(state, dispatch, view);
      },
    });
  }
  if (schema.marks.code) {
    commands.push({
      title: "Inline Code",
      description: "Format as inline code",
      icon: `<span><></span>`,
      command: (state, dispatch, view) => {
        return toggleMark(schema.marks.code)(state, dispatch, view);
      },
    });
  }

  return commands;
}
