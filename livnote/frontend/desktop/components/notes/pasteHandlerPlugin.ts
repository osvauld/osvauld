import { Plugin } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { DOMParser, Fragment, Slice, Node as PMNode } from "prosemirror-model";
import { ImageStorageService } from "./imageStorage";

/**
 * Creates a ProseMirror plugin that handles clipboard content
 * using the navigator.clipboard API with optimized asset storage
 */
export function pasteHandlerPlugin(imageStorage: ImageStorageService) {
  return new Plugin({
    props: {
      handlePaste: (view: EditorView, event: ClipboardEvent) => {
        (async () => {
          const clipboardData = event.clipboardData;

          try {
            const apiHandled = await tryNavigatorClipboardApi(view, event, imageStorage);

            if (apiHandled) {
              return;
            }

            if (clipboardData) {
              const handled = await processClipboardEvent(view, event, imageStorage);
              if (handled) {
                return;
              }
            }
          } catch (error) {
            console.error("Error in paste handler:", error);
          }
        })();

        return true;
      }
    }
  });
}

/**
 * Tries to process clipboard content using the navigator.clipboard API
 * Returns true if successful, false otherwise
 */
async function tryNavigatorClipboardApi(
  view: EditorView,
  event: ClipboardEvent,
  imageStorage: ImageStorageService
): Promise<boolean> {
  if (!navigator.clipboard?.read) {
    return false;
  }

  try {
    const clipboardItems = await navigator.clipboard.read();

    const typePreference = [
      'text/html',
      'text/plain',
      'image/png',
      'image/jpeg',
      'image/gif',
      'image/webp',
      'image/bmp',
      'image/svg+xml',
      'application/octet-stream',
      'image/*'
    ];

    for (const preferredType of typePreference) {
      for (const item of clipboardItems) {
        if (preferredType === 'image/*') {
          const imageTypes = item.types.filter(type => type.startsWith('image/'));
          if (imageTypes.length > 0) {
            try {
              const imageType = imageTypes[0];
              const blob = await item.getType(imageType);
              const base64Data = await blobToBase64(blob);
              await insertImageWithAssetStorage(view, base64Data, imageType, imageStorage);
              event.preventDefault();
              return true;
            } catch (error) {
              continue;
            }
          }
          continue;
        }

        if (item.types.includes(preferredType)) {
          try {
            const blob = await item.getType(preferredType);

            if (preferredType === 'text/html') {
              const html = await blob.text();
              await handleHtmlContent(view, html, imageStorage);
              event.preventDefault();
              return true;
            } else if (preferredType === 'text/plain') {
              const text = await blob.text();
              handleTextContent(view, text);
              event.preventDefault();
              return true;
            } else if (preferredType.startsWith('image/')) {
              const base64Data = await blobToBase64(blob);
              await insertImageWithAssetStorage(view, base64Data, preferredType, imageStorage);
              event.preventDefault();
              return true;
            } else if (preferredType === 'application/octet-stream') {
              if (isLikelyImage(blob)) {
                const headerBytes = await readBlobHeader(blob, 12);
                if (isProbablyImageHeader(headerBytes)) {
                  const base64Data = await blobToBase64(blob);
                  const mimeType = detectMimeTypeFromHeader(headerBytes);
                  await insertImageWithAssetStorage(view, base64Data, mimeType, imageStorage);
                  event.preventDefault();
                  return true;
                }
              }
            }
          } catch (error) {
            continue;
          }
        }
      }
    }

    return false;
  } catch (error) {
    return false;
  }
}

/**
 * Process clipboard content from ClipboardEvent
 */
