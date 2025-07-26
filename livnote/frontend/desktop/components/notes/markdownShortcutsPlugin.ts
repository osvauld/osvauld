import { Plugin, TextSelection } from "prosemirror-state";
import { inputRules, wrappingInputRule, textblockTypeInputRule, InputRule } from "prosemirror-inputrules";
import type { Schema, MarkType } from "prosemirror-model";

const headingRule = (level: number, schema: Schema) => {
  return textblockTypeInputRule(
    new RegExp(`^(#{${level}})\\s$`),
    schema.nodes.heading,
    { level }
  );
};

const blockquoteRule = (schema: Schema) => {
  return wrappingInputRule(
    /^\s*>\s$/,
    schema.nodes.blockquote
  );
};

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

const inlineMarkRule = (markType: MarkType, regex: RegExp) => {
  return new InputRule(regex, (state, match, start, end) => {
    const content = match[1];
    if (!content) return null;

    const mark = markType.create();
    const textNode = state.schema.text(content, [mark]);

    return state.tr.replaceWith(start, end, textNode);
  });
};

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

const codeBlockRule = (schema: Schema) => {
  return textblockTypeInputRule(
    /^```$/,
    schema.nodes.code_block
  );
};

const horizontalRuleRule = (schema: Schema) => {
  return new InputRule(
    /^[-*_]{3,}$/, // Matches 3 or more -, *, or _ characters at the start of a line
    (state, match, start, end) => {
      const { tr } = state;

      const horizontalRule = schema.nodes.horizontal_rule.create();

      tr.replaceWith(start, end, horizontalRule);

      const newParagraph = schema.nodes.paragraph.create();
      tr.insert(end, newParagraph);

      return tr.setSelection(TextSelection.create(tr.doc, end + 1));
    }
  );
};

export const markdownShortcutsPlugin = (schema: Schema) => {
  const rules = [
    // Heading rules
    headingRule(1, schema),
    headingRule(2, schema),
    headingRule(3, schema),
    blockquoteRule(schema),
    bulletListRule(schema),
    orderedListRule(schema),
    inlineMarkRule(schema.marks.strong, /\*\*([^\*]+)\*\*$/),      // **text**
    inlineMarkRule(schema.marks.strong, /__([^_]+)__$/),          // __text__
    inlineMarkRule(schema.marks.em, /(?<!\*)\*([^\*]+)\*(?!\*)$/),    // *text*
    inlineMarkRule(schema.marks.em, /(?<!_)_([^_]+)_(?!_)$/),      // _text_
    inlineMarkRule(schema.marks.strikethrough, /~~([^~]+)~~$/),    // ~~text~~
    inlineCodeInputRule(schema),                                  // ``code``
    codeBlockRule(schema),                                       // ```
    horizontalRuleRule(schema),                                  // ---, ***, or ___
  ];

  const markdownInputRules = inputRules({ rules });

  return new Plugin({
    ...markdownInputRules,
    props: {
      ...markdownInputRules.props,
      handleKeyDown: (view, event) => {
        if (event.key === 'Enter' && view.state.selection.$head.parent.type === schema.nodes.code_block) {
          const { state, dispatch } = view;
          const tr = state.tr.insertText('\n');
          dispatch(tr);
          return true; // Handled: Insert newline in code block
        }
        return false; // Not handled by this specific keydown logic
      }
    }
  });
}; 
