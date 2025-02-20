<script>
	import RichTextEditor from "./lib/RichTextEditor.svelte";

	function handleChange(event) {
		const { getContent } = event.detail;
		console.log("Content updated:", getContent());
	}
</script>

<style>
	:root {
		box-sizing: border-box;
		margin: 0;
		padding: 0;
	}
</style>

<RichTextEditor placeholder="Start writing..." on:change={handleChange} />
