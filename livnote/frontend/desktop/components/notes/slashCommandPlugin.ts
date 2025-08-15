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

// CSS-in-JS Styles
const SLASH_MENU_STYLES = `
  .slash-menu {
    position: absolute;
    background-color: #16171f;
    border-radius: 10px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    border: 1px solid #2a2b2f;
    width: 18rem;
    overflow: hidden;
    z-index: 50;
    top: 0;
    left: 0;
  }

  .slash-menu-header {
    padding: 8px;
    border-bottom: 1px solid #2a2b2f;
  }

  .slash-menu-title {
    font-size: 0.75rem;
    color: #85889C;
    padding: 4px 8px;
  }

  .slash-menu-content {
    max-height: 20rem;
    overflow-y: auto;
    padding: 4px;
  }

  .slash-menu-footer {
    background-color: #16171f;
    border-top: 1px solid #2a2b2f;
    padding: 8px 12px;
    font-size: 0.75rem;
    color: #85889C;
  }

  .slash-menu-footer span.key {
    color: #bbb;
  }

  .slash-menu-footer span.escape {
    float: right;
  }

  .menu-item {
    display: flex;
    align-items: center;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
    margin-bottom: 2px;
  }

  .menu-item:hover {
    background-color: #2a2b2f;
  }

  .menu-item.selected {
    background-color: #2a2b2f;
  }

  .menu-item-icon {
    margin-right: 4px;
    color: #aaa;
    width: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .menu-item-label {
    flex: 1;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .menu-item-shortcut {
    color: #85889C;
    font-size: 0.75rem;
  }

  .menu-item-icon-span {
    color: #85889C;
  }

`;

// Style injection function
function injectStyles() {
	// Check if styles are already injected
	if (document.querySelector("#slash-menu-styles")) {
		return;
	}

	const styleElement = document.createElement("style");
	styleElement.id = "slash-menu-styles";
	styleElement.textContent = SLASH_MENU_STYLES;
	document.head.appendChild(styleElement);
}

function isCursorSelection(selection: Selection): selection is TextSelection {
	return selection instanceof TextSelection && selection.empty;
}

