// Function to get title from content (first heading or first line)
export const extractTitle = (content) => {
	// Try to find a heading tag
	const headingMatch = content.match(/<heading[^>]*>(.*?)<\/heading>/);
	if (headingMatch && headingMatch[1]) {
		return headingMatch[1].replace(/<[^>]+>/g, "").trim();
	}

	// Otherwise, get the first paragraph or line
	const firstParagraphMatch = content.match(
		/<paragraph[^>]*>(.*?)<\/paragraph>/,
	);
	if (firstParagraphMatch && firstParagraphMatch[1]) {
		const text = firstParagraphMatch[1].replace(/<[^>]+>/g, "").trim();
		// Return first 30 chars if there's text
		return text
			? text.length > 30
				? text.substring(0, 30) + "..."
				: text
			: "Untitled Note";
	}

	return "Untitled Note";
};
