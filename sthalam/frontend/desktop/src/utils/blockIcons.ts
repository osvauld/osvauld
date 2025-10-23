/**
 * Block Icons and Visual Utilities
 * Maps block types to icons, colors, and preview text
 */

export const BLOCK_ICONS: Record<string, string> = {
	"screen-container": "🖥️",
	"section-container": "📦",
	heading: "H",
	text: "T",
	"markdown-text": "MD",
	"nav-button": "→",
	image: "🖼️",
	"form-field": "📝",
	"form-field-checkbox": "☑",
	"form-submit": "✓",
	thread: "💬",
	"branching-question": "❓"
};

export const BLOCK_COLORS: Record<string, string> = {
	"screen-container": "#89b4fa",
	"section-container": "#a6e3a1",
	heading: "#cba6f7",
	text: "#c9d1d9",
	"markdown-text": "#74c7ec",
	"nav-button": "#f9e2af",
	image: "#f5c2e7",
	"form-field": "#cba6f7",
	"form-field-checkbox": "#cba6f7",
	"form-submit": "#a6e3a1",
	thread: "#74c7ec",
	"branching-question": "#f9e2af"
};

export const BLOCK_TYPE_LABELS: Record<string, string> = {
	"screen-container": "Screen",
	"section-container": "Container",
	heading: "Heading",
	text: "Text",
	"markdown-text": "Markdown",
	"nav-button": "Navigation",
	image: "Image",
	"form-field": "Input",
	"form-field-checkbox": "Checkbox",
	"form-submit": "Submit",
	thread: "Thread",
	"branching-question": "Branch"
};

/**
 * Get icon for block type
 */
export function getBlockIcon(type: string): string {
	return BLOCK_ICONS[type] || "?";
}

/**
 * Get color for block type
 */
export function getBlockColor(type: string): string {
	return BLOCK_COLORS[type] || "#6e7681";
}

/**
 * Get human-readable label for block type
 */
export function getBlockTypeLabel(type: string): string {
	return BLOCK_TYPE_LABELS[type] || type;
}

/**
 * Get content preview for display in tree
 */
export function getContentPreview(block: any, maxLength: number = 60): string {
	const type = block.type;

	// For screen containers, show name
	if (type === "screen-container") {
		const name = block.name || "Screen";
		const isEntry = block.isEntryPoint ? " (Entry)" : "";
		return name + isEntry;
	}

	// For section containers, show name
	if (type === "section-container") {
		return block.name || "Container";
	}

	// For nav buttons, show "→ Target"
	if (type === "nav-button") {
		const target = block.targetScreen || "Unknown";
		const content = block.content || "";
		return content ? `${content}` : `→ ${target}`;
	}

	// For images, show src or alt
	if (type === "image") {
		return block.alt || block.src || "Image";
	}

	// For form fields, show label or type
	if (type.startsWith("form-field")) {
		return block.label || getBlockTypeLabel(type);
	}

	// For thread, show title
	if (type === "thread") {
		return block.title || "Thread";
	}

	// For text/heading/markdown, show truncated content
	if (block.content) {
		const content = block.content.trim();
		if (content.length > maxLength) {
			return content.substring(0, maxLength) + "...";
		}
		return content || "(empty)";
	}

	// Fallback to type label
	return getBlockTypeLabel(type);
}

/**
 * Check if block can have children
 */
export function canHaveChildren(type: string): boolean {
	return type === "screen-container" || type === "section-container";
}

/**
 * Get child count for display
 */
export function getChildCount(block: any): number {
	if (!block.children || !Array.isArray(block.children)) {
		return 0;
	}
	return block.children.length;
}
