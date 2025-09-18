import { Plugin, PluginKey } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import {
  addRowAfter,
  addRowBefore,
  addColumnAfter,
  addColumnBefore,
  deleteRow,
  deleteColumn,
  mergeCells,
  splitCell,
  toggleHeaderRow,
  toggleHeaderColumn,
  toggleHeaderCell,
  setCellAttr
} from "prosemirror-tables";
import { Selection } from "prosemirror-state";
import { createNearSelection } from "../utils/prosemirror-helpers";
const tableContextMenuKey = new PluginKey("table-context-menu");

interface MenuItem {
  label: string;
  icon?: string;
  action: (view: EditorView) => void;
  divider?: boolean;
  disabled?: boolean;
}

class TableContextMenu {
  private view: EditorView;
  private menu: HTMLElement | null = null;
  private clickPos: { pos: number; inside: number } | null = null;

  constructor(view: EditorView) {
    this.view = view;
    this.setupContextMenu();
  }

  private setupContextMenu() {
    // Inject styles
    this.injectStyles();

    // Bind global click to close menu
    document.addEventListener('click', this.handleDocumentClick);

    // Prevent default context menu in editor
    this.view.dom.addEventListener('contextmenu', this.handleContextMenu);
  }

  private injectStyles() {
    if (document.querySelector("#table-context-menu-styles")) {
      return;
    }

    const styles = `
      .table-context-menu {
        position: absolute;
        background-color: #16171f;
        border-radius: 10px;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        border: 1px solid #2a2b2f;
        min-width: 200px;
        overflow: hidden;
        z-index: 50;
        padding: 6px;
        top: 0;
        left: 0;
      }

      .context-menu-item {
        display: flex;
        align-items: center;
        padding: 4px 8px;
        border-radius: 4px;
        cursor: pointer;
        margin-bottom: 2px;
        color: #bfc0cc;
        transition: all 0.15s ease;
      }

      .context-menu-item:hover:not(.disabled) {
        background-color: #2a2b2f;
        color: #e4e4e7;
      }

      .context-menu-item.disabled {
        opacity: 0.4;
        cursor: not-allowed;
      }

      .context-menu-icon {
        margin-right: 4px;
        color: #aaa;
        width: 28px;
        display: flex;
        align-items: center;
        justify-content: center;
      }

      .context-menu-label {
        flex: 1;
        font-size: 0.875rem;
        font-weight: 500;
      }

      .context-menu-shortcut {
        color: #85889C;
        font-size: 0.75rem;
      }

      .context-menu-divider {
        height: 1px;
        background: #2a2b2f;
        margin: 4px 8px;
      }

      .context-menu-submenu {
        position: relative;
      }

      .context-menu-submenu::after {
        content: '▶';
        position: absolute;
        right: 12px;
        color: #85889C;
        font-size: 10px;
      }
    `;

    const styleElement = document.createElement("style");
    styleElement.id = "table-context-menu-styles";
    styleElement.textContent = styles;
    document.head.appendChild(styleElement);
  }

  private handleContextMenu = (event: MouseEvent) => {
    // Get position in the document
    const pos = this.view.posAtCoords({ left: event.clientX, top: event.clientY });
    if (!pos) return;

    // Check if we're in a table
    const $pos = this.view.state.doc.resolve(pos.pos);
    let inTable = false;
    let tableDepth = -1;

    for (let depth = $pos.depth; depth > 0; depth--) {
      const node = $pos.node(depth);
      if (node.type.name === 'table') {
        inTable = true;
        tableDepth = depth;
        break;
      }
      if (node.type.name === 'table_cell' || node.type.name === 'table_header') {
        inTable = true;
      }
    }

    if (inTable) {
      event.preventDefault();
      event.stopPropagation();

      this.clickPos = pos;

      // Use helper for safe selection
      const selection = createNearSelection(this.view.state, pos.pos);
      const tr = this.view.state.tr.setSelection(selection);
      this.view.dispatch(tr);

      this.showMenu(event.clientX, event.clientY);
    }
    // If not in table, let default context menu behavior happen
  };

  private handleDocumentClick = (event: MouseEvent) => {
    if (this.menu && !this.menu.contains(event.target as Node)) {
      this.hideMenu();
      // Refocus the editor after closing menu
      this.view.focus();
    }
  };