async function processClipboardEvent(
  view: EditorView,
  event: ClipboardEvent,
  imageStorage: ImageStorageService
): Promise<boolean> {
  if (!event.clipboardData) return false;

  try {
    if (event.clipboardData.files.length > 0) {
      for (let i = 0; i < event.clipboardData.files.length; i++) {
        const file = event.clipboardData.files[i];

        if (file.type.startsWith('image/')) {
          const base64Data = await blobToBase64(file);
          await insertImageWithAssetStorage(view, base64Data, file.type, imageStorage, file.name);
          event.preventDefault();
          return true;
        }

        if (file.type === 'application/octet-stream' || file.type === '') {
          if (file.name && /\.(jpg|jpeg|png|gif|bmp|webp|svg)$/i.test(file.name)) {
            const base64Data = await blobToBase64(file);
            const mimeType = getMimeTypeFromFilename(file.name);
            await insertImageWithAssetStorage(view, base64Data, mimeType, imageStorage, file.name);
            event.preventDefault();
            return true;
          }

          if (isLikelyImage(file)) {
            const headerBytes = await readBlobHeader(file, 12);
            if (isProbablyImageHeader(headerBytes)) {
              const base64Data = await blobToBase64(file);
              const mimeType = detectMimeTypeFromHeader(headerBytes);
              await insertImageWithAssetStorage(view, base64Data, mimeType, imageStorage, file.name);
              event.preventDefault();
              return true;
            }
          }
        }
      }
    }

    const html = event.clipboardData.getData('text/html');
    if (html) {
      await handleHtmlContent(view, html, imageStorage);
      event.preventDefault();
      return true;
    }

    const text = event.clipboardData.getData('text/plain');
    if (text) {
      handleTextContent(view, text);
      event.preventDefault();
      return true;
    }

    return false;
  } catch (error) {
    return false;
  }
}

/**
 * Insert image using the new asset storage system
 * This replaces the old insertImageWithStorage function
 */
async function insertImageWithAssetStorage(
  view: EditorView,
  base64Data: string,
  mimeType: string,
  imageStorage: ImageStorageService,
  filename?: string
): Promise<void> {
  try {

    const imageId = await imageStorage.storeImage(base64Data, mimeType, filename);

    const imageMetadata = imageStorage.getImageMetadata(imageId);

    const { schema } = view.state;
    const imageNode = schema.nodes.image.create({
      src: `yjs-image:${imageId}`, // Use the same protocol for consistency
      alt: filename || 'Pasted image',
      title: filename || 'Pasted image',
      width: imageMetadata?.width,
      height: imageMetadata?.height
    });

    const tr = view.state.tr.replaceSelectionWith(imageNode);
    view.dispatch(tr);

  } catch (error) {
    console.error("Error inserting image with asset storage:", error);
  }
}

/**
 * Handle HTML content from clipboard, including embedded images
 * Updated to use the new asset storage system
 */
