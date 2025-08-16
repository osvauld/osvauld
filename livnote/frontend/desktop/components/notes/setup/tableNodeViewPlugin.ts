import { EditorView, type NodeView } from "prosemirror-view";
import { Node as ProseMirrorNode } from "prosemirror-model";
import {
  addRowAfter,
  addRowBefore,
  addColumnAfter,
  addColumnBefore,
  deleteRow,
  deleteColumn
} from "prosemirror-tables";
import { Plugin } from "prosemirror-state";
export class TableNodeView implements NodeView {
  dom: HTMLElement;
  table: HTMLElement;
  controlsRow: HTMLElement | null = null;
  node: ProseMirrorNode;
  view: EditorView;
  getPos: () => number;
  contentDOM: HTMLElement;

  constructor(node: ProseMirrorNode, view: EditorView, getPos: () => number) {
    this.node = node;
    this.view = view;
    this.getPos = getPos;

    // Create wrapper container
    this.dom = document.createElement('div');
    this.dom.className = 'table-wrapper-with-controls';

    // Create controls container
    this.createControls();

    // Create the actual table - this will be the contentDOM
    this.table = document.createElement('table');
    this.table.className = 'prosemirror-table';
    this.contentDOM = this.table; // Important: Let ProseMirror handle table content

    this.dom.appendChild(this.table);
  }

  createControls() {
    // Create column controls row
    this.controlsRow = document.createElement('div');
    this.controlsRow.className = 'table-column-controls-overlay';
    this.controlsRow.style.cssText = `
      position: absolute;
      top: -35px;
      left: 30px;
      right: 0;
      height: 30px;
      display: flex;
      z-index: 10;
      pointer-events: none;
    `;

    const numCols = this.getColumnCount();

    // Create column control for each column
    for (let colIndex = 0; colIndex < numCols; colIndex++) {
      const colControl = document.createElement('div');
      colControl.className = 'column-control-overlay';
      colControl.style.cssText = `
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: center;
        background: #1c1d26;
        border: 1px solid #2a2b2f;
        opacity: 0;
        transition: opacity 0.2s ease;
        pointer-events: auto;
        gap: 4px;
      `;

      // Add column button
      const addButton = this.createControlButton('+', () => {
        this.showColumnMenu(colIndex);
      });

      // Drag handle (for future implementation)
      const dragHandle = this.createControlButton('⋮⋮', () => {
        // TODO: Implement column dragging
      });
      dragHandle.style.fontSize = '10px';

      colControl.appendChild(addButton);
      colControl.appendChild(dragHandle);

      // Show on hover
      colControl.addEventListener('mouseenter', () => {
        colControl.style.opacity = '1';
      });

      colControl.addEventListener('mouseleave', () => {
        colControl.style.opacity = '0';
      });

      this.controlsRow.appendChild(colControl);
    }

    this.dom.appendChild(this.controlsRow);

    // Create row controls (will be positioned absolutely)
    this.createRowControls();
  }

  createRowControls() {
    const numRows = this.node.childCount;

    for (let rowIndex = 0; rowIndex < numRows; rowIndex++) {
      const rowControl = document.createElement('div');
      rowControl.className = 'row-control-overlay';
      rowControl.style.cssText = `
        position: absolute;
        left: -35px;
        width: 30px;
        height: auto;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        background: #1c1d26;
        border: 1px solid #2a2b2f;
        opacity: 0;
        transition: opacity 0.2s ease;
        pointer-events: auto;
        gap: 2px;
        padding: 4px 0;
      `;

      // Position based on row
      const rowHeight = 40; // Approximate row height
      rowControl.style.top = `${rowIndex * rowHeight}px`;

      // Add row button
      const addButton = this.createControlButton('+', () => {
        this.showRowMenu(rowIndex);
      });

      // Drag handle
      const dragHandle = this.createControlButton('⋮⋮', () => {
        // TODO: Implement row dragging
      });
      dragHandle.style.fontSize = '8px';

      rowControl.appendChild(addButton);
      rowControl.appendChild(dragHandle);

      // Show on hover
      rowControl.addEventListener('mouseenter', () => {
        rowControl.style.opacity = '1';
      });

      rowControl.addEventListener('mouseleave', () => {
        rowControl.style.opacity = '0';
      });

      this.dom.appendChild(rowControl);
    }
  }

