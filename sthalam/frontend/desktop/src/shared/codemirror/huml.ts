/**
 * HUML Language Support for CodeMirror 6
 * Enhanced with Sthalam-specific syntax highlighting and code folding
 */

import { StreamLanguage, foldService } from '@codemirror/language';
import { LanguageSupport } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';

// Patterns
const keywords = /^(true|false|null|True|False|Null|TRUE|FALSE|NULL)$/;
const numberRegex = /^-?(?:0(?:[xX][0-9a-fA-F_]+|[oO][0-7_]+|[bB][01_]+)|[0-9_]+(?:\.[0-9_]+)?(?:[eE][+-]?[0-9_]+)?)/;

// Sthalam-specific keywords with semantic meaning
const blockTypes = new Set([
  'screen-container', 'section-container', 'modal',
  'heading', 'text', 'markdown-text', 'image',
  'nav-button', 'form',
  'form-field-text', 'form-field-email', 'form-field-tel',
  'form-field-password', 'form-field-number', 'form-field-textarea',
  'form-field-checkbox', 'form-field-select',
  'thread'
]);

const actionKeywords = new Set([
  'setState', 'navigate', 'submit'
]);

const targetKeywords = new Set([
  'state', 'content', 'user_content', 'ui'
]);

const resourceTypes = new Set([
  'website', 'form', 'dashboard'
]);

const modeKeywords = new Set([
  'markdown', 'html', 'plain'
]);

const specialProperties = new Set([
  'type', 'id', 'name', 'css', 'visible', 'uiStateRef',
  'action', 'target', 'stateKey', 'stateValue', 'stateUpdates',
  'targetContainerId', 'formId', 'fieldName', 'required',
  'label', 'placeholder', 'forEach', 'forEachAs', 'isEntryPoint',
  'src', 'alt', 'mode', 'eventName', 'submitEndpoint',
  'description', 'value', 'options', 'blocks', 'children',
  'screens', 'computed', 'uiStateTree', 'resourceType', 'submit',
  'targetScreen', 'targetContainer', 'isModal'
]);

// Container types for folding
const containerTypes = new Set([
  'screen-container', 'section-container', 'modal', 'form'
]);

