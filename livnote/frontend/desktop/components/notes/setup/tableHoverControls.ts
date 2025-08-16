
import { Plugin, PluginKey } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import {
  addRowAfter,
  addRowBefore,
  addColumnAfter,
  addColumnBefore,
  deleteRow,
  deleteColumn
} from "prosemirror-tables";

const tableHoverKey = new PluginKey("table-hover-enhanced");

interface HoverState {
  currentTable: HTMLElement | null;
  controls: HTMLElement | null;
  activeMenu: HTMLElement | null;
  hoveredRow: number | null;
  hoveredColumn: number | null;
}

export function tableHoverPlugin(): Plugin {
  let state: HoverState = {
    currentTable: null,
    controls: null,
    activeMenu: null,
    hoveredRow: null,
    hoveredColumn: null
  };

  function createControls(): HTMLElement {
    const container = document.createElement('div');
    container.className = 'table-hover-controls-enhanced';
    container.style.cssText = `
      position: absolute;
      pointer-events: none;
      z-index: 20;
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', sans-serif;
    `;
    return container;
  }

  function createAddButton(text: string, onClick: () => void, className: string = ''): HTMLElement {
    const button = document.createElement('button');
    button.innerHTML = text;
    button.className = `table-add-btn ${className}`;
    button.style.cssText = `
      position: absolute;
      width: 28px;
      height: 28px;
      background: #4094ef;
      border: 2px solid #ffffff;
      border-radius: 50%;
      color: white;
      font-size: 16px;
      font-weight: bold;
      cursor: pointer;
      display: flex;
      align-items: center;
      justify-content: center;
      pointer-events: auto;
      opacity: 0;
      transition: all 0.2s ease;
      box-shadow: 0 4px 12px rgba(64, 148, 239, 0.3);
      transform: scale(0.8);
      z-index: 1000;
    `;

    button.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      onClick();
    });

    button.addEventListener('mouseenter', () => {
      button.style.background = '#3182ce';
      button.style.transform = 'scale(1.1)';
      button.style.boxShadow = '0 6px 16px rgba(64, 148, 239, 0.4)';
    });

    button.addEventListener('mouseleave', () => {
      button.style.background = '#4094ef';
      button.style.transform = 'scale(1)';
      button.style.boxShadow = '0 4px 12px rgba(64, 148, 239, 0.3)';
    });

    return button;
  }

  function createMenuButton(type: 'row' | 'column', index: number, view: EditorView): HTMLElement {
    const button = document.createElement('button');
    button.className = `table-menu-btn table-menu-${type}`;
    button.setAttribute('data-index', index.toString());
    button.innerHTML = '⋯';
    button.style.cssText = `
      position: absolute;
      width: 20px;
      height: 20px;
      background: #6b7280;
      border: 1px solid #ffffff;
      border-radius: 4px;
      color: white;
      font-size: 12px;
      font-weight: bold;
      cursor: pointer;
      display: flex;
      align-items: center;
      justify-content: center;
      pointer-events: auto;
      opacity: 0;
      transition: all 0.2s ease;
      box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
      z-index: 1000;
    `;

    button.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      showContextMenu(e, type, index, view);
    });

    button.addEventListener('mouseenter', () => {
      button.style.background = '#4b5563';
      button.style.transform = 'scale(1.05)';
    });

    button.addEventListener('mouseleave', () => {
      button.style.background = '#6b7280';
      button.style.transform = 'scale(1)';
    });

    return button;
  }

  function showContextMenu(event: MouseEvent, type: 'row' | 'column', index: number, view: EditorView) {
    hideContextMenu();

    const menu = document.createElement('div');
    menu.className = 'table-context-menu';
    menu.style.cssText = `
      position: fixed;
      background: #16171f;
      border: 1px solid #2a2b2f;
      border-radius: 8px;
      box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
      z-index: 1000;
      min-width: 180px;
      padding: 8px 0;
      backdrop-filter: blur(8px);
    `;

    const menuItems = type === 'row'
      ? [
        { label: `Insert Row Above`, action: () => addRowBefore(view.state, view.dispatch) },
        { label: `Insert Row Below`, action: () => addRowAfter(view.state, view.dispatch) },
        { label: 'Delete Row', action: () => deleteRow(view.state, view.dispatch), danger: true }
      ]
      : [
        { label: `Insert Column Left`, action: () => addColumnBefore(view.state, view.dispatch) },
        { label: `Insert Column Right`, action: () => addColumnAfter(view.state, view.dispatch) },
        { label: 'Delete Column', action: () => deleteColumn(view.state, view.dispatch), danger: true }
      ];

    menuItems.forEach((item, idx) => {
      const menuItem = document.createElement('div');
      menuItem.textContent = item.label;
      menuItem.className = 'table-menu-item';
      menuItem.style.cssText = `
        padding: 12px 20px;
        color: ${item.danger ? '#ef4444' : '#bfc0cc'};
        cursor: pointer;
        font-size: 14px;
        font-weight: 500;
        transition: all 0.15s ease;
        display: flex;
        align-items: center;
        gap: 12px;
      `;

      const icon = document.createElement('span');
      icon.style.fontSize = '16px';
      icon.style.opacity = '0.7';

      if (item.label.includes('Above') || item.label.includes('Left')) {
        icon.textContent = '↑';
      } else if (item.label.includes('Below') || item.label.includes('Right')) {
        icon.textContent = '↓';
      } else if (item.label.includes('Delete')) {
        icon.textContent = '🗑';
        icon.style.opacity = '1';
      }

      menuItem.prepend(icon);

      menuItem.addEventListener('mouseenter', () => {
        menuItem.style.background = item.danger ? 'rgba(239, 68, 68, 0.1)' : '#2a2b2f';
        menuItem.style.color = item.danger ? '#f87171' : '#e4e4e7';
      });

      menuItem.addEventListener('mouseleave', () => {
        menuItem.style.background = 'transparent';
        menuItem.style.color = item.danger ? '#ef4444' : '#bfc0cc';
      });

      menuItem.addEventListener('click', () => {
        item.action();
        hideContextMenu();
        view.focus();
      });

      menu.appendChild(menuItem);

      if (idx === 1) {
        const separator = document.createElement('div');
        separator.style.cssText = `
          height: 1px;
          background: #2a2b2f;
          margin: 8px 0;
        `;
        menu.appendChild(separator);
      }
    });

    const rect = (event.target as HTMLElement).getBoundingClientRect();
    menu.style.left = `${rect.left + rect.width + 8}px`;
    menu.style.top = `${rect.top}px`;

    document.body.appendChild(menu);
    const menuRect = menu.getBoundingClientRect();
    if (menuRect.right > window.innerWidth) {
      menu.style.left = `${rect.left - menuRect.width - 8}px`;
    }
    if (menuRect.bottom > window.innerHeight) {
      menu.style.top = `${rect.top - menuRect.height + rect.height}px`;
    }

    state.activeMenu = menu;

    const hideOnClick = (e: MouseEvent) => {
      if (!menu.contains(e.target as Node)) {
        hideContextMenu();
        document.removeEventListener('click', hideOnClick);
      }
    };

    setTimeout(() => {
      document.addEventListener('click', hideOnClick);
    }, 0);

    menu.style.opacity = '0';
    menu.style.transform = 'scale(0.95) translateY(-4px)';
    setTimeout(() => {
      menu.style.transition = 'all 0.15s ease';
      menu.style.opacity = '1';
      menu.style.transform = 'scale(1) translateY(0)';
    }, 0);
  }

  function hideContextMenu() {
    if (state.activeMenu && state.activeMenu.parentNode) {
      state.activeMenu.style.opacity = '0';
      state.activeMenu.style.transform = 'scale(0.95) translateY(-4px)';
      setTimeout(() => {
        if (state.activeMenu && state.activeMenu.parentNode) {
          state.activeMenu.parentNode.removeChild(state.activeMenu);
        }
        state.activeMenu = null;
      }, 150);
    }
  }

  function getTableDimensions(table: HTMLElement): { rows: number; cols: number } {
    const firstRow = table.querySelector('tr');
    const rows = table.querySelectorAll('tr').length;
    const cols = firstRow ? firstRow.querySelectorAll('td, th').length : 0;
    return { rows, cols };
  }

  function getHoveredRowColumn(table: HTMLElement, x: number, y: number): { row: number | null; col: number | null } {
    const tableRect = table.getBoundingClientRect();
    const rows = table.querySelectorAll('tr');

    let hoveredRow: number | null = null;
    let hoveredCol: number | null = null;

    // Check if in row padding area (left side)
    if (x >= tableRect.left - 40 && x <= tableRect.left && y >= tableRect.top && y <= tableRect.bottom) {
      // Find which row
      rows.forEach((row, index) => {
        const rowRect = row.getBoundingClientRect();
        if (y >= rowRect.top && y <= rowRect.bottom) {
          hoveredRow = index;
        }
      });
    }

    // Check if in column padding area (top)
    if (y >= tableRect.top - 40 && y <= tableRect.top && x >= tableRect.left && x <= tableRect.right) {
      // Find which column
      const firstRow = table.querySelector('tr');
      if (firstRow) {
        const cells = firstRow.querySelectorAll('td, th');
        cells.forEach((cell, index) => {
          const cellRect = cell.getBoundingClientRect();
          if (x >= cellRect.left && x <= cellRect.right) {
            hoveredCol = index;
          }
        });
      }
    }

    // Check if in bottom padding area (for add row button)
    if (y >= tableRect.bottom && y <= tableRect.bottom + 40 && x >= tableRect.left && x <= tableRect.right) {
      return { row: -1, col: null }; // Special case for add row
    }

    // Check if in right padding area (for add column button)
    if (x >= tableRect.right && x <= tableRect.right + 40 && y >= tableRect.top && y <= tableRect.bottom) {
      return { row: null, col: -1 }; // Special case for add column
    }

    return { row: hoveredRow, col: hoveredCol };
  }

  function showTableControls(view: EditorView, table: HTMLElement, hoveredRow: number | null, hoveredCol: number | null) {
    if (!state.controls) {
      state.controls = createControls();
      document.body.appendChild(state.controls);
    }

    state.controls.innerHTML = '';

    const tableRect = table.getBoundingClientRect();

    // Show add row button in bottom padding
    if (hoveredRow === -1) {
      const addRowBtn = createAddButton('+', () => {
        addRowAfter(view.state, view.dispatch);
        view.focus();
      }, 'add-row-btn');

      addRowBtn.style.left = `${tableRect.left + (tableRect.width / 2) - 14}px`;
      addRowBtn.style.top = `${tableRect.bottom + 6}px`;
      addRowBtn.title = 'Add row';
      state.controls.appendChild(addRowBtn);
    }

    // Show add column button in right padding
    if (hoveredCol === -1) {
      const addColBtn = createAddButton('+', () => {
        addColumnAfter(view.state, view.dispatch);
        view.focus();
      }, 'add-col-btn');

      addColBtn.style.left = `${tableRect.right + 6}px`;
      addColBtn.style.top = `${tableRect.top + (tableRect.height / 2) - 14}px`;
      addColBtn.title = 'Add column';
      state.controls.appendChild(addColBtn);
    }

    // Show specific row menu button
    if (hoveredRow !== null && hoveredRow >= 0) {
      const rows = table.querySelectorAll('tr');
      const row = rows[hoveredRow];
      if (row) {
        const rowRect = row.getBoundingClientRect();
        const rowMenuBtn = createMenuButton('row', hoveredRow, view);

        rowMenuBtn.style.left = `${tableRect.left - 30}px`;
        rowMenuBtn.style.top = `${rowRect.top + (rowRect.height / 2) - 10}px`;
        rowMenuBtn.title = 'Row options';

        state.controls.appendChild(rowMenuBtn);
      }
    }

    // Show specific column menu button
    if (hoveredCol !== null && hoveredCol >= 0) {
      const firstRow = table.querySelector('tr');
      if (firstRow) {
        const cells = firstRow.querySelectorAll('td, th');
        const cell = cells[hoveredCol];
        if (cell) {
          const cellRect = cell.getBoundingClientRect();
          const colMenuBtn = createMenuButton('column', hoveredCol, view);

          colMenuBtn.style.left = `${cellRect.left + (cellRect.width / 2) - 10}px`;
          colMenuBtn.style.top = `${tableRect.top - 30}px`;
          colMenuBtn.title = 'Column options';

          state.controls.appendChild(colMenuBtn);
        }
      }
    }

    // Show controls with animation
    setTimeout(() => {
      if (state.controls) {
        const buttons = state.controls.querySelectorAll('.table-add-btn, .table-menu-btn');
        buttons.forEach((btn) => {
          (btn as HTMLElement).style.opacity = '1';
          (btn as HTMLElement).style.transform = 'scale(1)';
        });
      }
    }, 50);
  }

  function hideControls() {
    if (state.controls) {
      const buttons = state.controls.querySelectorAll('.table-add-btn, .table-menu-btn');
      buttons.forEach((btn) => {
        (btn as HTMLElement).style.opacity = '0';
        (btn as HTMLElement).style.transform = 'scale(0.8)';
      });

      setTimeout(() => {
        if (state.controls) {
          state.controls.innerHTML = '';
        }
      }, 200);
    }
  }

  function isInTableOrPadding(element: HTMLElement, x: number, y: number): { table: HTMLElement | null; inPadding: boolean } {
    const table = element.closest('table') || element.closest('.tableWrapper')?.querySelector('table');

    if (table) {
      const tableRect = table.getBoundingClientRect();
      const paddingSize = 40;

      // Check if in table or padding area
      const inArea = x >= tableRect.left - paddingSize &&
        x <= tableRect.right + paddingSize &&
        y >= tableRect.top - paddingSize &&
        y <= tableRect.bottom + paddingSize;

      const inTable = x >= tableRect.left && x <= tableRect.right &&
        y >= tableRect.top && y <= tableRect.bottom;

      return { table: inArea ? table : null, inPadding: inArea && !inTable };
    }

    return { table: null, inPadding: false };
  }

  function isInteractingWithControls(element: HTMLElement): boolean {
    return element.closest('.table-hover-controls-enhanced, .table-context-menu') !== null;
  }

  return new Plugin({
    key: tableHoverKey,

    view() {
      return {
        destroy() {
          if (state.controls && state.controls.parentNode) {
            state.controls.parentNode.removeChild(state.controls);
          }
          hideContextMenu();
          state.controls = null;
        }
      };
    },

    props: {
      handleDOMEvents: {
        mousemove(view: EditorView, event: MouseEvent) {
          if (isInteractingWithControls(event.target as HTMLElement)) {
            return false;
          }

          const { table, inPadding } = isInTableOrPadding(event.target as HTMLElement, event.clientX, event.clientY);

          if (table) {
            const { row, col } = getHoveredRowColumn(table, event.clientX, event.clientY);

            if (table !== state.currentTable || row !== state.hoveredRow || col !== state.hoveredColumn) {
              state.currentTable = table;
              state.hoveredRow = row;
              state.hoveredColumn = col;
              showTableControls(view, table, row, col);
            }
          } else {
            if (state.currentTable) {
              state.currentTable = null;
              state.hoveredRow = null;
              state.hoveredColumn = null;
              hideControls();
              hideContextMenu();
            }
          }

          return false;
        },

        mouseleave(view: EditorView, event: MouseEvent) {
          if (!isInteractingWithControls(event.relatedTarget as HTMLElement)) {
            state.currentTable = null;
            state.hoveredRow = null;
            state.hoveredColumn = null;
            hideControls();
            hideContextMenu();
          }
          return false;
        }
      }
    }
  });
}