async function handleHtmlContent(view: EditorView, html: string, imageStorage: ImageStorageService): Promise<void> {
  try {
    const domElement = document.createElement('div');
    domElement.innerHTML = html;

    const spansToProcess = domElement.querySelectorAll('span[style*="text-decoration"]');
    spansToProcess.forEach(span => {
      if (span instanceof HTMLElement && span.style.textDecoration.includes('underline')) {
        let childLinkElement: HTMLAnchorElement | null = null;
        let hasOtherSignificantContent = false;

        const significantChildNodes = Array.from(span.childNodes).filter(node => {
          if (node.nodeType === Node.ELEMENT_NODE) return true;
          if (node.nodeType === Node.TEXT_NODE && node.textContent?.trim()) return true;
          return false;
        });

        if (significantChildNodes.length === 1 &&
          significantChildNodes[0].nodeName === 'A' &&
          (significantChildNodes[0] as HTMLAnchorElement).hasAttribute('href')) {
          childLinkElement = significantChildNodes[0] as HTMLAnchorElement;
        } else {
          hasOtherSignificantContent = true;
        }

        if (childLinkElement && !hasOtherSignificantContent) {
          const newLink = childLinkElement.cloneNode(true) as HTMLAnchorElement;

          let existingLinkStyle = newLink.getAttribute('style') || '';
          if (existingLinkStyle && !existingLinkStyle.trim().endsWith(';')) {
            existingLinkStyle += '; ';
          }
          if (!newLink.style.textDecoration.includes('underline')) {
            newLink.setAttribute('style', `${existingLinkStyle}text-decoration: underline;`);
          }

          if (span.parentNode) {
            span.parentNode.replaceChild(newLink, span);
          }
        }
      }
    });

    const allAnchors = domElement.querySelectorAll('a');
    allAnchors.forEach(anchor => {
      if (anchor.style.textDecoration.includes('underline')) {
        anchor.style.textDecoration = anchor.style.textDecoration.replace(/underline/g, '').trim();
        if (anchor.style.textDecoration === '') {
          anchor.removeAttribute('style');
        }
        const styleAttr = anchor.getAttribute('style');
        if (styleAttr && styleAttr.trim() === '') {
          anchor.removeAttribute('style');
        }
      }
    });

    const images = domElement.querySelectorAll('img');

    if (images.length > 0) {

      for (let i = 0; i < images.length; i++) {
        const img = images[i];
        const src = img.getAttribute('src');

        if (src) {
          if (src.startsWith('data:')) {
            try {
              const mimeMatch = src.match(/^data:([^;]+);/);
              const mimeType = mimeMatch ? mimeMatch[1] : 'image/png';

              const imageId = await imageStorage.storeImage(src, mimeType);
              img.setAttribute('src', `yjs-image:${imageId}`);

              const imageMetadata = imageStorage.getImageMetadata(imageId);
              if (imageMetadata?.width) img.setAttribute('width', imageMetadata.width.toString());
              if (imageMetadata?.height) img.setAttribute('height', imageMetadata.height.toString());

            } catch (error) {
              console.error('Error processing data: image:', error);
              img.remove();
            }
          } else if (src.startsWith('blob:')) {
            try {
              const response = await fetch(src);
              const blob = await response.blob();
              const dataUrl = await blobToBase64(blob);

              const imageId = await imageStorage.storeImage(dataUrl, blob.type);
              img.setAttribute('src', `yjs-image:${imageId}`);

              const imageMetadata = imageStorage.getImageMetadata(imageId);
              if (imageMetadata?.width) img.setAttribute('width', imageMetadata.width.toString());
              if (imageMetadata?.height) img.setAttribute('height', imageMetadata.height.toString());

            } catch (error) {
              console.error('Error processing blob: image:', error);
              img.remove(); // Remove if blob fetch fails
            }
          }
        } else {
          img.remove(); // Remove if no src
        }
      }
    }

    const { schema } = view.state;
    const parser = DOMParser.fromSchema(schema);
    const slice = parser.parseSlice(domElement);

    const filteredNodes: PMNode[] = [];
    slice.content.forEach(node => {
      if (node.type === schema.nodes.paragraph) {
        if (node.content.size > 0 && node.textContent.trim() !== '') {
          filteredNodes.push(node);
        }
      } else {
        filteredNodes.push(node);
      }
    });

    if (filteredNodes.length === 0 && slice.content.size > 0) {
      return; // Nothing to insert
    }

    const newFragment = Fragment.fromArray(filteredNodes);
    const newSlice = new Slice(newFragment, slice.openStart, slice.openEnd);

    const tr = view.state.tr.replaceSelection(newSlice);
    view.dispatch(tr);

  } catch (error) {
    console.error("Error handling HTML content:", error);
  }
}

/**
 * Handle plain text content from clipboard
 */
function handleTextContent(view: EditorView, text: string): void {
  try {
    // Only try to parse as markdown if it looks intentionally formatted
    if (detectMarkdown(text)) {
      try {
        const nodes = parseMarkdown(text, view.state.schema);
        if (nodes.length > 0) {
          const fragment = Fragment.from(nodes);
          const slice = new Slice(fragment, 0, 0);
          const tr = view.state.tr.replaceSelection(slice);
          view.dispatch(tr);
          return;
        }
      } catch (error) {
        console.warn("Failed to parse as markdown, falling back to plain text:", error);
        // Fall through to plain text insertion
      }
    }

    // Insert as plain text if not markdown or if parsing failed
    const tr = view.state.tr.insertText(text);
    view.dispatch(tr);
  } catch (error) {
    console.error("Error handling text content:", error);
  }
}


/**
 * Parse unordered list with better nesting support
 */
