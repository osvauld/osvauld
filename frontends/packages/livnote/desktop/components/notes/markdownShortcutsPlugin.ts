import { Plugin, TextSelection } from "prosemirror-state";
import { inputRules, wrappingInputRule, textblockTypeInputRule, InputRule } from "prosemirror-inputrules";
import type { Schema, MarkType } from "prosemirror-model";

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

// Helper for creating generic inline mark input rules (bold, italic, strikethrough)
const inlineMarkRule = (markType: MarkType, regex: RegExp) => {
  return new InputRule(regex, (state, match, start, end) => {
    const content = match[1];
    if (!content) return null;

    const mark = markType.create();
    const textNode = state.schema.text(content, [mark]);
    
    return state.tr.replaceWith(start, end, textNode);
  });
};

// Specific input rule for inline code to handle trimming and cursor position
const inlineCodeInputRule = (schema: Schema) => {
  return new InputRule(
    /``([^`]+)``$/, // User updated Regex: matches ``content``
    (state, match, start, end) => {
      const fullMatchContent = match[1]; 
      if (!fullMatchContent) return null;

      const trimmedContent = fullMatchContent.trim();
      if (!trimmedContent) return null;

      const mark = schema.marks.code.create();
      const textNode = schema.text(trimmedContent, [mark]);
      
      const tr = state.tr.replaceWith(start, end, textNode);
      const newPos = start + trimmedContent.length;
      
      // Set selection after the inserted code and clear stored marks
      return tr.setSelection(TextSelection.create(tr.doc, newPos)).setStoredMarks([]);
    }
  );
};

// Create the markdown shortcuts plugin
export const markdownShortcutsPlugin = (schema: Schema) => {
  const rules = [
    // Heading rules
    headingRule(1, schema),
    headingRule(2, schema),
    headingRule(3, schema),
    // Blockquote rule
    blockquoteRule(schema),
    // List rules
    bulletListRule(schema),
    orderedListRule(schema),
    // Inline formatting rules 
    inlineMarkRule(schema.marks.strong, /\*\*([^\*]+)\*\*$/),      // **text**
    inlineMarkRule(schema.marks.strong, /__([^_]+)__$/),          // __text__
    inlineMarkRule(schema.marks.em, /(?<!\*)\*([^\*]+)\*(?!\*)$/),    // *text*
    inlineMarkRule(schema.marks.em, /(?<!_)_([^_]+)_(?!_)$/),      // _text_
    inlineMarkRule(schema.marks.strikethrough, /~~([^~]+)~~$/),    // ~~text~~
    // Use specific rule for inline code
    inlineCodeInputRule(schema),                                  // ``code``
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