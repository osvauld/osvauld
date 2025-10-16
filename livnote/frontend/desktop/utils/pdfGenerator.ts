import { jsPDF } from "jspdf";
import { open, BaseDirectory } from "@tauri-apps/plugin-fs";
import { getPdfStyles } from "./pdfStyles";

interface PdfResult {
  show: boolean;
  message: string;
  success: boolean;
}

/**
 * Remove comment-related markup from HTML for PDF export
 * Mirrors the cleanup logic in RichTextEditor.svelte
 */
function cleanupCommentMarkup(container: HTMLElement): void {
  // Find all comment-related elements by data attribute or class
  const commentSpans = container.querySelectorAll(
    "[data-livnote-comment], .livnote-comment-highlight"
  );
  
  commentSpans.forEach((span) => {
    // Remove all comment-specific attributes
    span.removeAttribute("data-livnote-comment");
    span.removeAttribute("data-livnote-comment-ids");
    span.removeAttribute("data-livnote-comment-count");
    span.removeAttribute("data-livnote-resolved");
    span.removeAttribute("data-livnote-author");
    span.removeAttribute("data-livnote-internal");

    // Remove all comment classes
    span.classList.remove(
      "livnote-comment-highlight",
      "active",
      "resolved",
      "comment-text-highlight"
    );

    // Aggressively remove ALL inline styles from comment spans
    // Comments should be invisible - just keep the text
    span.removeAttribute("style");
  });
}

export const pdfGenerator = async (content: string, givenTitle: string): Promise<PdfResult> => {
  try {
    const title = givenTitle;
    // Keep spaces and capitalization, only remove/replace unsafe filename characters
    const safeTitle = title
      .replace(/[<>:"/\\|?*]/g, '') // Remove invalid filename characters
      .replace(/\s+/g, ' ')          // Normalize multiple spaces to single space
      .trim();                        // Remove leading/trailing spaces

    // Generate sequence number from timestamp
    const sequence = String(Date.now() % 10000).padStart(4, '0');
    const uniqueFileName = `${safeTitle}-${sequence}.pdf`;

    const editorEl = document.querySelector(".ProseMirror");
    if (!editorEl) {
      throw new Error("Editor content not found");
    }

    // Wait for Inter font to be fully loaded before PDF generation
    // This ensures the PDF uses the correct Inter font from @osvauld/fonts
    try {
      await document.fonts.load('400 16px Inter');
      await document.fonts.load('500 16px Inter');
      await document.fonts.load('600 16px Inter');
      // Wait for all fonts to be ready
      await document.fonts.ready;
    } catch (fontError) {
      console.warn('Font loading warning:', fontError);
      // Continue with PDF generation even if font check fails
    }

    const container = document.createElement("div");
    
    // Create title heading if available (escape HTML to prevent injection)
    const titleHtml = title && title !== "Untitled" 
      ? `<h1 style="margin-top: 0; margin-bottom: 1.5em; padding-bottom: 0.5em; border-bottom: 2px solid #e5e5e5;">${title.replace(/</g, '&lt;').replace(/>/g, '&gt;')}</h1>`
      : '';
    
    container.innerHTML = `
      <div style="width: 100%;">
        <style>
          ${getPdfStyles()}
        </style>
        <div class="pdf-prosemirror">
          ${titleHtml}
          ${editorEl.innerHTML}
        </div>
      </div>
    `;

    // Clean up comment markup before PDF generation
    cleanupCommentMarkup(container);

    const pdf = new jsPDF({
      unit: "mm",
      format: "a4",
      compress: true,
      hotfixes: ["px_scaling"]
    });

    const pageWidth = 210;
    const contentWidth = 170;
    const marginTop = 20;
    const marginBottom = 20;

    // Process images before PDF generation
    const images = container.getElementsByTagName('img');
    const imageLoadPromises: Promise<void>[] = [];
    
    for (let i = 0; i < images.length; i++) {
      const img = images[i];
      
      // Add crossOrigin attribute to handle CORS
      img.crossOrigin = "anonymous";
      
      // Ensure max-width is respected
      if (!img.style.maxWidth) {
        img.style.maxWidth = "100%";
      }
      // Add default margins if not set
      if (!img.style.margin) {
        img.style.margin = "1.5em 0";
      }
      
      // Wait for image to load if not already loaded
      if (!img.complete) {
        imageLoadPromises.push(
          new Promise((resolve, reject) => {
            img.onload = () => resolve();
            img.onerror = () => {
              console.warn(`Failed to load image: ${img.src}`);
              resolve(); // Resolve anyway to not block PDF generation
            };
            // Add timeout to prevent hanging
            setTimeout(() => resolve(), 5000);
          })
        );
      }
    }
    
    // Wait for all images to load
    if (imageLoadPromises.length > 0) {
      await Promise.all(imageLoadPromises);
    }

    // Wrap pdf.html in a Promise
    return new Promise((resolve, reject) => {
      pdf.html(container, {
        callback: async function (pdf) {
          try {
            const totalPages = (pdf as any).internal.getNumberOfPages();
            for (let i = 1; i <= totalPages; i++) {
              pdf.setPage(i);
              pdf.setFontSize(10);
              pdf.setTextColor(100, 100, 100);
              const pageText = `Page ${i} of ${totalPages}`;
              const pageTextWidth =
                (pdf.getStringUnitWidth(pageText) * 10) /
                pdf.internal.scaleFactor;
              const pageTextX = (pageWidth - pageTextWidth) / 2;
              const pageNumberY = 285;
              pdf.text(pageText, pageTextX, pageNumberY);
            }

            const pdfData = pdf.output("arraybuffer");
            const pdfBuffer = new Uint8Array(pdfData);
            const filePath = uniqueFileName;

            const file = await open(filePath, {
              write: true,
              create: true,
              truncate: true,
              baseDir: BaseDirectory.Download,
            });

            await file.write(pdfBuffer);
            await file.close();

            resolve({
              show: true,
              message: `${uniqueFileName} Saved to Downloads folder`,
              success: true,
            });
          } catch (error: unknown) {
            console.error("Error saving PDF file:", error);
            reject({
              show: true,
              message: `Failed to save PDF file`,
              success: false,
            });
          }
        },
        x: 0,
        y: 0,
        width: contentWidth,
        windowWidth: 1000,
        margin: [marginTop, 20, marginBottom, 20],
        autoPaging: "text",
      });
    });
  } catch (error: unknown) {
    console.error("Error creating PDF:", error);
    return {
      show: true,
      message: `Failed to create PDF`,
      success: false,
    };
  }
};