function parseUnorderedList(
  lines: string[],
  startIndex: number,
  schema: any
): { list: PMNode | null, nextIndex: number } {
  const items: PMNode[] = [];
  let i = startIndex;

  while (i < lines.length) {
    const line = lines[i];
    const match = line.match(/^(\s*)[-*+]\s+(.*)$/);

    if (!match) {
      // Check if it's a continuation line (indented content)
      if (i > startIndex && line.trim() !== '' && (line.startsWith('  ') || line.startsWith('\t'))) {
        // This is a continuation of the previous item
        if (items.length > 0) {
          const lastItem = items[items.length - 1];
          const existingContent = lastItem.content;
          const additionalText = schema.text(' ' + line.trim());
          const paragraph = schema.nodes.paragraph.create({},
            existingContent.child(0).content.append(Fragment.from(additionalText))
          );
          items[items.length - 1] = schema.nodes.list_item.create({}, paragraph);
        }
        i++;
        continue;
      }
      break;
    }

    const indent = match[1].length;
    const content = match[2];

    // Handle nested lists by checking indentation
    if (indent > 0 && items.length > 0) {
      // This could be a nested list - for now, treat as regular item
      // Full nesting support would require recursive parsing
    }

    const itemContent = parseInlineMarkdown(content, schema);
    if (itemContent.size > 0) {
      const paragraph = schema.nodes.paragraph.create({}, itemContent);
      items.push(schema.nodes.list_item.create({}, paragraph));
    }
    i++;
  }

  if (items.length > 0) {
    return { list: schema.nodes.bullet_list.create({}, items), nextIndex: i };
  }

  return { list: null, nextIndex: i };
}

/**
 * Parse ordered list
 */
function parseOrderedList(
  lines: string[],
  startIndex: number,
  schema: any
): { list: PMNode | null, nextIndex: number } {
  const items: PMNode[] = [];
  let i = startIndex;

  while (i < lines.length) {
    const line = lines[i];
    const match = line.match(/^(\s*)\d+\.\s+(.*)$/);

    if (!match) {
      // Check for continuation lines
      if (i > startIndex && line.trim() !== '' && (line.startsWith('  ') || line.startsWith('\t'))) {
        if (items.length > 0) {
          const lastItem = items[items.length - 1];
          const existingContent = lastItem.content;
          const additionalText = schema.text(' ' + line.trim());
          const paragraph = schema.nodes.paragraph.create({},
            existingContent.child(0).content.append(Fragment.from(additionalText))
          );
          items[items.length - 1] = schema.nodes.list_item.create({}, paragraph);
        }
        i++;
        continue;
      }
      break;
    }

    const content = match[2];
    const itemContent = parseInlineMarkdown(content, schema);

    if (itemContent.size > 0) {
      const paragraph = schema.nodes.paragraph.create({}, itemContent);
      items.push(schema.nodes.list_item.create({}, paragraph));
    }
    i++;
  }

  if (items.length > 0) {
    return { list: schema.nodes.ordered_list.create({}, items), nextIndex: i };
  }

  return { list: null, nextIndex: i };
}

/**
 * Parse markdown text and convert to ProseMirror nodes (IMPROVED VERSION)
 */
