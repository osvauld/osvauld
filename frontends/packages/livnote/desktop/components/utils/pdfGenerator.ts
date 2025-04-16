import { jsPDF } from "jspdf";
import { open, BaseDirectory } from "@tauri-apps/plugin-fs";
import { extractTitle } from "./helper";

interface PdfResult {
	show: boolean;
	message: string;
	success: boolean;
}

export const pdfGenerator = async (content: string, givenTitle?: string): Promise<PdfResult> => {
	try {
		const title = givenTitle || extractTitle(content);
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
            .pdf-prosemirror {
              position: relative;
              padding: 15px;
              min-height: 100px;
              outline: none;
              line-height: 1.5;
              color: black;
              background: white;
              flex-grow: 1;
            }
            .pdf-prosemirror p {
              margin: 0 0 1em 0;
            }
            .pdf-prosemirror h1 {
              font-size: 2em;
              margin: 0.67em 0;
              color: black;
              font-weight: bold;
            }
            .pdf-prosemirror h2 {
              font-size: 1.5em;
              margin: 0.83em 0;
              color: black;
              font-weight: bold;
            }
            .pdf-prosemirror h3 {
              font-size: 1.17em;
              margin: 1em 0;
              color: black;
              font-weight: bold;
            }
            .pdf-prosemirror h4 {
              font-size: 1.1em;
              margin: 1.1em 0;
              color: black;
              font-weight: bold;
            }
            .pdf-prosemirror h5 {
              font-size: 1.05em;
              margin: 1.2em 0;
              color: black;
              font-weight: bold;
            }
            .pdf-prosemirror h6 {
              font-size: 1em;
              margin: 1.3em 0;
              color: black;
              font-weight: bold;
            }
            .pdf-prosemirror ul {
              padding-left: 1.5em;
              margin: 0.5em 0;
              list-style-type: disc;
            }
            .pdf-prosemirror ul li {
              margin: 0.2em 0;
              position: relative;
            }
            .pdf-prosemirror ol {
              padding-left: 1.5em;
              margin: 0.5em 0;
              list-style-type: decimal;
            }
            .pdf-prosemirror blockquote {
              border-left: 3px solid rgb(80, 80, 82);
              margin-left: 0;
              margin-right: 0;
              padding-left: 1em;
              font-style: italic;
              color: #000;
              background-color: rgba(167, 162, 162, 0.82);
              border-radius: 4px;
              padding: 8px 16px 8px 12px;
            }
          </style>
          <div class="pdf-prosemirror" style="font-family: 'Inter', 'Segoe UI', sans-serif; line-height: 1.5; color: black;">
            ${editorEl.innerHTML}
          </div>
        </div>
      `;

		const pdf = new jsPDF("p", "mm", "a4");
		const pageWidth = 210;
		const contentWidth = 170;

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
							baseDir: BaseDirectory.Document,
						});

						await file.write(pdfBuffer);
						await file.close();

						resolve({
							show: true,
							message: `Note exported as PDF to Documents folder: ${uniqueFileName}`,
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
				margin: [15, 15, 15, 15],
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
