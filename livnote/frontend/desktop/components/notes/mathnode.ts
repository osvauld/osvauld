import type { NodeSpec } from "prosemirror-model";

/**
 * Math node specifications based on prosemirror-math library requirements
 * 
 * IMPORTANT: These specs follow the library's required structure.
 * Do not modify the marked properties or the library won't work correctly!
 */

/**
 * Inline math node - appears within text flow
 * Example: The equation $x^2 + y^2 = r^2$ describes a circle.
 */
export const mathInlineSpec: NodeSpec = {
  group: "inline math",
  content: "text*",        // important!
  inline: true,            // important!
  atom: true,              // important!
  toDOM: () => ["math-inline", { class: "math-node" }, 0],
  parseDOM: [{
    tag: "math-inline"   // important!
  }]
};

/**
 * Display math node - appears on its own line, centered
 * Example:
 *   ∫₀¹ x² dx = 1/3
 * 
 * Extended with label and number attributes for equation numbering
 */
export const mathDisplaySpec: NodeSpec = {
  group: "block math",
  content: "text*",        // important!
  atom: true,              // important!
  code: true,              // important!
  attrs: {
    // Custom attributes for equation numbering
    label: { default: null },    // Optional label for referencing (e.g., "eq:pythagorean")
    number: { default: null }     // Computed equation number (managed by numbering plugin)
  },
  toDOM: (node) => {
    const attrs: any = { class: "math-node" };

    if (node.attrs.label) {
      attrs["data-math-label"] = node.attrs.label;
    }

    if (node.attrs.number) {
      attrs["data-math-number"] = node.attrs.number;
    }

    return ["math-display", attrs, 0];
  },
  parseDOM: [{
    tag: "math-display",  // important!
    getAttrs(dom: HTMLElement) {
      return {
        label: dom.getAttribute("data-math-label") || null,
        number: dom.getAttribute("data-math-number") || null
      };
    }
  }]
};
