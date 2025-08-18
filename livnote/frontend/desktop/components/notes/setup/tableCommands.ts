import { EditorState, Transaction, Selection } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";

/**
 * Creates a 3x3 table with header row
 */
export function createTable(schema: Schema): any {
  const { table, table_row, table_cell, table_header, paragraph } = schema.nodes;

  if (!table || !table_row || !table_cell || !table_header || !paragraph) {
    throw new Error("Table nodes not found in schema");
  }

  // Create header row (3 header cells)
  const headerCells = [];
  for (let col = 0; col < 3; col++) {
    headerCells.push(
      table_header.create(
        {},
        paragraph.create({}, schema.text(`Header ${col + 1}`))
      )
    );
  }
  const headerRow = table_row.create({}, headerCells);

  // Create 2 data rows (3 cells each)
  const dataRows = [];
  for (let row = 0; row < 2; row++) {
    const cells = [];
    for (let col = 0; col < 3; col++) {
      cells.push(
        table_cell.create({}, paragraph.create())
      );
    }
    dataRows.push(table_row.create({}, cells));
  }

  // Create the complete table
  const allRows = [headerRow, ...dataRows];
  return table.create({}, allRows);
}

/**
 * Command to insert a table at the current cursor position
 */
export function insertTable() {
  return (state: EditorState, dispatch?: (tr: Transaction) => void, view?: EditorView) => {
    if (!dispatch) return false;

    const { selection, schema } = state;
    const table = createTable(schema);

    // Insert the table at current selection
    const tr = state.tr.replaceSelectionWith(table);
    dispatch(tr);

    // Focus the first cell after insertion
    if (view) {
      // Wait for next tick to ensure table is rendered
      setTimeout(() => {
        const newState = view.state;
        const tablePos = tr.mapping.map(selection.from);

        // Find the first cell position (header cell)
        const resolvedPos = newState.doc.resolve(tablePos);
        let cellPos = tablePos + 1; // Start of table content

        // Navigate to first cell content
        try {
          const firstRow = resolvedPos.nodeAfter?.child(0); // First row
          if (firstRow) {
            const firstCell = firstRow.child(0); // First cell
            if (firstCell) {
              cellPos += 2; // Position inside first cell
              const cellSelection = view.state.tr.setSelection(
                view.state.selection.constructor.near(view.state.doc.resolve(cellPos))
              );
              view.dispatch(cellSelection);
              view.focus();
            }
          }
        } catch (error) {
          // Fallback: just focus the view
          view.focus();
        }
      }, 10);
    }

    return true;
  };
}

/**
 * Check if cursor is currently inside a table
 */
export function isInTable(state: EditorState): boolean {
  const { $from } = state.selection;

  for (let depth = $from.depth; depth > 0; depth--) {
    const node = $from.node(depth);
    if (node.type.spec.tableRole === "table") {
      return true;
    }
  }

  return false;
}

/**
 * Get information about the current table (if cursor is in one)
 */
export function getTableInfo(state: EditorState) {
  const { $from } = state.selection;

  for (let depth = $from.depth; depth > 0; depth--) {
    const node = $from.node(depth);
    if (node.type.spec.tableRole === "table") {
      try {
        const tablePos = $from.before(depth);
        const cellPos = $from.before(depth - 1);
        
        // Validate positions are within bounds
        if (tablePos >= 0 && cellPos >= 0) {
          return {
            table: node,
            tablePos: tablePos,
            depth: depth,
            cell: $from.node(depth - 1), // The cell containing cursor
            cellPos: cellPos
          };
        }
      } catch (error) {
        // If we can't get valid positions, continue to next depth
        continue;
      }
    }
  }

  return null;
}

/**
 * Delete entire table command
 * This ensures the complete table is removed, not just cells
 */
export function deleteTable(state: EditorState, dispatch?: (tr: Transaction) => void): boolean {
  const tableInfo = getTableInfo(state);

  if (!tableInfo) return false;

  if (dispatch) {
    const { tablePos, table } = tableInfo;
    const tr = state.tr;

    // Delete the entire table node
    tr.delete(tablePos, tablePos + table.nodeSize);

    // Ensure we have valid content after deletion
    // If document becomes empty, add a paragraph
    if (tr.doc.content.size === 0) {
      const paragraph = state.schema.nodes.paragraph.create();
      tr.insert(0, paragraph);
      tr.setSelection(Selection.near(tr.doc.resolve(1)));
    } else {
      // Place cursor after deletion
      try {
        const $pos = tr.doc.resolve(Math.min(tablePos, tr.doc.content.size - 1));
        tr.setSelection(Selection.near($pos));
      } catch (e) {
        // Fallback to start of document
        tr.setSelection(Selection.near(tr.doc.resolve(1)));
      }
    }

    dispatch(tr);
  }

  return true;
}

/**
 * Check if the table has only one cell remaining
 */
export function isLastCellInTable(state: EditorState): boolean {
  const tableInfo = getTableInfo(state);
  if (!tableInfo) return false;

  const { table } = tableInfo;
  let cellCount = 0;

  table.forEach((row: any) => {
    row.forEach(() => {
      cellCount++;
    });
  });

  return cellCount === 1;
}

/**
 * Enhanced delete backward command that handles table deletion
 */
export function deleteBackwardEnhanced(state: EditorState, dispatch?: (tr: Transaction) => void): boolean {
  const { $from, empty } = state.selection;

  // Check if we're at the start of a cell in a table with only one cell
  if (empty && isInTable(state)) {
    const tableInfo = getTableInfo(state);

    if (tableInfo) {
      // Check if cursor is at the very start of the cell content
      const cellStart = $from.start($from.depth);
      const cursorAtCellStart = $from.pos === cellStart;

      // If we're at the start of the last cell and it's empty, delete the table
      if (cursorAtCellStart && isLastCellInTable(state)) {
        const cell = $from.parent;
        if (cell.content.size === 0 ||
          (cell.content.size === 2 && cell.firstChild?.type.name === 'paragraph' && cell.firstChild.content.size === 0)) {
          return deleteTable(state, dispatch);
        }
      }
    }
  }

  return false;
}

/**
 * Enhanced select all that can select entire table
 */
export function selectTable(state: EditorState, dispatch?: (tr: Transaction) => void): boolean {
  const tableInfo = getTableInfo(state);

  if (!tableInfo) return false;

  if (dispatch) {
    const { tablePos, table } = tableInfo;
    const tr = state.tr;

    // Create a node selection for the entire table
    const resolvedPos = state.doc.resolve(tablePos);
    const selection = state.selection.constructor.create(
      state.doc,
      tablePos,
      tablePos + table.nodeSize
    );

    tr.setSelection(selection);
    dispatch(tr);
  }

  return true;
}
