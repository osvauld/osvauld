import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";
import type { Node as ProseMirrorNode } from "prosemirror-model";

export const mathNumberingKey = new PluginKey("mathNumbering");

interface EquationInfo {
  pos: number;
  label: string;
  number: number;
}

/**
 * Plugin that automatically numbers labeled display math equations
 * 
 * - Scans document for math_display nodes with labels
 * - Assigns sequential numbers based on document order
 * - Updates node attributes with computed numbers
 * - Maintains a map of label -> number for references
 */
export function mathNumberingPlugin() {
  return new Plugin({
    key: mathNumberingKey,

    state: {
      init(config, state) {
        return computeEquationNumbers(state.doc);
      },

      apply(tr, value, oldState, newState) {
        // Recompute on document changes
        if (tr.docChanged) {
          return computeEquationNumbers(newState.doc);
        }
        return value;
      }
    },

    appendTransaction(transactions, oldState, newState) {
      // Check if equation numbering changed
      const oldNumbers = mathNumberingKey.getState(oldState);
      const newNumbers = mathNumberingKey.getState(newState);

      if (!oldNumbers || !newNumbers) return null;

      // Compare equation numbers
      const needsUpdate = hasNumberingChanged(oldNumbers, newNumbers);

      if (!needsUpdate) return null;

      // Create transaction to update node attributes with new numbers
      const tr = newState.tr;
      let updated = false;

      newNumbers.equations.forEach((eq) => {
        const node = newState.doc.nodeAt(eq.pos);
        if (node && node.type.name === "math_display") {
          // Only update if number actually changed
          if (node.attrs.number !== eq.number) {
            tr.setNodeMarkup(eq.pos, undefined, {
              ...node.attrs,
              number: eq.number
            });
            updated = true;
          }
        }
      });

      return updated ? tr : null;
    },

    props: {
      decorations(state) {
        const equations = mathNumberingKey.getState(state);
        if (!equations) return null;

        const decorations: Decoration[] = [];

        // Add decorations for equation numbers (visual only, no DOM changes needed)
        // The actual rendering is handled by the math node view

        return DecorationSet.create(state.doc, decorations);
      }
    }
  });
}

/**
 * Scan document and compute equation numbers for labeled equations
 */
function computeEquationNumbers(doc: ProseMirrorNode): {
  equations: EquationInfo[];
  labelMap: Map<string, number>;
} {
  const equations: EquationInfo[] = [];
  const labelMap = new Map<string, number>();
  let equationNumber = 1;

  doc.descendants((node, pos) => {
    if (node.type.name === "math_display" && node.attrs.label) {
      const label = node.attrs.label;

      equations.push({
        pos,
        label,
        number: equationNumber
      });

      labelMap.set(label, equationNumber);
      equationNumber++;
    }
  });

  return { equations, labelMap };
}

/**
 * Check if equation numbering has changed between states
 */
function hasNumberingChanged(
  oldNumbers: { equations: EquationInfo[]; labelMap: Map<string, number> },
  newNumbers: { equations: EquationInfo[]; labelMap: Map<string, number> }
): boolean {
  if (oldNumbers.equations.length !== newNumbers.equations.length) {
    return true;
  }

  for (let i = 0; i < oldNumbers.equations.length; i++) {
    const oldEq = oldNumbers.equations[i];
    const newEq = newNumbers.equations[i];

    if (oldEq.label !== newEq.label || oldEq.number !== newEq.number) {
      return true;
    }
  }

  return false;
}

/**
 * Get equation number by label
 * Useful for implementing equation references
 */
export function getEquationNumber(state: any, label: string): number | null {
  const numbers = mathNumberingKey.getState(state);
  if (!numbers) return null;

  return numbers.labelMap.get(label) || null;
}
