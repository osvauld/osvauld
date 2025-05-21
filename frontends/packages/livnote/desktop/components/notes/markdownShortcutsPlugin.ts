import { Plugin } from "prosemirror-state";
import { inputRules, wrappingInputRule, textblockTypeInputRule } from "prosemirror-inputrules";
import type { Schema } from "prosemirror-model";

// Helper function to create heading input rules
const headingRule = (level: number, schema: Schema) => {
  return textblockTypeInputRule(
    new RegExp(`^(#{${level}})\\s$`),
    schema.nodes.heading,
    { level }
  );
};

// Blockquote input rule
const blockquoteRule = (schema: Schema) => {
  return wrappingInputRule(
    /^\s*>\s$/,
    schema.nodes.blockquote
  );
};

// List input rules
const bulletListRule = (schema: Schema) => {
  return wrappingInputRule(
    /^\s*[-*]\s$/,
    schema.nodes.bullet_list
  );
};

const orderedListRule = (schema: Schema) => {
  return wrappingInputRule(
    /^\s*\d+\.\s$/,
    schema.nodes.ordered_list
  );
};

// Create the markdown shortcuts plugin
export const markdownShortcutsPlugin = (schema: Schema) => {
  const rules = [
    // Heading rules
    headingRule(1, schema), // # Heading
    headingRule(2, schema), // ## Heading
    headingRule(3, schema), // ### Heading
    // Blockquote rule
    blockquoteRule(schema),
    // List rules
    bulletListRule(schema),    // - or * for bullet lists
    orderedListRule(schema),   // 1. for ordered lists
  ];

  const markdownInputRules = inputRules({ rules });

  return new Plugin({
    ...markdownInputRules,
    props: {
      ...markdownInputRules.props,
      handleKeyDown: (view, event) => {
        return false;
      }
    }
  });
}; 