import { jsPDF } from "jspdf";
import { open, BaseDirectory } from "@tauri-apps/plugin-fs";

interface PdfResult {
  show: boolean;
  message: string;
  success: boolean;
}

export const pdfGenerator = async (content: string, givenTitle: string): Promise<PdfResult> => {
  try {
    const title = givenTitle;
    const safeTitle = title.replace(/[^a-z0-9]/gi, "_").toLowerCase();

    // Generate sequence number from timestamp
    const sequence = String(Date.now() % 10000).padStart(4, '0');
    const uniqueFileName = `${safeTitle}-${sequence}.pdf`;

    const editorEl = document.querySelector(".ProseMirror");
    if (!editorEl) {
      throw new Error("Editor content not found");
    }

    const container = document.createElement("div");
    container.innerHTML = `
        <div style="width: 100%;">
          <style>
            @import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap');
            
            .pdf-prosemirror {
              position: relative;
              padding: 20px;
              min-height: 100px;
              outline: none;
              line-height: 1.5;
              color: black;
              background: white;
              flex-grow: 1;
              font-family: 'Inter', 'Segoe UI', sans-serif;
              font-size: 16px;
              word-break: break-word;
              overflow-wrap: break-word;
            }
            
            .pdf-prosemirror p {
              margin: 0 0 1em 0;
              line-height: 1.5;
              word-break: break-word;
              white-space: normal;
              display: block;
              position: relative;
            }
            
            .pdf-prosemirror h1 {
              font-size: 2em;
              margin: 0.67em 0;
              color: black;
              font-weight: 700;
              line-height: 1.2;
            }
            
            .pdf-prosemirror h2 {
              font-size: 1.5em;
              margin: 0.83em 0;
              color: black;
              font-weight: 700;
              line-height: 1.2;
            }
            
            .pdf-prosemirror h3 {
              font-size: 1.17em;
              margin: 1em 0;
              color: black;
              font-weight: 600;
              line-height: 1.2;
            }
            
            .pdf-prosemirror h4 {
              font-size: 1.1em;
              margin: 1.1em 0;
              color: black;
              font-weight: 600;
              line-height: 1.2;
            }
            
            .pdf-prosemirror h5 {
              font-size: 1.05em;
              margin: 1.2em 0;
              color: black;
              font-weight: 600;
              line-height: 1.2;
            }
            
            .pdf-prosemirror h6 {
              font-size: 1em;
              margin: 1.3em 0;
              color: black;
              font-weight: 600;
              line-height: 1.2;
            }
            
            .pdf-prosemirror ul {
              padding-left: 1.5em;
              margin: 0.5em 0;
              list-style-type: disc;
            }
            
            .pdf-prosemirror ul li {
              margin: 0.2em 0;
              position: relative;
              line-height: 1.5;
            }
            
            .pdf-prosemirror ol {
              padding-left: 1.5em;
              margin: 0.5em 0;
              list-style-type: decimal;
            }
            
            .pdf-prosemirror ol li {
              margin: 0.2em 0;
              line-height: 1.5;
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
            
            .pdf-prosemirror blockquote {
              border-left: 3px solid #4a4b53;
              margin: 1em 0;
              padding: 8px 16px 8px 12px;
              font-style: italic;
              color: #333;
              background-color: #f5f5f5;
              border-radius: 4px;
            }
            
            .pdf-prosemirror code {
              background: #f0f0f0;
              padding: 0.1em 0.3em;
              border-radius: 3px;
              font-family: 'Courier New', Courier, monospace;
              font-size: 0.9em;
            }
            
            .pdf-prosemirror pre {
              background: #f0f0f0;
              padding: 1em;
              border-radius: 4px;
              overflow-x: auto;
              font-family: 'Courier New', Courier, monospace;
              font-size: 0.9em;
              line-height: 1.4;
            }
            
            /* Text alignment styles */
            .pdf-prosemirror [style*="text-align: center"] {
              text-align: center;
            }
            
            .pdf-prosemirror [style*="text-align: right"] {
              text-align: right;
            }
            
            /* Indentation styles */
            .pdf-prosemirror [data-indent="1"] {
              margin-left: 2em;
            }
            
            .pdf-prosemirror [data-indent="2"] {
              margin-left: 4em;
            }
            
            .pdf-prosemirror [data-indent="3"] {
              margin-left: 6em;
            }
            
            /* Strong and emphasis styles */
            .pdf-prosemirror strong {
              font-weight: 700;
            }
            
            .pdf-prosemirror em {
              font-style: italic;
            }

            /* Link styling */
            .pdf-prosemirror a {
              color: rgb(50, 104, 185);
              display: inline-block;
              text-decoration: underline;
              text-decoration-color: rgb(50, 104, 185);
              line-height: inherit;
              vertical-align: baseline;
              position: static;
              float: none;
            }

          </style>
          <div class="pdf-prosemirror">
            ${editorEl.innerHTML}
          </div>
        </div>
      `;

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
    for (let i = 0; i < images.length; i++) {
      const img = images[i];
      // Add loading="eager" to ensure images are loaded before PDF generation
      img.loading = "eager";
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
              message: `Note exported as PDF to Downloads folder: ${uniqueFileName}`,
              success: true,
            });
          } catch (error: unknown) {
            console.error("Error saving PDF file:", error);
            reject({
              show: true,
              message: `Failed to save PDF file: ${error instanceof Error ? error.message : 'Unknown error'}`,
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
      message: `Failed to create PDF: ${error instanceof Error ? error.message : 'Unknown error'}`,
      success: false,
    };
  }
};
