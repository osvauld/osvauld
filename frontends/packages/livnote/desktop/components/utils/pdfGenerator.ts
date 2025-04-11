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

		const editorEl = document.querySelector(".ProseMirror");
		if (!editorEl) {
			throw new Error("Editor content not found");
		}

		const container = document.createElement("div");
		container.innerHTML = `
        <div style="width: 100%;">
          <div style="font-family: 'Inter', 'Segoe UI', sans-serif; line-height: 1.5; color: black; ">
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
						const filePath = `${safeTitle}.pdf`;

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
							message: `Note exported as PDF to Documents folder: ${safeTitle}.pdf`,
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
