import {
  mathPlugin,
  mathBackspaceCmd,
  makeBlockMathInputRule,
  makeInlineMathInputRule,
  REGEX_INLINE_MATH_DOLLARS,
  REGEX_BLOCK_MATH_DOLLARS
} from "@benrbray/prosemirror-math";
import { Schema } from "prosemirror-model";
import { mathNumberingPlugin } from "./mathNumbering";
import { inputRules } from "prosemirror-inputrules";
import type { Plugin, EditorState, Transaction } from "prosemirror-state";

/**
 * Create math plugins for the editor
 */
export function createMathPlugins(schema: Schema): Plugin[] {
  if (!schema.nodes.math_inline || !schema.nodes.math_display) {
    console.error("Math nodes not found in schema!");
    return [];
  }

  const inlineMathInputRule = makeInlineMathInputRule(
    REGEX_INLINE_MATH_DOLLARS,
    schema.nodes.math_inline
  );

  const blockMathInputRule = makeBlockMathInputRule(
    REGEX_BLOCK_MATH_DOLLARS,
    schema.nodes.math_display
  );

  return [
    mathPlugin,
    inputRules({
      rules: [inlineMathInputRule, blockMathInputRule]
    }),
    mathNumberingPlugin()
  ];
}

export { mathBackspaceCmd };

/**
 * Insert inline math with placeholder text
 * User needs to click it to start editing
 */
export const insertInlineMath = (schema: Schema) => {
  return (state: EditorState, dispatch?: (tr: Transaction) => void) => {
    const mathNode = schema.nodes.math_inline.create(null, schema.text("x"));

    if (dispatch) {
      const tr = state.tr.replaceSelectionWith(mathNode);
      dispatch(tr);
    }

    return true;
  };
};

/**
 * Insert display math block with placeholder text
 * User needs to click it to start editing
 */
export const insertDisplayMath = (schema: Schema) => {
  return (state: EditorState, dispatch?: (tr: Transaction) => void) => {
    const { $from } = state.selection;

    if ($from.depth < 1) {
      return false;
    }

    if (dispatch) {
      const mathNode = schema.nodes.math_display.create(null, schema.text("x^2 + y^2 = r^2"));

      const blockStart = $from.before($from.depth);
      const blockEnd = $from.after($from.depth);
      const tr = state.tr.replaceRangeWith(blockStart, blockEnd, mathNode);

      dispatch(tr);
    }

    return true;
  };
};
