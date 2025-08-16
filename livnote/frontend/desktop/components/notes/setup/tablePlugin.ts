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
import { tableHoverPlugin } from "./tableHoverControls";
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
    "Mod-Shift-=": addRowAfter,
    "Mod-Shift--": deleteRow,

    // Column operations  
    "Mod-Shift-\\": addColumnAfter,
    "Mod-Shift-Backspace": deleteColumn,

    // Cell operations
    "Mod-Shift-m": mergeCells,
    "Mod-Shift-s": splitCell,

    // Header toggles
    "Mod-Shift-h": toggleHeaderRow,
    "Mod-Shift-j": toggleHeaderColumn,
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
      lastColumnResizable: true
    }),

    // Keyboard shortcuts for table operations
    createTableKeymap(schema),
    tableHoverPlugin()
  ];
}

/**
 * Table utilities plugin for additional functionality
 */
export function tableUtilsPlugin(): Plugin {
  return new Plugin({
    props: {
      // Handle clicks on table elements
      handleClick(view, pos, event) {
        // Future: Add click handlers for table controls
        return false;
      },

      // Handle DOM events
      handleDOMEvents: {
        // Future: Add hover handlers for table controls
        mouseover(view, event) {
          // Placeholder for hover controls
          return false;
        },

        mouseout(view, event) {
          // Placeholder for hover controls cleanup
          return false;
        }
      }
    },

    // Plugin state for tracking table interactions
    state: {
      init() {
        return {
          hoveredTable: null,
          hoveredCell: null
        };
      },

      apply(tr, prev) {
        // Future: Track table hover state changes
        return prev;
      }
    }
  });
}

/**
 * Main function to get all table plugins
 * Use this in your plugin list
 */
export function getTablePlugins(schema: Schema): Plugin[] {
  return createTablePlugins(schema);
}
