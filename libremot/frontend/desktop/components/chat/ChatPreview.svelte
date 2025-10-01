<script lang="ts">
	
		interface Props {
		previewHTML: string;
		maxHeight?: string;
		minHeight?: string;
	}

	let {
		previewHTML = "",
		maxHeight = "180px",
		minHeight = "180px",
	}: Props = $props();
	// For chat preview, we'll show a simple text preview
	// In a real implementation, you might want to parse HTML and extract text
	const getTextPreview = (html: string) => {
		if (!html) return "No messages yet";
		// Simple HTML tag removal for preview
		return html.replace(/<[^>]*>/g, '').substring(0, 100) + (html.length > 100 ? '...' : '');
	};
</script>

<div 
	class="text-osvauld-fieldText opacity-80 text-sm"
	style="max-height: {maxHeight}; min-height: {minHeight};"
>
	{getTextPreview(previewHTML)}
</div>
