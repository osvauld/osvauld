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
        const clipboardData = event.clipboardData;
        if (!clipboardData) return false;

        // Snapshot all clipboard data synchronously before async processing
        const snapshotData = {
          files: Array.from(clipboardData.files),
          html: clipboardData.getData('text/html'),
          text: clipboardData.getData('text/plain'),
          types: Array.from(clipboardData.types)
        };

        // Always prevent default and handle asynchronously
        event.preventDefault();

        // Process with both methods - try navigator.clipboard API first, then snapshot
        (async () => {
          try {
            // First try the navigator clipboard API for better image handling
            const apiHandled = await tryNavigatorClipboardApi(view, imageStorage);
            if (apiHandled) {
              return;
            }

            // Fall back to snapshot data
            await processSnapshotData(view, snapshotData, imageStorage);
          } catch (error) {
            console.error("Error in paste handler:", error);
          }
        })();

        return true; // Always return true to prevent default
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
              return true;
            } else if (preferredType === 'text/plain') {
              const text = await blob.text();
              handleTextContent(view, text);
              return true;
            } else if (preferredType.startsWith('image/')) {
              const base64Data = await blobToBase64(blob);
              await insertImageWithAssetStorage(view, base64Data, preferredType, imageStorage);
              return true;
            } else if (preferredType === 'application/octet-stream') {
              if (isLikelyImage(blob)) {
                const headerBytes = await readBlobHeader(blob, 12);
                if (isProbablyImageHeader(headerBytes)) {
                  const base64Data = await blobToBase64(blob);
                  const mimeType = detectMimeTypeFromHeader(headerBytes);
                  await insertImageWithAssetStorage(view, base64Data, mimeType, imageStorage);
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
 * Process the snapshotted clipboard data
 */
async function processSnapshotData(
  view: EditorView,
  data: {
    files: File[];
    html: string;
    text: string;
    types: string[];
  },
  imageStorage: ImageStorageService
): Promise<void> {
  // Process files first (handles images)
  if (data.files.length > 0) {
    for (const file of data.files) {
      if (file.type.startsWith('image/')) {
        const base64Data = await blobToBase64(file);
        await insertImageWithAssetStorage(view, base64Data, file.type, imageStorage, file.name);
        return;
      }

      // Handle files with unknown mime types
      if (file.type === 'application/octet-stream' || file.type === '') {
        if (file.name && /\.(jpg|jpeg|png|gif|bmp|webp|svg)$/i.test(file.name)) {
          const base64Data = await blobToBase64(file);
          const mimeType = getMimeTypeFromFilename(file.name);
          await insertImageWithAssetStorage(view, base64Data, mimeType, imageStorage, file.name);
          return;
        }

        if (isLikelyImage(file)) {
          const headerBytes = await readBlobHeader(file, 12);
          if (isProbablyImageHeader(headerBytes)) {
            const base64Data = await blobToBase64(file);
            const mimeType = detectMimeTypeFromHeader(headerBytes);
            await insertImageWithAssetStorage(view, base64Data, mimeType, imageStorage, file.name);
            return;
          }
        }
      }
    }
  }

  // Process HTML content
  if (data.html) {
    await handleHtmlContent(view, data.html, imageStorage);
    return;
  }

  // Process text content (markdown)
  if (data.text) {
    handleTextContent(view, data.text);
  }
}

/**
 * Insert image using the new asset storage system
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

    // Constrain initial dimensions for pasted images
    let width = imageMetadata?.width || 200;
    let height = imageMetadata?.height || 150;

    const maxInitialWidth = 600;
    const maxInitialHeight = 450;

    if (width && height) {
      const aspectRatio = width / height;

      // Scale down if too wide
      if (width > maxInitialWidth) {
        width = maxInitialWidth;
        height = maxInitialWidth / aspectRatio;
      }

      // Scale down if too tall
      if (height > maxInitialHeight) {
        height = maxInitialHeight;
        width = maxInitialHeight * aspectRatio;
      }

      width = Math.round(width);
      height = Math.round(height);
    }

    const { schema } = view.state;
    const imageNode = schema.nodes.image.create({
      src: `yjs-image:${imageId}`,
      alt: filename || 'Pasted image',
      title: filename || 'Pasted image',
      width: width,
      height: height
    });

    const tr = view.state.tr.replaceSelectionWith(imageNode);
    view.dispatch(tr);
  } catch (error) {
    console.error("Error inserting image with asset storage:", error);
  }
}
/**
 * Handle HTML content from clipboard, including embedded images
 */
async function handleHtmlContent(view: EditorView, html: string, imageStorage: ImageStorageService): Promise<void> {
  try {
    const domElement = document.createElement('div');
    domElement.innerHTML = html;

    // Remove problematic black text colors for dark theme
    const styledElements = domElement.querySelectorAll('*[style]');
    styledElements.forEach(el => {
      if (el instanceof HTMLElement) {
        const style = el.style;
        if (style.color === 'rgb(0, 0, 0)' || style.color === '#000000' || style.color === 'black') {
          style.removeProperty('color');
        }
        if (style.caretColor === 'rgb(0, 0, 0)') {
          style.removeProperty('caret-color');
        }
        if (!style.cssText.trim()) {
          el.removeAttribute('style');
        }
      }
    });

    // Enforce minimum font size (16px) for pasted content
    const MIN_FONT_PX = 16;
    const toPx = (value: string): number | null => {
      const v = value.trim().toLowerCase();
      if (v.endsWith('px')) return parseFloat(v);
      if (v.endsWith('pt')) return parseFloat(v) * (96 / 72); // pt -> px
      return null; // skip other units to avoid clobbering larger styles
    };

    // Adjust elements with inline font-size and legacy <font size=""> tags
    const allElements = domElement.querySelectorAll('*');
    allElements.forEach(node => {
      if (!(node instanceof HTMLElement)) return;

      // Inline style font-size
      const fs = node.style.fontSize;
      if (fs) {
        const px = toPx(fs);
        if (px !== null && px < MIN_FONT_PX) {
          node.style.fontSize = `${MIN_FONT_PX}px`;
        }
      }

      // Legacy <font size> handling
      if (node.tagName.toLowerCase() === 'font') {
        const fontNode = node as HTMLFontElement;
        if (fontNode.getAttribute('size') !== null) {
          node.style.fontSize = `${MIN_FONT_PX}px`;
          fontNode.removeAttribute('size');
        }
      }
    });

    // Check if content should be parsed as markdown
    const textContent = domElement.textContent || '';
    if (textContent && detectMarkdown(textContent)) {
      handleTextContent(view, textContent);
      return;
    }

    // Process link formatting
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

    // Clean up anchor text decoration
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

    // Process images in HTML content
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
              img.remove();
            }
          }
        } else {
          img.remove();
        }
      }
    }

    // Parse and insert content
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
      return;
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
      }
    }

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

    const indent = match[1].length;
    const content = match[2];

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
 * Parse markdown text and convert to ProseMirror nodes
 */
