# PDF Image Splitting Issue - Failure Analysis Report

## Problem Statement
Images in PDF exports are being cut in half when page breaks occur, creating an undesirable appearance where the top portion appears on one page and the bottom portion on the next page.

## Root Cause Analysis
The issue stems from jsPDF's `html()` method with `autoPaging: "text"` mode, which:
- Breaks pages at text node boundaries rather than block element boundaries
- Does not respect CSS `page-break-inside: avoid` properties for images
- Treats images as inline content that can be split across pages

## Attempted Solutions & Results

### Method 1: CSS Page-Break Properties ❌ FAILED
**Approach:** Added CSS rules with `page-break-inside: avoid` and `break-inside: avoid` to images, figures, and other block elements.

**Implementation:**
```css
.pdf-prosemirror img {
  page-break-inside: avoid !important;
  break-inside: avoid !important;
}
```

**Result:** No effect. jsPDF's text autopaging mode ignores these CSS properties.

### Method 2: Changed AutoPaging Mode to "slice" ❌ FAILED
**Approach:** Changed from `autoPaging: "text"` to `autoPaging: "slice"` to handle block elements better.

**Implementation:**
```javascript
pdf.html(container, {
  autoPaging: "slice",
  // ... other options
});
```

**Result:** Images still split across pages. The "slice" mode didn't provide the expected block-aware pagination.

### Method 3: Changed AutoPaging Mode to "true" ❌ FAILED
**Approach:** Used `autoPaging: true` to enable CSS-aware pagination.

**Implementation:**
```javascript
pdf.html(container, {
  autoPaging: true,
  // ... other options
});
```

**Result:** **CATASTROPHIC FAILURE** - Font sizes became abnormally large, content became unreadable, and images were still cut off. This mode completely broke the PDF rendering.

### Method 4: Programmatic Image Wrapping ❌ FAILED
**Approach:** Wrapped images in protective `<div>` containers with inline page-break styles.

**Implementation:**
```javascript
function wrapImagesForPagination(container) {
  const images = container.querySelectorAll('img');
  images.forEach(img => {
    const wrapper = document.createElement('div');
    wrapper.className = 'pdf-image-wrapper';
    wrapper.style.cssText = 'page-break-inside: avoid; break-inside: avoid;';
    img.parentNode.insertBefore(wrapper, img);
    wrapper.appendChild(img);
  });
}
```

**Result:** No improvement. The wrapper divs were ignored by jsPDF's rendering engine.

### Method 5: Height-Based Page Break Insertion (First Attempt) ❌ FAILED
**Approach:** Calculate content heights and insert explicit page breaks before images that would be split.

**Implementation:**
- Created temporary DOM for height measurement
- Used approximate page height (970px)
- Inserted `<div class="pdf-page-break">` elements with `page-break-before: always`

**Result:** Failed due to incorrect height calculations and element matching issues.

### Method 6: Refined Height-Based Page Break Insertion ❌ FAILED
**Approach:** Improved the height calculation to match jsPDF's exact rendering parameters.

**Implementation:**
```javascript
// Precise calculations matching jsPDF options
const CONTENT_WIDTH_MM = 170; // width option
const WINDOW_WIDTH_PX = 1000; // windowWidth option  
const PAGE_HEIGHT_MM = 297; // A4
const USABLE_HEIGHT_MM = 257; // minus margins
const PX_PER_MM = WINDOW_WIDTH_PX / CONTENT_WIDTH_MM;
const PAGE_HEIGHT_PX = Math.floor(USABLE_HEIGHT_MM * PX_PER_MM); // ≈1512px
```

**Result:** Still failed. The explicit page break divs are not being respected by jsPDF.

## Technical Analysis of Failures

### Why CSS Page-Break Properties Don't Work
1. **jsPDF's HTML Renderer:** Uses html2canvas internally, which doesn't implement full CSS page-break support
2. **Text Autopaging Mode:** Specifically designed for text flow, not block element preservation
3. **Canvas-Based Rendering:** The final output is a canvas, not a traditional CSS layout engine

### Why Explicit Page Break Divs Don't Work
1. **Limited CSS Support:** jsPDF's CSS parser has limited support for page-break properties
2. **Rendering Pipeline:** The html2canvas → jsPDF pipeline doesn't process page-break CSS rules
3. **Text Mode Limitation:** `autoPaging: "text"` mode specifically ignores block-level page breaks

### Why Alternative AutoPaging Modes Failed
1. **"slice" Mode:** Still doesn't respect CSS page-break properties
2. **"true" Mode:** Completely breaks the rendering pipeline, causing font and layout issues

## Current State
- **Baseline:** PDF generation works correctly for text content
- **Issue:** Images are consistently split across page boundaries
- **Impact:** Professional appearance is compromised
- **Status:** No working solution found through standard jsPDF approaches

## Potential Alternative Approaches (Not Yet Attempted)

### Option A: Manual Canvas-Based Rendering
- Use html2canvas to render each page manually
- Calculate image positions and force page breaks
- More complex but full control over pagination

### Option B: Pre-Process HTML Structure
- Split content into page-sized chunks before PDF generation
- Ensure images never cross chunk boundaries
- Requires complex content analysis

### Option C: Alternative PDF Library
- Switch from jsPDF to a different library (PDFKit, Puppeteer, etc.)
- May require significant refactoring

### Option D: Image Resizing Strategy
- Dynamically resize large images to fit within page boundaries
- Trade-off between image quality and pagination

## Recommendations

1. **Immediate:** Accept the current limitation and document it as a known issue
2. **Short-term:** Implement Option D (image resizing) as a workaround
3. **Long-term:** Evaluate Option C (alternative PDF library) for a complete solution

## Conclusion

The PDF image splitting issue is a fundamental limitation of jsPDF's HTML rendering pipeline. All standard approaches using CSS page-break properties and explicit page break elements have failed because jsPDF's text autopaging mode doesn't implement proper page-break support for block elements.

The most promising path forward would be either implementing a manual canvas-based rendering approach or migrating to a different PDF generation library that provides better control over page breaks and block element handling.

## Files Modified During Investigation

- `utils/pdfGenerator.ts` - Main PDF generation logic
- `utils/pdfStyles.ts` - CSS styles for PDF export

## Code Changes Summary

### Added Functions (Currently Disabled)
- `insertImagePageBreaks()` - Height-based page break insertion
- `getElementXPath()` - Element path calculation
- `getElementByXPath()` - Element retrieval by path

### CSS Additions
- Page-break prevention rules for images, figures, tables, code blocks
- Global orphans/widows control
- Explicit page-break div styling

### Configuration Attempts
- Various `autoPaging` mode experiments
- html2canvas configuration options
- Image loading and CORS handling improvements

---

*Report generated on: $(date)*
*Status: All attempted solutions failed - issue remains unresolved*
