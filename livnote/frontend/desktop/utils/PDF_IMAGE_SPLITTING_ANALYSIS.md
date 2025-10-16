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

### Method 7: Manual Page Splitting with Height Calculation ❌ FAILED
**Approach:** Complete bypass of jsPDF's autopaging by pre-calculating content heights, splitting into page-sized chunks, and rendering each page separately with `autoPaging: false`.

**Implementation:**

Created `utils/pdfPageSplitter.ts` with:

```typescript
// Precise page dimension calculations
const CONTENT_WIDTH_MM = 170;
const WINDOW_WIDTH_PX = 1000;
const PAGE_HEIGHT_MM = 297; // A4
const USABLE_HEIGHT_MM = 257; // minus margins
const PX_PER_MM = WINDOW_WIDTH_PX / CONTENT_WIDTH_MM;
const PAGE_HEIGHT_PX = Math.floor(USABLE_HEIGHT_MM * PX_PER_MM); // ~1512px
const SAFETY_MARGIN_PX = 100; // buffer for page numbers
const EFFECTIVE_PAGE_HEIGHT_PX = PAGE_HEIGHT_PX - SAFETY_MARGIN_PX;

// Measure element heights
function measureElementHeight(element) {
  // Special handling for images using naturalWidth/naturalHeight
  if (element.tagName === 'IMG' && img.complete) {
    const containerWidth = WINDOW_WIDTH_PX;
    let imgWidth = img.naturalWidth;
    let imgHeight = img.naturalHeight;
    
    // Scale to fit container
    if (imgWidth > containerWidth) {
      const scale = containerWidth / imgWidth;
      imgHeight = imgHeight * scale;
    }
    
    return imgHeight + margins;
  }
  
  // Default: clone element into hidden container and measure
  const clone = element.cloneNode(true);
  measurementContainer.appendChild(clone);
  const height = clone.getBoundingClientRect().height;
  measurementContainer.removeChild(clone);
  return height;
}

// Split content into pages
function splitContentIntoPages(container, firstPageReservedHeight) {
  const pages = [];
  let currentPage = createPageContainer();
  let currentHeight = 0;
  
  for (const child of container.children) {
    const elementHeight = measureElementHeight(child);
    const isAtomic = isAtomicElement(child); // Images, tables, code blocks
    
    const pageMaxHeight = isFirstPage 
      ? EFFECTIVE_PAGE_HEIGHT_PX - firstPageReservedHeight 
      : EFFECTIVE_PAGE_HEIGHT_PX;
    
    if (currentHeight + elementHeight > pageMaxHeight) {
      if (isAtomic) {
        // Start new page for atomic elements
        pages.push(currentPage);
        currentPage = createPageContainer();
        currentPage.appendChild(child.cloneNode(true));
        currentHeight = elementHeight;
      } else {
        // Keep text on current page or move to next if >50% full
        // ...
      }
    } else {
      currentPage.appendChild(child.cloneNode(true));
      currentHeight += elementHeight;
    }
  }
  
  return pages;
}
```

Modified `utils/pdfGenerator.ts`:

```typescript
// Measure title height for first page reservation
const titleHeight = measureTitleHeight(titleHtml);

// Split content into pages
const splitResult = splitContentIntoPages(pdfProsemirror, titleHeight);

// Create all PDF pages upfront
for (let i = 1; i < splitResult.pages.length; i++) {
  pdf.addPage();
}

// Render each page to its designated PDF page
for (let pageIndex = 0; pageIndex < splitResult.pages.length; pageIndex++) {
  pdf.setPage(pageIndex + 1); // Set target page
  
  const pageWrapper = createPageWrapper(
    splitResult.pages[pageIndex],
    pageIndex === 0 ? titleHtml : ''
  );
  
  await pdf.html(pageWrapper, {
    autoPaging: false, // CRITICAL: No autopaging
    // ... other options
  });
}
```

**Multiple Sub-Attempts:**

**7a. Multiple Sequential pdf.html() Calls**
- Called `pdf.html()` for each page sequentially
- Added pages with `pdf.addPage()` between renders
- **Result:** First page blank, all content rendered to second page, images still split

**7b. Pre-Create Pages, Use setPage()**
- Created all pages upfront with `pdf.addPage()`
- Used `pdf.setPage(pageIndex)` before each `pdf.html()` call
- **Result:** Same issue - blank first page, content overflow

**7c. Simple vs Complex Container Detection**
- Attempted to distinguish image wrappers from complex containers
- Simple wrappers (≤2 children): Treated as atomic
- Complex containers: Allowed to split
- **Result:** Text between elements disappeared, content still overflowed under page numbers