function parseMarkdown(text: string, schema: any): PMNode[] {
  const nodes: PMNode[] = [];
  const lines = text.split('\n');

  let i = 0;
  while (i < lines.length) {
    const line = lines[i];

    // Code blocks
    if (line.trim().startsWith('```')) {
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith('```')) {
        codeLines.push(lines[i]);
        i++;
      }
      const codeContent = codeLines.join('\n');
      nodes.push(schema.nodes.code_block.create({},
        codeContent ? schema.text(codeContent) : schema.text('')
      ));
      i++;
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

    // Blockquote
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
 * Helper function to safely get a mark type from schema
 */
function getMark(schema: any, markName: string) {
  return schema.marks[markName] || null;
}

/**
 * Parse inline markdown with proper escape handling and schema guards
 */
function parseInlineMarkdown(text: string, schema: any): Fragment {
  if (!text) return Fragment.empty;

  const nodes: PMNode[] = [];
  let remaining = text;

  while (remaining.length > 0) {
    let matched = false;

    // Handle escaped characters first
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
      const patterns = [
        // Links first (highest priority)
        {
          pattern: /^\[([^\]]+)\]\(([^)]+)\)/,
          handler: (match: RegExpMatchArray) => {
            const linkText = match[1];
            const href = match[2];
            const linkType = getMark(schema, 'link');

            if (!linkType) {
              return [schema.text(`[${linkText}](${href})`)];
            }

            const linkMark = linkType.create({ href, title: linkText });

            if (/[*_`~]/.test(linkText)) {
              const linkContent = parseInlineMarkdown(linkText, schema);
              const result: PMNode[] = [];
              linkContent.forEach((node: PMNode) => {
                if (node.isText) {
                  const marks = [...node.marks, linkMark];
                  result.push(schema.text(node.text, marks));
                }
              });
              return result;
            } else {
              return [schema.text(linkText, [linkMark])];
            }
          }
        },
        // Inline code
        {
          pattern: /^`([^`]+)`/,
          handler: (match: RegExpMatchArray) => {
            const codeType = getMark(schema, 'code');
            return [schema.text(match[1], codeType ? [codeType.create()] : [])];
          }
        },
        // Bold **
        {
          pattern: /^\*\*([^*]+)\*\*/,
          handler: (match: RegExpMatchArray) => {
            const innerText = match[1];
            const strongType = getMark(schema, 'strong');

            if (innerText.includes('*') || innerText.includes('_')) {
              const innerContent = parseInlineMarkdown(innerText, schema);
              const strongMark = strongType ? strongType.create() : undefined;
              const result: PMNode[] = [];
              innerContent.forEach((node: PMNode) => {
                if (node.isText) {
                  const marks = strongMark ? [...node.marks, strongMark] : node.marks;
                  result.push(schema.text(node.text, marks));
                }
              });
              return result;
            }

            return [schema.text(innerText, strongType ? [strongType.create()] : [])];
          }
        },
        // Bold __
        {
          pattern: /^__([^_]+)__/,
          handler: (match: RegExpMatchArray) => {
            const strongType = getMark(schema, 'strong');
            return [schema.text(match[1], strongType ? [strongType.create()] : [])];
          }
        },
        // Strikethrough
        {
          pattern: /^~~([^~]+)~~/,
          handler: (match: RegExpMatchArray) => {
            const strikeType = getMark(schema, 'strikethrough');
            return [schema.text(match[1], strikeType ? [strikeType.create()] : [])];
          }
        },
        // Italic *
        {
          pattern: /^\*([^*]+)\*/,
          handler: (match: RegExpMatchArray) => {
            const emType = getMark(schema, 'em');
            return [schema.text(match[1], emType ? [emType.create()] : [])];
          }
        },
        // Italic _
        {
          pattern: /^_([^_]+)_/,
          handler: (match: RegExpMatchArray) => {
            const emType = getMark(schema, 'em');
            return [schema.text(match[1], emType ? [emType.create()] : [])];
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
    const lastNode = nodes[nodes.length - 1];
    nodes[nodes.length - 1] = schema.text(lastNode.text + text);
  } else {
    nodes.push(schema.text(text));
  }
}

/**
 * Detect if text contains intentional markdown syntax
 */
function detectMarkdown(text: string): boolean {
  let markdownScore = 0;
  const threshold = 1.5;

  const strongIndicators = [
    { pattern: /^#{1,6}\s+\S/m, score: 2 },
    { pattern: /^```[^`]*```/ms, score: 3 },
    { pattern: /^\s*```\w*\s*$/m, score: 3 },
    { pattern: /^\s*[-*+]\s+\S.*(\n\s*[-*+]\s+|$)/m, score: 2 },
    { pattern: /^\s*\d+\.\s+\S.*(\n\s*\d+\.\s+|$)/m, score: 2 },
    { pattern: /^\s*>\s+\S/m, score: 2 },
    { pattern: /\[([^\]]+)\]\(([^)]+)\)/g, score: 2 },
    { pattern: /^[-*_]{3,}\s*$/m, score: 2 },
  ];

  const mediumIndicators = [
    { pattern: /`[^`\n]+`/, score: 2 },
    { pattern: /\*\*\S[^*]+\S\*\*/, score: 2 },
    { pattern: /__\S[^_]+\S__/, score: 2 },
    { pattern: /~~\S[^~]+\S~~/, score: 2 },
  ];

  const weakIndicators = [
    { pattern: /(?:^|\s)\*\S[^*\n]+\S\*(?:\s|$)/, score: 0.5 },
    { pattern: /(?:^|\s)_\S[^_\n]+\S_(?:\s|$)/, score: 0.5 },
  ];

  // Check strong indicators first
  for (const { pattern, score } of strongIndicators) {
    if (pattern.test(text)) {
      markdownScore += score;
      if (pattern.source.includes('```') && text.includes('\n')) {
        markdownScore += 0.5;
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

    if (weakMatches >= 2) {
      markdownScore += 0.5;
    }
  }

  // Special cases for higher confidence
  if (/`[^`]+`/.test(text) && /```/.test(text)) {
    markdownScore += 1;
  }

  if (/^\s*[-*+\d]\s+/m.test(text) && /[*_`~]/.test(text)) {
    markdownScore += 0.5;
  }

  return markdownScore >= threshold;
}

/**
 * Check if a blob is likely an image based on size
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
 * Convert blob to base64 string
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

/**
 * Detect MIME type from file header bytes
 */
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

/**
 * Get MIME type from filename extension
 */
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
