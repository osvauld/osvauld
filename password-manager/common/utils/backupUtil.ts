// Add these imports at the top of your component
import { jsPDF } from "jspdf";
import { open, BaseDirectory } from "@tauri-apps/plugin-fs";

// Improved function to generate the PDF

const extractUsername = (certificateData) => {
	// Split the certificate into lines
	const lines = certificateData.split("\n");

	// Find all lines that start with "Comment:"
	const commentLines = lines.filter((line) =>
		line.trim().startsWith("Comment:"),
	);

	// Return the second comment if it exists
	if (commentLines.length >= 2) {
		// Extract just the comment value (everything after "Comment:")
		const secondComment = commentLines[1]
			.trim()
			.substring("Comment:".length)
			.trim();
		return secondComment;
	}

	// Return null if there's no second comment
	return null;
};

export async function generateCertificatePDF(certificateData) {
	// Initialize PDF
	const pdf = new jsPDF({
		orientation: "portrait",
		unit: "mm",
		format: "a4",
	});

	const username = extractUsername(certificateData);

	// Set PDF properties
	pdf.setProperties({
		title: "LivNote Recovery Certificate",
		subject: "Recovery certificate for LivNote",
		author: "Osvauld",
		keywords: "recovery, certificate",
		creator: "LivNote by Osvauld",
	});

	// Get page dimensions
	const pageWidth = pdf.internal.pageSize.getWidth();
	const pageHeight = pdf.internal.pageSize.getHeight();

	// Define colors - slightly adjusted for better contrast
	const primaryColor = "#e91e63"; // Pink/red color
	const lightBgColor = "#fce8ec"; // Lighter pink background for better contrast
	const textColor = "#333333";
	const borderColor = "#ffb0c5"; // Light border color for boxes

	// Define margins
	const margin = 20;
	const contentWidth = pageWidth - margin * 2;

	// Current date for the certificate
	const currentDate = new Date().toLocaleDateString("en-US", {
		year: "numeric",
		month: "long",
		day: "numeric",
	});

	// Header with logo and title - make it slightly larger and more rounded
	pdf.setFillColor(primaryColor);
	pdf.roundedRect(margin - 5, margin - 5, contentWidth + 10, 25, 5, 5, "F");

	// Title text - increased font size
	pdf.setTextColor(255, 255, 255); // White text
	pdf.setFontSize(22);
	pdf.setFont("helvetica", "bold");
	pdf.text("LivNote Emergency Kit", pageWidth / 2, margin + 10, {
		align: "center",
	});

	// Creation date - improved spacing
	pdf.setTextColor(textColor);
	pdf.setFontSize(11);
	pdf.setFont("helvetica", "normal");
	pdf.text(`Created on ${currentDate}`, pageWidth / 2, margin + 32, {
		align: "center",
	});

	// Main content section with light pink background
	// We assume certificateData is the complete certificate text including BEGIN/END blocks and comments
	const fields = [
		{ label: "USER ID", value: username },
		{ label: "RECOVERY CERTIFICATE", value: certificateData },
	];

	let yPos = margin + 40;
	const fieldHeight = 210; // Increased height for the fields section

	// Main content background with border
	pdf.setFillColor(lightBgColor);
	pdf.roundedRect(margin - 5, yPos, contentWidth + 10, fieldHeight, 5, 5, "F");
	pdf.setDrawColor(borderColor);
	pdf.setLineWidth(0.5);
	pdf.roundedRect(margin - 5, yPos, contentWidth + 10, fieldHeight, 5, 5, "S");

	// Content title
	pdf.setFontSize(16);
	pdf.setTextColor(primaryColor);
	pdf.setFont("helvetica", "bold");
	pdf.text("LivNote Account Details", pageWidth / 2, yPos + 15, {
		align: "center",
	});

	yPos += 30;

	fields.forEach((field) => {
		// Label with better spacing
		pdf.setFontSize(11);
		pdf.setTextColor(textColor);
		pdf.setFont("helvetica", "bold");
		pdf.text(field.label, margin + 3, yPos);

		// Value - improved handling for certificate
		pdf.setFontSize(10);
		pdf.setFont("courier", "normal");
		pdf.setDrawColor(200, 200, 200);
		pdf.setFillColor(255, 255, 255);

		if (field.label === "RECOVERY CERTIFICATE") {
			const certWidth = contentWidth - 10;
			const certHeight = 140; // Increased height for the certificate box

			// Add certificate box with slight shadow effect
			pdf.setDrawColor(220, 220, 220);
			pdf.roundedRect(margin + 1, yPos + 3, certWidth, certHeight, 3, 3, "FD");

			// Don't format the certificate text - just place it as is
			pdf.setFont("courier", "normal");
			pdf.setTextColor(textColor);
			pdf.setFontSize(7); // Small font size to fit the entire certificate

			// Use the certificate data exactly as provided without any formatting changes
			const lines = certificateData.split("\n");
			const lineHeight = 3.5; // Reduced line height to fit more text

			// Render each line exactly as it is in the input
			lines.forEach((line, index) => {
				pdf.text(line, margin + 5, yPos + 10 + index * lineHeight);
			});

			yPos += certHeight + 10;
		} else {
			// User ID box with better styling
			pdf.roundedRect(margin + 1, yPos + 3, contentWidth - 10, 12, 3, 3, "FD");
			pdf.setTextColor(textColor);
			pdf.text(field.value, margin + 5, yPos + 11);
			yPos += 20;
		}
	});

	// Help section after the light red background
	yPos += 15; // Increased spacing after the fields

	// Footer with border
	pdf.setFillColor(245, 245, 245);
	pdf.rect(0, pageHeight - 20, pageWidth, 20, "F");

	pdf.setFontSize(9);
	pdf.setTextColor(textColor);
	pdf.setFont("helvetica", "normal");
	pdf.text("LivNote by Osvauld - Page 1 of 1", pageWidth / 2, pageHeight - 10, {
		align: "center",
	});

	// Get the date string for the filename (YYYYMMDD format)
	const dateStr = new Date().toISOString().slice(0, 10).replace(/-/g, "");
	const fileName = `livnote_recovery_${dateStr}.pdf`;

	// Get PDF as array buffer
	const pdfData = pdf.output("arraybuffer");

	// Convert to Uint8Array for file writing
	const pdfBuffer = new Uint8Array(pdfData);

	// Save the PDF file to Documents directory
	const file = await open(fileName, {
		write: true,
		create: true,
		truncate: true,
		baseDir: BaseDirectory.Document,
	});

	// Write PDF data
	await file.write(pdfBuffer);

	// Close the file
	await file.close();

	return true;
}