**Why This Failed:**

1. **jsPDF's html() Method Limitation:**
   - Even with `autoPaging: false`, `pdf.html()` doesn't respect manual page targeting via `setPage()`
   - Multiple calls to `pdf.html()` appear to overwrite or ignore previous content
   - The rendering canvas doesn't honor pre-created PDF page structure

2. **Height Measurement Inaccuracy:**
   - DOM measurement (getBoundingClientRect) != jsPDF's html2canvas rendering
   - Images measured at 553px rendered at different heights in PDF
   - Margins, padding, and font rendering differ between DOM and canvas
   - Safety margins insufficient to compensate for rendering differences

3. **Atomic Element Detection Issues:**
   - Treating DIVs with images as atomic bundled unrelated content
   - Complex containers with multiple children and images couldn't be properly split
   - No way to split partial content within a container

4. **Missing Content:**
   - Text between images and tables disappeared
   - Content wrapped in same container as images got treated as one unit
   - No mechanism to extract and preserve inter-element content

5. **Page Number Overlap:**
   - Even with 100px safety margin, content rendered under page numbers
   - jsPDF's canvas output height != calculated DOM height
   - Table rendered at Y coordinate overlapping with page number at Y=285mm

**Logs Showed:**
```
Total pages: 2 (splitter created 2 pages)
Page 1: 34 elements, Page 2: 4 elements
Canvas renderer: 1860px (exceeded calculated 1403px limit)
Final PDF: 2 pages, but first page blank, second page overflow
```

**Fundamental Issues Discovered:**

1. **Rendering Pipeline Disconnect:**
   - Pre-splitting based on DOM measurements
   - But jsPDF uses html2canvas for final render
   - Two different rendering engines = different results
   - No way to ensure consistency

2. **jsPDF's html() Method Design:**
   - Designed to work as a single-call operation
   - Multiple calls with manual pagination not a supported use case
   - Internal state management conflicts with external page control

3. **No True Element-Level Control:**
   - Can't tell jsPDF "render only this element to this page"
   - All-or-nothing rendering per `html()` call
   - Manual content splitting disconnects from jsPDF's layout engine

**Result:** **COMPLETE FAILURE** - Created more problems than it solved. Blank pages, missing content, pagination overlap, and images still not properly handled.

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
- **Status:** **No working solution found through ANY jsPDF-based approach**
  - 7 major methods attempted across multiple iterations
  - All approaches failed to solve the core image splitting issue
  - Manual page splitting (Method 7) introduced additional problems (blank pages, missing content, pagination overlap)

## Remaining Alternative Approaches

### Option A: Browser Native Print API ⭐ RECOMMENDED
**Approach:** Use Tauri's native print/save-as-PDF functionality leveraging the browser's PDF engine.

**Pros:**
- ✅ Perfect CSS `page-break-inside: avoid` support (browsers implement this correctly)
- ✅ No external libraries needed
- ✅ Native font rendering and vector graphics
- ✅ Tauri already provides print APIs
- ✅ Zero bundle size increase

**Cons:**
- ⚠️ Requires user interaction for "Save as PDF" dialog (unless Tauri provides silent print command)
- ⚠️ File naming needs additional handling

**Implementation Path:**
```javascript
// Use window.print() with proper @media print CSS
// OR Tauri's programmatic print command
import { invoke } from '@tauri-apps/api/core';

// Option 1: User dialog
window.print();

// Option 2: Programmatic (if Tauri supports)
await invoke('print_to_pdf', { path: downloadsDir + filename });
```

### Option B: Puppeteer/Playwright via Tauri Sidecar
**Approach:** Bundle headless Chromium as a Tauri sidecar for server-side PDF generation.

**Pros:**
- ✅ Industry standard (used by Notion, Confluence)
- ✅ Complete CSS support including page-break properties
- ✅ Same quality as Chrome's "Print to PDF"
- ✅ Silent/programmatic generation

**Cons:**
- ❌ ~150MB bundle size increase (Chromium binary)
- ❌ More complex architecture
- ❌ Longer build times

### Option C: Accept Limitation + Image Resizing
**Approach:** Pre-process large images to fit within page boundaries, accept occasional splits for very tall images.