function parseMarkdown(text: string, schema: any): PMNode[] {
  const nodes: PMNode[] = [];
  const lines = text.split('\n');

  let i = 0;
  while (i < lines.length) {
    const line = lines[i];

    // Code block - handle properly
    if (line.trim().startsWith('```')) {
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith('```')) {
        codeLines.push(lines[i]);
        i++;
      }
      // Create code block even if empty
      const codeContent = codeLines.join('\n');
      nodes.push(schema.nodes.code_block.create({},
        codeContent ? schema.text(codeContent) : schema.text('')
      ));
      i++; // Skip closing ```
      continue;
    }

    // Headers
    const headerMatch = line.match(/^(#{1,6})\s+(.+)$/);
    if (headerMatch) {
      const level = headerMatch[1].length;
      const content = parseInlineMarkdown(headerMatch[2], schema);
      nodes.push(schema.nodes.heading.create({ level }, content));
      i++;
      continue;
    }

    // Horizontal rule
    if (/^[-*_]{3,}\s*$/.test(line.trim())) {
      nodes.push(schema.nodes.horizontal_rule.create());
      i++;
      continue;
    }

    // Blockquote - parse each line separately to maintain structure
    if (line.startsWith('>')) {
      const quoteNodes: PMNode[] = [];
      while (i < lines.length && lines[i].startsWith('>')) {
        const quoteLine = lines[i].replace(/^>\s?/, '');
        if (quoteLine.trim()) {
          const content = parseInlineMarkdown(quoteLine, schema);
          quoteNodes.push(schema.nodes.paragraph.create({}, content));
        }
        i++;
      }
      if (quoteNodes.length > 0) {
        nodes.push(schema.nodes.blockquote.create({}, quoteNodes));
      }
      continue;
    }

    // Lists
    if (/^\s*[-*+]\s+/.test(line)) {
      const listResult = parseUnorderedList(lines, i, schema);
      if (listResult.list) {
        nodes.push(listResult.list);
        i = listResult.nextIndex;
        continue;
      }
    }

    if (/^\s*\d+\.\s+/.test(line)) {
      const listResult = parseOrderedList(lines, i, schema);
      if (listResult.list) {
        nodes.push(listResult.list);
        i = listResult.nextIndex;
        continue;
      }
    }

    // Empty line
    if (line.trim() === '') {
      i++;
      continue;
    }

    // Regular paragraph
    const content = parseInlineMarkdown(line, schema);
    if (content.size > 0) {
      nodes.push(schema.nodes.paragraph.create({}, content));
    }
    i++;
  }

  return nodes;
}

/**
 * Parse inline markdown with proper escape handling (FIXED VERSION)
 */
function parseInlineMarkdown(text: string, schema: any): Fragment {
  if (!text) return Fragment.empty;

  const nodes: PMNode[] = [];
  let remaining = text;

  while (remaining.length > 0) {
    let matched = false;

    // Handle escaped characters - remove backslash and keep the character
    if (remaining.startsWith('\\') && remaining.length > 1) {
      const char = remaining[1];
      const escapableChars = ['*', '_', '`', '~', '[', ']', '(', ')', '#', '-', '+', '!', '\\', '|', '{', '}'];
      if (escapableChars.includes(char)) {
        appendTextNode(nodes, char, schema);
        remaining = remaining.slice(2);
        matched = true;
        continue;
      }
    }

    if (!matched) {
      // Patterns array with improved handlers
      const patterns = [
        // Links first (highest priority)
        {
          pattern: /^\[([^\]]+)\]\(([^)]+)\)/,
          handler: (match: RegExpMatchArray) => {
            const linkText = match[1];
            const href = match[2];
            const linkMark = schema.marks.link.create({ href, title: linkText });

            // Check if link text has formatting
            if (/[*_`~]/.test(linkText)) {
              // Parse the link text for inline formatting
              const linkContent = parseInlineMarkdown(linkText, schema);
              const result: PMNode[] = [];

              linkContent.forEach((node: PMNode) => {
                if (node.isText) {
                  // Add link mark to existing marks
                  const marks = [...node.marks, linkMark];
                  result.push(schema.text(node.text, marks));
                }
              });

              return result;
            } else {
              // Plain text link
              return [schema.text(linkText, [linkMark])];
            }
          }
        },
        // Inline code (high priority to avoid conflicts)
        {
          pattern: /^`([^`]+)`/,
          handler: (match: RegExpMatchArray) => {
            const mark = schema.marks.code.create();
            return [schema.text(match[1], [mark])];
          }
        },
        // Bold ** (must come before single *)
        {
          pattern: /^\*\*([^*]+)\*\*/,
          handler: (match: RegExpMatchArray) => {
            const innerText = match[1];
            // Check for nested emphasis
            if (innerText.includes('*') || innerText.includes('_')) {
              const innerContent = parseInlineMarkdown(innerText, schema);
              const strongMark = schema.marks.strong.create();
              const result: PMNode[] = [];
              innerContent.forEach((node: PMNode) => {
                if (node.isText) {
                  const marks = [...node.marks, strongMark];
                  result.push(schema.text(node.text, marks));
                }
              });
              return result;
            }
            const mark = schema.marks.strong.create();
            return [schema.text(innerText, [mark])];
          }
        },
        // Bold __
        {
          pattern: /^__([^_]+)__/,
          handler: (match: RegExpMatchArray) => {
            const mark = schema.marks.strong.create();
            return [schema.text(match[1], [mark])];
          }
        },
        // Strikethrough
        {
          pattern: /^~~([^~]+)~~/,
          handler: (match: RegExpMatchArray) => {
            const mark = schema.marks.strikethrough.create();
            return [schema.text(match[1], [mark])];
          }
        },
        // Italic * (check it's not part of **)
        {
          pattern: /^\*([^*]+)\*/,
          handler: (match: RegExpMatchArray) => {
            const mark = schema.marks.em.create();
            return [schema.text(match[1], [mark])];
          }
        },
        // Italic _
        {
          pattern: /^_([^_]+)_/,
          handler: (match: RegExpMatchArray) => {
            const mark = schema.marks.em.create();
            return [schema.text(match[1], [mark])];
          }
        }
      ];

      for (const { pattern, handler } of patterns) {
        const match = remaining.match(pattern);
        if (match) {
          const result = handler(match);
          if (result) {
            nodes.push(...result);
            remaining = remaining.slice(match[0].length);
            matched = true;
            break;
          }
        }
      }
    }

    // No pattern matched - take one character as plain text
    if (!matched) {
      const char = remaining[0];
      appendTextNode(nodes, char, schema);
      remaining = remaining.slice(1);
    }
  }

  return Fragment.from(nodes);
}
/**
 * Helper to append text to nodes array, merging with previous text node if possible
 */
function appendTextNode(nodes: PMNode[], text: string, schema: any): void {
  if (nodes.length > 0 && nodes[nodes.length - 1].isText &&
    nodes[nodes.length - 1].marks.length === 0) {
    // Append to previous plain text node
    const lastNode = nodes[nodes.length - 1];
    nodes[nodes.length - 1] = schema.text(lastNode.text + text);
  } else {
    nodes.push(schema.text(text));
  }
}
/**
 * Detect if text contains intentional markdown syntax
 * Uses a scoring system to avoid false positives
 */
function detectMarkdown(text: string): boolean {
  let markdownScore = 0;
  let threshold = 2; // Require at least 2 points to trigger markdown parsing

  // Strong indicators (more likely to be intentional markdown)
  const strongIndicators = [
    { pattern: /^#{1,6}\s+\S/m, score: 2 },              // Headers with content
    { pattern: /^```[^`]*```/ms, score: 3 },             // Code blocks (multiline flag)
    { pattern: /^\s*```\w*\s*$/m, score: 3 },            // Opening code fence
    { pattern: /^\s*[-*+]\s+\S.*(\n\s*[-*+]\s+|$)/m, score: 2 }, // Multiple list items or single with content
    { pattern: /^\s*\d+\.\s+\S.*(\n\s*\d+\.\s+|$)/m, score: 2 }, // Multiple ordered items or single with content
    { pattern: /^\s*>\s+\S/m, score: 2 },                // Blockquotes with content
    { pattern: /\[([^\]]+)\]\(([^)]+)\)/g, score: 2 },   // Links (very specific syntax)
    { pattern: /^[-*_]{3,}\s*$/m, score: 2 },            // Horizontal rules
  ];

  // Medium indicators (could be markdown, need other context)
  const mediumIndicators = [
    { pattern: /`[^`\n]+`/, score: 1 },                  // Inline code
    { pattern: /\*\*\S[^*]+\S\*\*/, score: 1 },          // Bold with **
    { pattern: /__\S[^_]+\S__/, score: 1 },              // Bold with __
    { pattern: /~~\S[^~]+\S~~/, score: 1 },              // Strikethrough
  ];

  // Weak indicators (often coincidental)
  const weakIndicators = [
    { pattern: /(?:^|\s)\*\S[^*\n]+\S\*(?:\s|$)/, score: 0.5 }, // Italic with *
    { pattern: /(?:^|\s)_\S[^_\n]+\S_(?:\s|$)/, score: 0.5 },   // Italic with _
  ];

  // Check strong indicators first
  for (const { pattern, score } of strongIndicators) {
    const matches = text.match(pattern);
    if (matches) {
      markdownScore += score;
      // If we find code blocks or multiple structural elements, boost confidence
      if (pattern.source.includes('```') && text.includes('\n')) {
        markdownScore += 0.5; // Extra boost for multiline code blocks
      }
    }
  }

  // Check medium indicators
  let mediumMatches = 0;
  for (const { pattern, score } of mediumIndicators) {
    if (pattern.test(text)) {
      mediumMatches++;
      markdownScore += score;
    }
  }

  // If we have multiple medium indicators, it's likely markdown
  if (mediumMatches >= 2) {
    markdownScore += 0.5;
  }

  // Only check weak indicators if we already have some confidence
  if (markdownScore > 0) {
    let weakMatches = 0;
    for (const { pattern, score } of weakIndicators) {
      if (pattern.test(text)) {
        weakMatches++;
        markdownScore += score;
      }
    }

    // Multiple weak indicators together suggest intentional markdown
    if (weakMatches >= 2) {
      markdownScore += 0.5;
    }
  }

  // Special case: if text has inline code and code blocks, very likely markdown
  if (/`[^`]+`/.test(text) && /```/.test(text)) {
    markdownScore += 1;
  }

  // Special case: if text has both lists and formatting, likely markdown
  if (/^\s*[-*+\d]\.\?\s+/m.test(text) && /[*_`~]/.test(text)) {
    markdownScore += 0.5;
  }

  return markdownScore >= threshold;
}
/**
 * Check if a blob is likely an image based on magic numbers/signatures
 */
function isLikelyImage(blob: Blob): boolean {
  return blob.size > 10 && blob.size < 20 * 1024 * 1024;
}

/**
 * Check if byte array matches common image format headers
 */
function isProbablyImageHeader(bytes: Uint8Array): boolean {
  if (bytes.length < 4) return false;

  // JPEG - starts with FF D8 FF
  if (bytes[0] === 0xFF && bytes[1] === 0xD8 && bytes[2] === 0xFF) {
    return true;
  }

  // PNG - starts with 89 50 4E 47
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4E && bytes[3] === 0x47) {
    return true;
  }

  // GIF - starts with 47 49 46 38
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x38) {
    return true;
  }

  // BMP - starts with 42 4D
  if (bytes[0] === 0x42 && bytes[1] === 0x4D) {
    return true;
  }

  // WEBP - starts with 52 49 46 46 and has WEBP at offset 8
  if (bytes.length >= 12 &&
    bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46 &&
    bytes[8] === 0x57 && bytes[9] === 0x45 && bytes[10] === 0x42 && bytes[11] === 0x50) {
    return true;
  }

  // SVG - starts with either '<svg' or '<?xml'
  if (bytes.length >= 5) {
    // Check for '<svg'
    if (bytes[0] === 0x3C && bytes[1] === 0x73 && bytes[2] === 0x76 && bytes[3] === 0x67) {
      return true;
    }
    // Check for '<?xml'
    if (bytes[0] === 0x3C && bytes[1] === 0x3F && bytes[2] === 0x78 && bytes[3] === 0x6D && bytes[4] === 0x6C) {
      return true;
    }
  }

  return false;
}

/**
 * Convert blob to base64 string using a FileReader with proper async/await pattern
 */
async function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

/**
 * Read the first few bytes of a blob to analyze its header
 */
async function readBlobHeader(blob: Blob, bytesToRead: number): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const arrayBuffer = reader.result as ArrayBuffer;
      resolve(new Uint8Array(arrayBuffer));
    };
    reader.onerror = () => reject(reader.error);

    const slicedBlob = blob.slice(0, bytesToRead);
    reader.readAsArrayBuffer(slicedBlob);
  });
}

function detectMimeTypeFromHeader(bytes: Uint8Array): string {
  if (bytes[0] === 0xFF && bytes[1] === 0xD8 && bytes[2] === 0xFF) {
    return 'image/jpeg';
  }
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4E && bytes[3] === 0x47) {
    return 'image/png';
  }
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x38) {
    return 'image/gif';
  }
  if (bytes[0] === 0x42 && bytes[1] === 0x4D) {
    return 'image/bmp';
  }
  if (bytes.length >= 12 &&
    bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46 &&
    bytes[8] === 0x57 && bytes[9] === 0x45 && bytes[10] === 0x42 && bytes[11] === 0x50) {
    return 'image/webp';
  }
  return 'image/png'; // Default
}

function getMimeTypeFromFilename(filename: string): string {
  const ext = filename.toLowerCase().split('.').pop();
  const mimeMap: { [key: string]: string } = {
    'jpg': 'image/jpeg',
    'jpeg': 'image/jpeg',
    'png': 'image/png',
    'gif': 'image/gif',
    'bmp': 'image/bmp',
    'webp': 'image/webp',
    'svg': 'image/svg+xml'
  };
  return mimeMap[ext || ''] || 'image/png';
}