  private showMenu(x: number, y: number) {
    // Remove existing menu if any
    this.hideMenu();

    // Create menu items
    const items = this.getMenuItems();

    // Create menu element
    this.menu = document.createElement('div');
    this.menu.className = 'table-context-menu';

    // Render menu items
    items.forEach((item, index) => {
      if (item.divider) {
        const divider = document.createElement('div');
        divider.className = 'context-menu-divider';
        this.menu!.appendChild(divider);
      } else {
        const menuItem = document.createElement('div');
        menuItem.className = `context-menu-item ${item.disabled ? 'disabled' : ''}`;

        // Icon
        if (item.icon) {
          const icon = document.createElement('span');
          icon.className = 'context-menu-icon';
          icon.innerHTML = item.icon;
          menuItem.appendChild(icon);
        }

        // Label
        const label = document.createElement('span');
        label.className = 'context-menu-label';
        label.textContent = item.label;
        menuItem.appendChild(label);

        // Click handler
        if (!item.disabled) {
          menuItem.addEventListener('click', (e) => {
            e.preventDefault();
            e.stopPropagation();
            item.action(this.view);
            this.hideMenu();
            this.view.focus();
          });
        }

        this.menu!.appendChild(menuItem);
      }
    });

    // Position menu
    document.body.appendChild(this.menu);

    // Adjust position to prevent menu from going off-screen
    const menuRect = this.menu.getBoundingClientRect();
    const windowWidth = window.innerWidth;
    const windowHeight = window.innerHeight;

    let finalX = x;
    let finalY = y;

    if (x + menuRect.width > windowWidth) {
      finalX = windowWidth - menuRect.width - 10;
    }

    if (y + menuRect.height > windowHeight) {
      finalY = windowHeight - menuRect.height - 10;
    }

    this.menu.style.left = `${finalX}px`;
    this.menu.style.top = `${finalY}px`;
  }

  private hideMenu() {
    if (this.menu && this.menu.parentNode) {
      this.menu.parentNode.removeChild(this.menu);
      this.menu = null;
    }
    // Always refocus editor when menu is hidden
    setTimeout(() => {
      this.view.focus();
    }, 0);
  }

  private getMenuItems(): MenuItem[] {
    const items: MenuItem[] = [];

    // Row operations
    items.push({
      label: 'Insert Row Above',
      icon: '↑',
      action: (view) => addRowBefore(view.state, view.dispatch)
    });

    items.push({
      label: 'Insert Row Below',
      icon: '↓',
      action: (view) => addRowAfter(view.state, view.dispatch)
    });

    items.push({
      label: 'Delete Row',
      icon: '✕',
      action: (view) => deleteRow(view.state, view.dispatch)
    });

    items.push({ divider: true } as MenuItem);

    // Column operations
    items.push({
      label: 'Insert Column Before',
      icon: '←',
      action: (view) => addColumnBefore(view.state, view.dispatch)
    });

    items.push({
      label: 'Insert Column After',
      icon: '→',
      action: (view) => addColumnAfter(view.state, view.dispatch)
    });

    items.push({
      label: 'Delete Column',
      icon: '✕',
      action: (view) => deleteColumn(view.state, view.dispatch)
    });

    items.push({ divider: true } as MenuItem);

    // Cell operations
    const canMerge = this.canMergeCells();
    const canSplit = this.canSplitCell();

    items.push({
      label: 'Merge Cells',
      icon: '⊞',
      action: (view) => mergeCells(view.state, view.dispatch),
      disabled: !canMerge
    });

    items.push({
      label: 'Split Cell',
      icon: '⊟',
      action: (view) => splitCell(view.state, view.dispatch),
      disabled: !canSplit
    });

    items.push({ divider: true } as MenuItem);

    // Header operations
    items.push({
      label: 'Toggle Header Row',
      icon: '⊤',
      action: (view) => toggleHeaderRow(view.state, view.dispatch)
    });

    items.push({
      label: 'Toggle Header Column',
      icon: '⊣',
      action: (view) => toggleHeaderColumn(view.state, view.dispatch)
    });

    items.push({
      label: 'Toggle Header Cell',
      icon: '⊡',
      action: (view) => toggleHeaderCell(view.state, view.dispatch)
    });

    items.push({ divider: true } as MenuItem);

    // Table operations
    items.push({
      label: 'Delete Table',
      icon: '🗑',
      action: (view) => this.deleteTable(view)
    });

    return items;
  }

  private canMergeCells(): boolean {
    // Check if current selection spans multiple cells
    const state = this.view.state;
    const { selection } = state;

    // Simple check - you might want to make this more robust
    try {
      mergeCells(state, () => { });
      return true;
    } catch {
      return false;
    }
  }

  private canSplitCell(): boolean {
    // Check if current cell can be split
    const state = this.view.state;

    try {
      splitCell(state, () => { });
      return true;
    } catch {
      return false;
    }
  }

  private deleteTable(view: EditorView) {
    const { state, dispatch } = view;
    const { selection } = state;
    const $pos = selection.$from;

    // Find the table node
    for (let depth = $pos.depth; depth > 0; depth--) {
      const node = $pos.node(depth);
      if (node.type.name === 'table') {
        const pos = $pos.before(depth);
        const tr = state.tr.delete(pos, pos + node.nodeSize);
        dispatch(tr);
        return;
      }
    }
  }

  destroy() {
    this.hideMenu();
    this.view.dom.removeEventListener('contextmenu', this.handleContextMenu);
    document.removeEventListener('click', this.handleDocumentClick);
  }
}

export function tableContextMenuPlugin(): Plugin {
  let contextMenu: TableContextMenu | null = null;

  return new Plugin({
    key: tableContextMenuKey,

    view(editorView) {
      contextMenu = new TableContextMenu(editorView);
      return {
        destroy() {
          contextMenu?.destroy();
        }
      };
    }
  });
}