**Pros:**
- ✅ Minimal code changes
- ✅ Reduces (but doesn't eliminate) image splitting

**Cons:**
- ❌ Image quality loss
- ❌ Doesn't fully solve the problem
- ❌ Very tall images still split

### Option D: Alternative JS PDF Library
**Approach:** Replace jsPDF with a different library.

**Candidates:**
- **pdfmake:** Programmatic PDF generation (not HTML-based)
  - ❌ Would require complete rewrite to convert ProseMirror → pdfmake format
  - ❌ Significant maintenance burden
  
- **html-pdf-node:** Uses Puppeteer under the hood
  - ⚠️ Similar to Option B but less control
  
- **React-PDF / @react-pdf/renderer:**
  - ❌ React-specific, requires component rewrite

## Updated Recommendations

### Immediate (1-2 days)
**Implement Option A: Browser Native Print API**

This is the **cleanest and most maintainable** solution:
1. Add `@media print` CSS rules to `pdfStyles.ts`:
   ```css
   @media print {
     img, figure, pre, table, blockquote {
       page-break-inside: avoid !important;
       break-inside: avoid !important;
     }
   }
   ```

2. Replace `pdfGenerator.ts` content with browser print:
   ```typescript
   export const pdfGenerator = async (content, title) => {
     // Prepare print-friendly view
     // Trigger window.print() or Tauri print command
     // Handle file saving via Tauri dialog
   };
   ```

3. Test cross-platform (macOS, Windows, Linux)

**Pros:** Solves the problem completely, minimal code, no external dependencies

**Trade-off:** May require user to click "Save" in print dialog (acceptable UX for desktop app)

### Short-term (1 week)
**If Option A not feasible:** Evaluate Option B (Puppeteer sidecar)
- Research Tauri sidecar setup
- Prototype PDF generation via Chromium
- Measure bundle size impact

### Long-term
**Monitor jsPDF updates** - The library may eventually add proper page-break support, but this is not guaranteed.

## Conclusion

After **7 comprehensive attempts** spanning CSS approaches, autopaging configuration, and manual page splitting with height calculations, we conclude:

### Root Cause
**jsPDF's `html()` method is fundamentally incompatible with proper page-break handling:**
1. Uses html2canvas for rendering (canvas-based, not CSS layout)
2. Autopaging operates on text nodes, not block elements
3. No support for CSS `page-break-inside: avoid`
4. Multiple `pdf.html()` calls don't compose properly
5. Manual page control via `setPage()` ignored by html2canvas rendering
6. Height measurement discrepancies between DOM and canvas output

### The Path Forward
**Browser native print is the only viable solution** that:
- Respects CSS page-break properties (proven, standard behavior)
- Provides high-quality output without complexity
- Doesn't require external binaries or massive refactoring
- Works reliably across all platforms

**jsPDF should be abandoned for HTML-to-PDF conversion in favor of browser-native PDF generation or headless browser solutions (Puppeteer).**

## Files Modified During Investigation

### Core Files
- `utils/pdfGenerator.ts` - Main PDF generation logic (heavily modified across all attempts)
- `utils/pdfStyles.ts` - CSS styles for PDF export

### New Files Created (Method 7)
- `utils/pdfPageSplitter.ts` - Manual page splitting implementation (442 lines)
  - Element height measurement functions
  - Atomic element detection
  - Page splitting algorithm with first-page title reservation
  - Comprehensive logging for debugging

## Code Changes Summary

### Method 1-6 Additions
- CSS page-break prevention rules for images, figures, tables, code blocks
- Global orphans/widows control
- Explicit page-break div styling
- Various `autoPaging` mode configurations
- html2canvas configuration options
- Image loading and CORS handling improvements

### Method 7 Implementation (Manual Page Splitting)
**Created `utils/pdfPageSplitter.ts`:**
- `measureElementHeight()` - DOM and image-specific height calculations
- `isAtomicElement()` - Detection of non-splittable elements (images, tables, code blocks)
- `splitContentIntoPages()` - Main splitting algorithm with ~400 lines of logic
- `createPageContainer()` - Page wrapper creation
- `logSplitStats()` - Debugging output

**Modified `utils/pdfGenerator.ts`:**
- Title height measurement for first-page reservation
- Integration with page splitter
- Multiple render strategies attempted:
  - Sequential `pdf.html()` calls with `pdf.addPage()`
  - Pre-creating pages with `pdf.setPage()` targeting
  - Various `autoPaging` configurations (`false`, `"text"`)
- Comprehensive logging at each render step

### Issues Encountered
- Blank first pages despite content splitting
- Content overflow under page numbers
- Missing text between elements
- Images still being split despite atomic marking
- Height measurement != final render height
- Multiple `pdf.html()` calls interfering with each other

---

*Report last updated: October 2025*  
*Status: **7 methods attempted, all failed** - jsPDF incompatible with reliable page-break handling*  
*Recommendation: **Migrate to browser-native print API or Puppeteer***