export function slashCommandPlugin(schema: Schema) {
	let menu: HTMLElement | null = null;
	let lastState: EditorState | null = null;
	let scrollHandler: (() => void) | null = null;

	const commands = getCommands(schema);
	let isMenuOpen = false;

	// Inject styles when plugin is created
	injectStyles();

	function filterCommands(query: string = "") {
		return commands.filter((cmd) =>
			cmd.title.toLowerCase().includes(query.toLowerCase()),
		);
	}

	function createMenu() {
		if (menu) return menu;

		menu = document.createElement("div");
		menu.className = "slash-menu";
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

		// Create header
		const header = document.createElement("div");
		header.className = "slash-menu-header";

		const title = document.createElement("div");
		title.className = "slash-menu-title";
		title.textContent = "Basic blocks";
		header.appendChild(title);

		// Create content
		const content = document.createElement("div");
		content.className = "slash-menu-content";

		if (filteredCommands.length === 0) {
			const noResults = document.createElement("div");
			noResults.className = "menu-item";
			noResults.textContent = "No commands found";
			content.appendChild(noResults);
		} else {
			filteredCommands.forEach((cmd, index) => {
				const item = document.createElement("div");
				item.className = "menu-item";
				item.setAttribute("tabindex", "0");
				item.setAttribute("data-command-index", index.toString());

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

				// Icon
				const icon = document.createElement("div");
				icon.className = "menu-item-icon";
				icon.innerHTML = cmd.icon || `<span>•</span>`;
				item.appendChild(icon);

				// Label
				const label = document.createElement("div");
				label.className = "menu-item-label";
				label.textContent = cmd.title;
				item.appendChild(label);

				// Shortcut (if available)
				if (cmd.description) {
					const shortcut = document.createElement("div");
					shortcut.className = "menu-item-shortcut";
					shortcut.textContent = cmd.description;
					item.appendChild(shortcut);
				}

				content.appendChild(item);
			});

			// Ensure there is always a default selected item
			if (!content.querySelector(".menu-item.selected")) {
				const firstItem = content.querySelector(".menu-item") as HTMLElement;
				if (firstItem) firstItem.classList.add("selected");
			}
		}

		// Create footer
		const footer = document.createElement("div");
		footer.className = "slash-menu-footer";
		footer.innerHTML =
			'Type <span class="key">/ </span> on the page<span class="escape">esc</span>';

		// Assemble menu
		menu.appendChild(header);
		menu.appendChild(content);
		menu.appendChild(footer);

		return menu;
	}

	function positionMenu(view: EditorView) {
		if (!menu) return;

		const { state } = view;
		const { selection } = state;

		const scrollContainer = view.dom.closest<HTMLElement>(".editor-main");
		if (!scrollContainer) {
			console.warn(
				"Slash menu: Could not find '.editor-main' scroll container. Positioning may be incorrect.",
			);
			closeMenu(view);
			return;
		}
		const offsetParent = (menu.offsetParent as HTMLElement) || document.body;
		const coords = view.coordsAtPos(selection.from);

		const scrollContainerRect = scrollContainer.getBoundingClientRect();
		const offsetParentRect = offsetParent.getBoundingClientRect();
		const menuRect = menu.getBoundingClientRect();
		const cursorVisible =
			coords.top >= scrollContainerRect.top &&
			coords.bottom <= scrollContainerRect.bottom;

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
						} 
						else {
								const newMenu = renderMenu(view, filteredCommands);
								if (menu && menu.parentNode) {
									menu.parentNode.replaceChild(newMenu, menu);
									menu = newMenu;
								} else {
									menu = newMenu;
									editorView.dom.parentNode?.appendChild(menu);
								}
								positionMenu(view);
							}
					} else if (isMenuOpen && CLOSE_REGEX.test(textBefore)) {
						const match = CLOSE_REGEX.exec(textBefore);
						const query = match ? match[1] : "";

						const filteredCommands = filterCommands(query);
						renderMenu(view, filteredCommands, query);
						positionMenu(view);
					} else if (isMenuOpen) {
						closeMenu();
					}
				},

				destroy: () => {
					if (scrollHandler && scrollableContainer) {
						scrollableContainer.removeEventListener("scroll", scrollHandler);
						scrollHandler = null;
					}
					closeMenu();
					menu = null;
 					lastState = null;
				},
			};
		},

		props: {
			handleKeyDown(view, event) {
				if (!isMenuOpen) return false;

				if (event.key === "ArrowDown" || event.key === "ArrowUp") {
					event.preventDefault();

					event.stopPropagation();

					const items = menu?.querySelectorAll(".menu-item") || [];
					if (items.length === 0) return false;

					let currentIndex = Array.from(items).findIndex((item) =>
						item.classList.contains("selected"),
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
						item.classList.remove("selected");
					});

					const nextItem = items[nextIndex] as HTMLElement;
					if (nextItem) {
						nextItem.classList.add("selected");
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
						".menu-item.selected",
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
                    // Consume Escape to close the menu and prevent other handlers
                    event.preventDefault();
                    event.stopPropagation();
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

	if (schema.nodes.paragraph) {
		commands.push({
			title: "Text",
			description: "",
			icon: `<span class="menu-item-icon-span">T</span>`,
			command: (state, dispatch, view) => {
				return setBlockType(schema.nodes.paragraph)(state, dispatch, view);
			},
		});
	}

	if (schema.nodes.heading) {
		for (let i = 1; i <= 3; i++) {
			commands.push({
				title: `Heading ${i}`,
				description: `#`.repeat(i),
				icon: `<strong class="menu-item-icon-span">H${i}</strong>`,
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

	if (schema.nodes.bullet_list) {
		commands.push({
			title: "Bullet List",
			description: "-",
			icon: `<span class="menu-item-icon-span">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M4.809 12.75a1.25 1.25 0 1 1 0 2.5 1.25 1.25 0 0 1 0-2.5M16 13.375a.625.625 0 1 1 0 1.25H8.5a.625.625 0 0 1 0-1.25zM4.809 4.75a1.25 1.25 0 1 1 0 2.5 1.25 1.25 0 0 1 0-2.5M16 5.375a.625.625 0 1 1 0 1.25H8.5a.625.625 0 0 1 0-1.25z"></path>
      </svg>
      </span>`,
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
			description: "1.",
			icon: `<span class="menu-item-icon-span">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
      <path d="M5.088 3.026a.55.55 0 0 1 .27.474v4a.55.55 0 0 1-1.1 0V4.435l-.24.134a.55.55 0 1 1-.535-.962l1.059-.588a.55.55 0 0 1 .546.007M8.5 5.375a.625.625 0 1 0 0 1.25H16a.625.625 0 1 0 0-1.25zm0 8a.625.625 0 0 0 0 1.25H16a.625.625 0 1 0 0-1.25zM6 16.55H3.5a.55.55 0 0 1-.417-.908l1.923-2.24a.7.7 0 0 0 .166-.45.335.335 0 0 0-.266-.327l-.164-.035a.6.6 0 0 0-.245.004l-.03.007a.57.57 0 0 0-.426.44.55.55 0 1 1-1.08-.206 1.67 1.67 0 0 1 1.248-1.304l.029-.007c.24-.058.49-.061.732-.01l.164.035c.664.14 1.138.726 1.138 1.404 0 .427-.153.84-.432 1.165L4.697 15.45H6a.55.55 0 0 1 0 1.1"></path>
      </svg>
      </span>`,
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
			description: "'",
			icon: `<span class="menu-item-icon-span">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
      <path d="M15.796 4.971a5.067 5.067 0 0 0-5.067 5.067v.635a4.433 4.433 0 0 0 4.433 4.433 3.164 3.164 0 1 0-3.11-3.75 3.2 3.2 0 0 1-.073-.683v-.635a3.817 3.817 0 0 1 3.817-3.817h.635a.625.625 0 1 0 0-1.25zm-9.054 0a5.067 5.067 0 0 0-5.067 5.068v.634a4.433 4.433 0 0 0 4.433 4.433 3.164 3.164 0 1 0-3.11-3.75 3.2 3.2 0 0 1-.073-.683v-.634A3.817 3.817 0 0 1 6.742 6.22h.635a.625.625 0 1 0 0-1.25z"></path>
      </svg>
      </span>`,
			command: (state, dispatch, view) => {
				return wrapIn(schema.nodes.blockquote)(state, dispatch, view);
			},
		});
	}

	if (schema.nodes.code_block) {
		commands.push({
			title: "Code Block",
			description: "```",
			icon: `<span class="menu-item-icon-span">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
     <path d="M12.6 3.172a.625.625 0 0 0-1.201-.344l-4 14a.625.625 0 0 0 1.202.344zM5.842 5.158a.625.625 0 0 1 0 .884L1.884 10l3.958 3.958a.625.625 0 0 1-.884.884l-4.4-4.4a.625.625 0 0 1 0-.884l4.4-4.4a.625.625 0 0 1 .884 0m8.316 0a.625.625 0 0 1 .884 0l4.4 4.4a.625.625 0 0 1 0 .884l-4.4 4.4a.625.625 0 0 1-.884-.884L18.116 10l-3.958-3.958a.625.625 0 0 1 0-.884"></path>
      </svg>
      </span>`,
			command: (state, dispatch, view) => {
				return setBlockType(schema.nodes.code_block)(state, dispatch, view);
			},
		});
	}

	if (schema.marks.strong) {
		commands.push({
			title: "Bold",
			description: "",
			icon: `<strong class="menu-item-icon-span">B</strong>`,
			command: (state, dispatch, view) => {
				return toggleMark(schema.marks.strong)(state, dispatch, view);
			},
		});
	}

	if (schema.marks.em) {
		commands.push({
			title: "Italic",
			description: "",
			icon: `<em class="menu-item-icon-span"><svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor" >
        <path d="m16.7 4.7-.1.9h-.3c-.6 0-1 0-1.4.3-.3.3-.4.6-.5 1.1l-2.1 9.8v.6c0 .5.4.8 1.4.8h.2l-.2.8H8l.2-.8h.2c1.1 0 1.8-.5 2-1.5l2-9.8.1-.5c0-.6-.4-.8-1.4-.8h-.3l.2-.9h5.8Z" fill-rule="evenodd" fill="currentColor">
        </path>
      </svg></em>`,
			command: (state, dispatch, view) => {
				return toggleMark(schema.marks.em)(state, dispatch, view);
			},
		});
	}
	if (schema.marks.code) {
		commands.push({
			title: "Inline Code",
			description: "",
			icon: `<span class="menu-item-icon-span"><></span>`,
			command: (state, dispatch, view) => {
				return toggleMark(schema.marks.code)(state, dispatch, view);
			},
		});
	}

	return commands;
}