  createControlButton(text: string, onClick: () => void): HTMLElement {
    const button = document.createElement('button');
    button.textContent = text;
    button.style.cssText = `
      background: #4094ef;
      border: none;
      border-radius: 3px;
      color: white;
      width: 16px;
      height: 16px;
      font-size: 10px;
      font-weight: bold;
      cursor: pointer;
      display: flex;
      align-items: center;
      justify-content: center;
    `;

    button.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      onClick();
    });

    button.addEventListener('mouseenter', () => {
      button.style.background = '#3182ce';
    });

    button.addEventListener('mouseleave', () => {
      button.style.background = '#4094ef';
    });

    return button;
  }

  showRowMenu(rowIndex: number) {
    const menu = this.createContextMenu([
      { label: 'Insert Row Above', action: () => addRowBefore(this.view.state, this.view.dispatch) },
      { label: 'Insert Row Below', action: () => addRowAfter(this.view.state, this.view.dispatch) },
      { label: 'Delete Row', action: () => deleteRow(this.view.state, this.view.dispatch) }
    ]);

    this.showMenu(menu);
  }

  showColumnMenu(colIndex: number) {
    const menu = this.createContextMenu([
      { label: 'Insert Column Left', action: () => addColumnBefore(this.view.state, this.view.dispatch) },
      { label: 'Insert Column Right', action: () => addColumnAfter(this.view.state, this.view.dispatch) },
      { label: 'Delete Column', action: () => deleteColumn(this.view.state, this.view.dispatch) }
    ]);

    this.showMenu(menu);
  }

  createContextMenu(items: Array<{ label: string; action: () => void }>): HTMLElement {
    const menu = document.createElement('div');
    menu.className = 'table-context-menu';
    menu.style.cssText = `
      position: fixed;
      background: #16171f;
      border: 1px solid #2a2b2f;
      border-radius: 6px;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
      z-index: 1000;
      min-width: 150px;
      padding: 4px 0;
    `;

    items.forEach(item => {
      const menuItem = document.createElement('div');
      menuItem.textContent = item.label;
      menuItem.style.cssText = `
        padding: 8px 16px;
        color: #bfc0cc;
        cursor: pointer;
        font-size: 14px;
      `;

      menuItem.addEventListener('mouseenter', () => {
        menuItem.style.background = '#2a2b2f';
      });

      menuItem.addEventListener('mouseleave', () => {
        menuItem.style.background = 'transparent';
      });

      menuItem.addEventListener('click', () => {
        item.action();
        this.hideMenu();
      });

      menu.appendChild(menuItem);
    });

    return menu;
  }

  showMenu(menu: HTMLElement) {
    // Position menu at cursor
    const rect = this.view.dom.getBoundingClientRect();
    menu.style.left = `${rect.left + 50}px`;
    menu.style.top = `${rect.top + 50}px`;

    document.body.appendChild(menu);

    // Hide menu on click outside
    const hideOnClick = (e: MouseEvent) => {
      if (!menu.contains(e.target as Node)) {
        this.hideMenu();
        document.removeEventListener('click', hideOnClick);
      }
    };

    setTimeout(() => {
      document.addEventListener('click', hideOnClick);
    }, 0);
  }

  hideMenu() {
    const menu = document.querySelector('.table-context-menu');
    if (menu && menu.parentNode) {
      menu.parentNode.removeChild(menu);
    }
  }

  getColumnCount(): number {
    if (this.node.childCount === 0) return 0;
    return this.node.child(0).childCount;
  }

  update(node: ProseMirrorNode): boolean {
    if (node.type !== this.node.type) return false;
    this.node = node;
    // Recreate controls when table structure changes
    this.updateControls();
    return true;
  }

  updateControls() {
    // Remove old controls
    const oldControls = this.dom.querySelectorAll('.column-control-overlay, .row-control-overlay');
    oldControls.forEach(control => control.remove());

    // Recreate controls
    if (this.controlsRow) {
      this.controlsRow.innerHTML = '';
    }
    this.createControls();
  }

  destroy() {
    this.hideMenu();
  }
}
export function tableNodeViewPlugin(): Plugin {
  return new Plugin({
    props: {
      nodeViews: {
        table(node, view, getPos) {
          return new TableNodeView(node, view, getPos as () => number);
        }
      }
    }
  });
}
