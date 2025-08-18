import { Plugin } from "prosemirror-state";
import {
  tableEditing,
  columnResizing,
  goToNextCell,
  addColumnAfter,
  addColumnBefore,
  deleteColumn,
  addRowAfter,
  addRowBefore,
  deleteRow,
  mergeCells,
  splitCell,
  toggleHeaderRow,
  toggleHeaderColumn
} from "prosemirror-tables";
import { keymap } from "prosemirror-keymap";
import { Schema } from "prosemirror-model";
import { tableContextMenuPlugin } from "./tableContextMenuPlugin";
import { deleteTable, deleteBackwardEnhanced, selectTable, isInTable, isLastCellInTable, getTableInfo } from "./tableCommands";

/**
 * Creates the table editing plugin with keyboard shortcuts
 */
export function createTableKeymap(schema: Schema): Plugin {
  return keymap({
    // Tab to next cell
    "Tab": goToNextCell(1),
    // Shift+Tab to previous cell
    "Shift-Tab": goToNextCell(-1),

    // Row operations
    "Mod-Alt-Up": addRowBefore,
    "Mod-Alt-Down": addRowAfter,
    "Mod-Shift-Delete": (state, dispatch) => {
      // Try to delete row first
      if (deleteRow(state, dispatch)) {
        return true;
      }
      // If that fails (last row), delete the entire table
      return deleteTable(state, dispatch);
    },

    // Column operations  
    "Mod-Alt-Left": addColumnBefore,
    "Mod-Alt-Right": addColumnAfter,
    "Mod-Alt-Delete": (state, dispatch) => {
      // Try to delete column first
      if (deleteColumn(state, dispatch)) {
        return true;
      }
      // If that fails (last column), delete the entire table
      return deleteTable(state, dispatch);
    },

    // Cell operations
    "Mod-Shift-m": mergeCells,
    "Mod-Shift-s": splitCell,

    // Header toggles
    "Mod-Shift-h": toggleHeaderRow,
    "Mod-Shift-j": toggleHeaderColumn,

    // Delete entire table
    "Mod-Shift-Backspace": deleteTable,

    // Enhanced backspace handling for table deletion
    "Backspace": (state, dispatch, view) => {
      // First try our enhanced delete backward
      if (deleteBackwardEnhanced(state, dispatch)) {
        return true;
      }

      // If we're in a table and at the beginning of a cell
      if (isInTable(state)) {
        const { $from, empty } = state.selection;

        if (empty) {
          // Check if we're at the very start of the cell
          const cellStart = $from.start($from.depth);

          if ($from.pos === cellStart) {
            // Check if this is the only cell with no content
            if (isLastCellInTable(state)) {
              const cell = $from.parent;

              // If cell is empty or contains only an empty paragraph
              if (cell.content.size === 0 ||
                (cell.firstChild?.type.name === 'paragraph' &&
                  cell.firstChild.content.size === 0)) {
                return deleteTable(state, dispatch);
              }
            }
          }
        }
      }

      // Let default backspace behavior handle it
      return false;
    },

    // Delete key handling
    "Delete": (state, dispatch) => {
      // If entire table is selected, delete it
      const { from, to } = state.selection;
      const tableInfo = getTableInfo(state);

      if (tableInfo) {
        const { tablePos, table } = tableInfo;

        // Check if entire table is selected
        if (from <= tablePos && to >= tablePos + table.nodeSize) {
          return deleteTable(state, dispatch);
        }
      }

      return false;
    },

    // Select entire table
    "Mod-a": (state, dispatch, view) => {
      if (isInTable(state)) {
        // First Cmd+A selects current cell content
        // Second Cmd+A selects entire table
        const { $from } = state.selection;
        const cellStart = $from.start($from.depth);
        const cellEnd = $from.end($from.depth);

        // Check if current cell is already fully selected
        if (state.selection.from === cellStart && state.selection.to === cellEnd) {
          // Select the entire table
          return selectTable(state, dispatch);
        }
      }

      return false;
    }
  });
}


/**
 * Creates all table-related plugins
 */
export function createTablePlugins(schema: Schema): Plugin[] {
  return [
    // Core table editing functionality
    tableEditing(),

    // Column resizing by dragging
    columnResizing({
      handleWidth: 5,
      cellMinWidth: 50,
      lastColumnResizable: true,
      View: undefined
    }),

    // Keyboard shortcuts for table operations
    createTableKeymap(schema),

    // Context menu for table operations
    tableContextMenuPlugin()
  ];
}

/**
 * Main function to get all table plugins
 * Use this in your plugin list
 */
export function getTablePlugins(schema: Schema): Plugin[] {
  return createTablePlugins(schema);
}