export const humlLanguage = StreamLanguage.define({
  name: 'huml',

  startState() {
    return {
      inMultilineString: null as null | string,
      multilineStringIndent: 0,
      expectValue: false,
      lastKey: null as string | null,
      indentStack: [] as number[],
    };
  },

  token(stream, state) {
    // Handle multiline strings
    if (state.inMultilineString) {
      const closeDelim = state.inMultilineString;
      const content = stream.string.slice(stream.pos);
      const closeIndex = content.indexOf(closeDelim);

      if (closeIndex >= 0) {
        stream.pos += closeIndex + closeDelim.length;
        state.inMultilineString = null;
        state.multilineStringIndent = 0;
        return 'string';
      } else {
        stream.skipToEnd();
        return 'string';
      }
    }

    // Handle start of line
    if (stream.sol()) {
      state.expectValue = false;
    }

    // Skip whitespace
    if (stream.eatSpace()) return null;

    // Comments
    if (stream.match(/^#.*/)) {
      return 'comment';
    }

    // Multiline string starters - """
    if (stream.match('"""')) {
      const delimiter = '"""';
      state.inMultilineString = delimiter;
      state.multilineStringIndent = stream.indentation();

      const remaining = stream.string.slice(stream.pos);
      const closeIndex = remaining.indexOf(delimiter);

      if (closeIndex >= 0) {
        stream.pos += closeIndex + delimiter.length;
        state.inMultilineString = null;
        state.multilineStringIndent = 0;
      } else {
        stream.skipToEnd();
      }
      return 'string';
    }

    // Multiline string starters - ```
    if (stream.match('```')) {
      const delimiter = '```';
      state.inMultilineString = delimiter;
      state.multilineStringIndent = stream.indentation();

      const remaining = stream.string.slice(stream.pos);
      const closeIndex = remaining.indexOf(delimiter);

      if (closeIndex >= 0) {
        stream.pos += closeIndex + delimiter.length;
        state.inMultilineString = null;
        state.multilineStringIndent = 0;
      } else {
        stream.skipToEnd();
      }
      return 'string';
    }

    // Keys (before expecting value)
    if (!state.expectValue) {
      // Quoted key
      if (stream.match(/^"[^"]+"\s*:/)) {
        state.expectValue = true;
        return 'propertyName';
      }

      // Key with :: (vector/nested)
      const vectorKeyMatch = stream.match(/^([a-zA-Z_][a-zA-Z0-9_-]*)::/);
      if (vectorKeyMatch && Array.isArray(vectorKeyMatch)) {
        state.expectValue = true;
        const keyName = vectorKeyMatch[1];
        state.lastKey = keyName;

        // Special styling for top-level mode sections
        if (keyName === 'publisher' || keyName === 'viewer') {
          return 'keyword strong emphasis';
        }
        // Special styling for important top-level sections
        if (keyName === 'content' || keyName === 'state' || keyName === 'computed') {
          return 'keyword strong';
        }
        // Special styling for important properties
        if (keyName === 'type') {
          return 'keyword';
        }
        if (keyName === 'blocks' || keyName === 'children' || keyName === 'screens') {
          return 'keyword strong';
        }
        if (keyName === 'css') {
          return 'propertyName meta';
        }
        if (specialProperties.has(keyName)) {
          return 'propertyName strong';
        }
        return 'propertyName strong';
      }

      // Regular key with :
      const scalarKeyMatch = stream.match(/^([a-zA-Z_][a-zA-Z0-9_-]*):/);
      if (scalarKeyMatch && Array.isArray(scalarKeyMatch)) {
        state.expectValue = true;
        const keyName = scalarKeyMatch[1];
        state.lastKey = keyName;

        // Special styling for important properties
        if (keyName === 'type') {
          return 'keyword';
        }
        if (keyName === 'name' || keyName === 'id') {
          return 'propertyName strong';
        }
        if (keyName === 'css') {
          return 'propertyName meta';
        }
        if (keyName === 'action' || keyName === 'target') {
          return 'keyword';
        }
        if (specialProperties.has(keyName)) {
          return 'propertyName strong';
        }
        return 'propertyName';
      }
    }

    // List item marker
    if (stream.match(/^-\s+/)) {
      state.expectValue = true;
      return 'punctuation';
    }

    // Values
    // Quoted strings with CEL expressions
    const quotedString = stream.match(/^"([^"\\]|\\.)*"/);
    if (quotedString && Array.isArray(quotedString)) {
      const str = quotedString[0];
      // Check if it contains CEL expression markers {{...}}
      if (str.includes('{{') && str.includes('}}')) {
        return 'string special'; // Special highlighting for CEL strings
      }
      return 'string';
    }

    if (stream.match(/^'([^'\\]|\\.)*'/)) {
      return 'string';
    }

    // Numbers
    if (stream.match(numberRegex)) {
      return 'number';
    }

    // Keywords and identifiers (values)
    const word = stream.match(/^[a-zA-Z_][a-zA-Z0-9_-]*/);
    if (word && Array.isArray(word)) {
      const wordStr = word[0];

      // Boolean/null keywords
      if (keywords.test(wordStr)) {
        return 'atom';
      }

      // Context-aware highlighting based on last key
      if (state.lastKey === 'type' && blockTypes.has(wordStr)) {
        // Block types - use className for distinctive color
        if (containerTypes.has(wordStr)) {
          return 'type variableName.special'; // Containers get special color
        }
        if (wordStr.startsWith('form')) {
          return 'type strong'; // Form-related types
        }
        return 'type'; // Other block types
      }

      if (state.lastKey === 'action' && actionKeywords.has(wordStr)) {
        return 'keyword';
      }

      if (state.lastKey === 'target' && targetKeywords.has(wordStr)) {
        return 'keyword strong';
      }

      if (state.lastKey === 'resourceType' && resourceTypes.has(wordStr)) {
        return 'type';
      }

      if (state.lastKey === 'mode' && modeKeywords.has(wordStr)) {
        return 'keyword';
      }

      // Default identifier
      return 'variableName';
    }

    // Inline collection brackets
    if (stream.eat('[') || stream.eat('{')) {
      return 'bracket';
    }
    if (stream.eat(']') || stream.eat('}')) {
      return 'bracket';
    }

    // Comma in inline collections
    if (stream.eat(',')) {
      return 'separator';
    }

    // Double colon for nested dict marker when used alone
    if (stream.match('::')) {
      return 'punctuation';
    }

    // Default - consume one character
    stream.next();
    return null;
  },

  languageData: {
    commentTokens: { line: '#' },
    closeBrackets: { brackets: ['[', '{', '"', "'"] },
  },
});

/**
 * Simple indentation-based folding for HUML
 */
function foldByIndent(state: EditorState, lineStart: number, lineEnd: number) {
  const line = state.doc.lineAt(lineStart);
  const indent = line.text.search(/\S/);

  if (indent < 0) return null;

  let lastLine = line;
  let foldEnd = lineEnd;

  // Find all consecutive lines with greater indentation
  for (let pos = line.to + 1; pos < state.doc.length; ) {
    const nextLine = state.doc.lineAt(pos);
    const nextIndent = nextLine.text.search(/\S/);

    if (nextIndent < 0) {
      // Empty line - continue
      pos = nextLine.to + 1;
      continue;
    }

    if (nextIndent <= indent) {
      // Back to same or less indentation - end fold
      break;
    }

    lastLine = nextLine;
    foldEnd = nextLine.to;
    pos = nextLine.to + 1;
  }

  // Only fold if we have child content
  if (lastLine.number > line.number) {
    return { from: line.to, to: foldEnd };
  }

  return null;
}

/**
 * HUML language support for CodeMirror with folding
 */
export function huml() {
  return new LanguageSupport(humlLanguage, [
    // Add indentation-based folding service
    foldService.of(foldByIndent)
  ]);
}
