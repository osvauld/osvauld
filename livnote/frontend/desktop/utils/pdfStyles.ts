/**
 * PDF Styles for Note Export
 * Adapted from editor CSS files for print/PDF output with light theme
 */

export const getPdfStyles = (): string => {
  return `
    /* Base styles with Inter font properly configured */
    .pdf-prosemirror {
      position: relative;
      padding: 20px;
      min-height: 100px;
      outline: none;
      line-height: 1.5;
      color: #000;
      background: white;
      flex-grow: 1;
      font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
      font-size: 16px;
      font-weight: 400;
      font-feature-settings: "cv02", "cv03", "cv04", "cv11";
      -webkit-font-smoothing: antialiased;
      -moz-osx-font-smoothing: grayscale;
      word-break: break-word;
      overflow-wrap: break-word;
    }
    
    /* Paragraph styles */
    .pdf-prosemirror p {
      margin: 0 0 0.75em 0;
      line-height: 1.6;
      word-break: break-word;
      white-space: normal;
      display: block;
      position: relative;
    }
    
    /* Heading styles with correct font weights matching editor */
    .pdf-prosemirror h1 {
      font-size: 2em;
      margin: 0.5em 0 0.4em 0;
      color: #000;
      font-weight: 600;
      line-height: 1.2;
    }
    
    .pdf-prosemirror h2 {
      font-size: 1.5em;
      margin: 0.6em 0 0.4em 0;
      color: #000;
      font-weight: 600;
      line-height: 1.2;
    }
    
    .pdf-prosemirror h3 {
      font-size: 1.17em;
      margin: 0.7em 0 0.4em 0;
      color: #000;
      font-weight: 600;
      line-height: 1.2;
    }
    
    .pdf-prosemirror h4 {
      font-size: 1.1em;
      margin: 0.8em 0 0.4em 0;
      color: #000;
      font-weight: 500;
      line-height: 1.2;
    }
    
    .pdf-prosemirror h5 {
      font-size: 1.05em;
      margin: 0.8em 0 0.4em 0;
      color: #000;
      font-weight: 500;
      line-height: 1.2;
    }
    
    .pdf-prosemirror h6 {
      font-size: 1em;
      margin: 0.8em 0 0.4em 0;
      color: #000;
      font-weight: 500;
      line-height: 1.2;
    }
    
    /* Strong and emphasis styles */
    .pdf-prosemirror strong, .pdf-prosemirror b {
      font-weight: 600;
    }
    
    .pdf-prosemirror em, .pdf-prosemirror i {
      font-style: italic;
    }
    
    /* Text decorations */
    .pdf-prosemirror u {
      text-decoration: underline;
    }
    
    .pdf-prosemirror s {
      text-decoration: line-through;
    }
    
    /* List styling */
    .pdf-prosemirror ul {
      padding-left: 1.8em;
      margin: 0.5em 0 0.75em 0;
      list-style-type: disc;
      list-style-position: outside;
    }
    
    .pdf-prosemirror ul li {
      margin: 0.25em 0;
      padding-left: 0.25em;
      line-height: 1.5;
    }
    
    .pdf-prosemirror ul li::marker {
      font-size: 0.8em;
    }
    
    .pdf-prosemirror ul li p {
      margin: 0;
      line-height: inherit;
    }
    
    .pdf-prosemirror ol {
      padding-left: 1.8em;
      margin: 0.5em 0 0.75em 0;
      list-style-type: decimal;
      list-style-position: outside;
    }
    
    .pdf-prosemirror ol li {
      margin: 0.25em 0;
      padding-left: 0.25em;
      line-height: 1.5;
    }
    
    .pdf-prosemirror ol li::marker {
      font-size: 0.9em;
    }
    
    .pdf-prosemirror ol li p {
      margin: 0;
      line-height: inherit;
    }
    
    /* Blockquote styling */
    .pdf-prosemirror blockquote {
      border-left: 3px solid #666;
      margin: 0.75em 0;
      padding: 8px 16px 8px 12px;
      font-style: italic;
      color: #333;
      background-color: #f5f5f5;
      border-radius: 4px;
    }
    
    /* Inline code style */
    .pdf-prosemirror code {
      background: #f0f0f0;
      padding: 0.1em 0.4em;
      border-radius: 3px;
      font-family: 'Courier New', Courier, monospace;
      font-size: 0.9em;
      color: #000;
    }
    
    /* Code block styles */
    .pdf-prosemirror pre {
      background: #f5f5f5;
      color: #000;
      font-family: 'Courier New', Courier, monospace;
      padding: 0.75em;
      border-radius: 4px;
      margin: 0.75em 0;
      overflow-x: auto;
      font-size: 0.9em;
      line-height: 1.4;
      white-space: pre-wrap;
      word-wrap: break-word;
    }
    
    /* Reset inline code styles when inside a pre (code block) */
    .pdf-prosemirror pre code {
      background: transparent;
      padding: 0;
      border-radius: 0;
      font-size: inherit;
    }
    
    /* Link styling adapted for print */
    .pdf-prosemirror a {
      color: #3268b9;
      text-decoration: underline;
      text-decoration-color: #3268b9;
      display: inline;
      line-height: inherit;
      vertical-align: baseline;
    }
    
    /* Image styles */
    .pdf-prosemirror img {
      max-width: 100%;
      height: auto;
      margin: 1.5em 0;
      border-radius: 4px;
      display: block;
    }
    
    /* Image alignment */
    .pdf-prosemirror img[style*="text-align: center"] {
      margin-left: auto;
      margin-right: auto;
    }
    
    .pdf-prosemirror img[style*="text-align: right"] {
      margin-left: auto;
      margin-right: 0;
    }
    
    /* Image captions */
    .pdf-prosemirror figure {
      margin: 1.5em 0;
      display: block;
    }
    
    .pdf-prosemirror figure img {
      margin: 0 0 0.5em 0;
    }
    
    .pdf-prosemirror figcaption {
      font-size: 0.9em;
      color: #666;
      text-align: center;
      margin-top: 0.5em;
    }
    
    /* Image with text wrapping */
    .pdf-prosemirror .image-float-left {
      float: left;
      margin: 0.5em 1em 0.5em 0;
      max-width: 50%;
    }
    
    .pdf-prosemirror .image-float-right {
      float: right;
      margin: 0.5em 0 0.5em 1em;
      max-width: 50%;
    }
    
    /* Text alignment styles */
    .pdf-prosemirror [style*="text-align: center"] {
      text-align: center;
    }
    
    .pdf-prosemirror [style*="text-align: right"] {
      text-align: right;
    }
    
    .pdf-prosemirror [style*="text-align: left"] {
      text-align: left;
    }
    
    /* Indentation styles */
    .pdf-prosemirror [data-indent="1"],
    .pdf-prosemirror p[data-indent="1"],
    .pdf-prosemirror h1[data-indent="1"],
    .pdf-prosemirror h2[data-indent="1"],
    .pdf-prosemirror h3[data-indent="1"],
    .pdf-prosemirror h4[data-indent="1"],
    .pdf-prosemirror h5[data-indent="1"],
    .pdf-prosemirror h6[data-indent="1"] {
      margin-left: 2em;
    }
    
    .pdf-prosemirror [data-indent="2"],
    .pdf-prosemirror p[data-indent="2"],
    .pdf-prosemirror h1[data-indent="2"],
    .pdf-prosemirror h2[data-indent="2"],
    .pdf-prosemirror h3[data-indent="2"],
    .pdf-prosemirror h4[data-indent="2"],
    .pdf-prosemirror h5[data-indent="2"],
    .pdf-prosemirror h6[data-indent="2"] {
      margin-left: 4em;
    }
    
    .pdf-prosemirror [data-indent="3"],
    .pdf-prosemirror p[data-indent="3"],
    .pdf-prosemirror h1[data-indent="3"],
    .pdf-prosemirror h2[data-indent="3"],
    .pdf-prosemirror h3[data-indent="3"],
    .pdf-prosemirror h4[data-indent="3"],
    .pdf-prosemirror h5[data-indent="3"],
    .pdf-prosemirror h6[data-indent="3"] {
      margin-left: 6em;
    }
    
    /* Table styles adapted from tableStyles.css */
    .pdf-prosemirror table {
      border-collapse: collapse;
      table-layout: fixed;
      width: 100%;
      margin: 2em 0;
      border: 1px solid #ccc;
    }
    
    .pdf-prosemirror td,
    .pdf-prosemirror th {
      min-width: 50px;
      border: 1px solid #ccc;
      padding: 8px 12px;
      vertical-align: top;
      box-sizing: border-box;
      position: relative;
      background: white;
      color: #000;
    }
    
    .pdf-prosemirror th {
      font-weight: 600;
      text-align: left;
      background: #f0f0f0;
      color: #000;
      font-size: 14px;
      letter-spacing: 0.025em;
    }
    
    .pdf-prosemirror td {
      font-size: 14px;
    }
    
    .pdf-prosemirror td p,
    .pdf-prosemirror th p {
      margin: 0;
    }
    
    /* Cell alignment helpers */
    .pdf-prosemirror td[align="center"],
    .pdf-prosemirror th[align="center"] {
      text-align: center;
    }
    
    .pdf-prosemirror td[align="right"],
    .pdf-prosemirror th[align="right"] {
      text-align: right;
    }
    
    /* Merged cells styling */
    .pdf-prosemirror td[colspan],
    .pdf-prosemirror th[colspan] {
      text-align: center;
    }
    
    .pdf-prosemirror td[rowspan],
    .pdf-prosemirror th[rowspan] {
      vertical-align: middle;
    }
    
    /* Math equation styles adapted for PDF (light theme) */
    .pdf-prosemirror .katex,
    .pdf-prosemirror .katex * {
      color: #000 !important;
    }
    
    .pdf-prosemirror .katex .mathnormal {
      color: #000 !important;
    }
    
    .pdf-prosemirror .math-node,
    .pdf-prosemirror math-inline,
    .pdf-prosemirror math-display {
      color: #000 !important;
      background: transparent !important;
    }
    
    .pdf-prosemirror .math-render {
      padding: 0.25rem;
      background: transparent !important;
    }
    
    .pdf-prosemirror .math-display {
      margin: 1rem 0;
      padding: 0.5rem 0;
      text-align: center;
    }
    
    .pdf-prosemirror .math-inline {
      margin: 0 0.125rem;
      display: inline-block;
      vertical-align: middle;
    }
    
    /* Hide equation numbers and labels in PDF */
    .pdf-prosemirror .equation-number,
    .pdf-prosemirror .equation-label-container {
      display: none !important;
    }
    
    /* Math source code container (editing view) - hide in PDF */
    .pdf-prosemirror .math-src {
      display: none !important;
    }
    
    /* CRITICAL: Hide all comment-related styles */
    .pdf-prosemirror .livnote-comment-highlight,
    .pdf-prosemirror span[data-livnote-comment],
    .pdf-prosemirror span[data-livnote-comment-ids] {
      border: none !important;
      border-bottom: none !important;
      border-top: none !important;
      border-left: none !important;
      border-right: none !important;
      background: transparent !important;
      background-color: transparent !important;
      padding: 0 !important;
      box-shadow: none !important;
      text-decoration: none !important;
      outline: none !important;
    }
    
    .pdf-prosemirror .livnote-comment-highlight::after,
    .pdf-prosemirror .livnote-comment-highlight::before,
    .pdf-prosemirror span[data-livnote-comment]::after,
    .pdf-prosemirror span[data-livnote-comment]::before {
      display: none !important;
      content: none !important;
    }
    
    .pdf-prosemirror .livnote-comment-highlight.active,
    .pdf-prosemirror .livnote-comment-highlight.resolved,
    .pdf-prosemirror .livnote-comment-highlight.comment-text-highlight {
      border: none !important;
      border-bottom: none !important;
      background: transparent !important;
      background-color: transparent !important;
      text-decoration: none !important;
    }
    
    /* Hide ProseMirror separator elements */
    .pdf-prosemirror .ProseMirror-separator {
      display: none !important;
    }
  `;
};

