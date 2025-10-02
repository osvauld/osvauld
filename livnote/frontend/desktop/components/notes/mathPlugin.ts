import {
  mathPlugin,
  mathBackspaceCmd,
  insertMathCmd,
  makeBlockMathInputRule,
  makeInlineMathInputRule,
  REGEX_INLINE_MATH_DOLLARS,
  REGEX_BLOCK_MATH_DOLLARS
} from "@benrbray/prosemirror-math";
import { Schema } from "prosemirror-model";
import { mathNumberingPlugin } from "./mathNumbering";
import { inputRules } from "prosemirror-inputrules";
import type { Plugin } from "prosemirror-state";

/**
 * Create math plugins for the editor
 * 
 * Returns an array of plugins:
 * 1. The main math plugin from prosemirror-math (handles rendering and editing)
 * 2. Input rules plugin (handles $...$ and $$...$$ auto-conversion)
 * 3. Our custom numbering plugin (handles equation numbering)
 */
export function createMathPlugins(schema: Schema): Plugin[] {
  if (!schema.nodes.math_inline || !schema.nodes.math_display) {
    console.error("Math nodes not found in schema!");
    return [];
  }

  // Create input rules
  const inlineMathInputRule = makeInlineMathInputRule(
    REGEX_INLINE_MATH_DOLLARS,
    schema.nodes.math_inline
  );

  const blockMathInputRule = makeBlockMathInputRule(
    REGEX_BLOCK_MATH_DOLLARS,
    schema.nodes.math_display
  );

  return [
    // Main math plugin - this includes NodeViews internally
    mathPlugin,

    // Input rules for $...$ and $$...$$
    inputRules({
      rules: [inlineMathInputRule, blockMathInputRule]
    }),

    // Equation numbering plugin
    mathNumberingPlugin()
  ];
}

export { mathBackspaceCmd };

export const insertInlineMath = (schema: Schema) => insertMathCmd(schema.nodes.math_inline);

export const insertDisplayMath = (schema: Schema) => insertMathCmd(schema.nodes.math_display);
