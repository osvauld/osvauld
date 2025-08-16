import { EditorState, Transaction } from "prosemirror-state";
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
      return {
        table: node,
        tablePos: $from.before(depth),
        depth: depth,
        cell: $from.node(depth - 1), // The cell containing cursor
        cellPos: $from.before(depth - 1)
      };
    }
  }

  return null;
}
