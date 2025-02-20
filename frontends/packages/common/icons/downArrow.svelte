<script>
	export let type;
	function setbackground(type) {
		const typeToClassMap = {
			reader: "#F9E2AF",
			manager: "#F5C2E7",
			owner: "#A6E3A1",
			common: "#6E7681",
			indicator: "#74C7EC",
			profile: "#85889C",
			profileActive: "#F2F2F0",
		};
		return typeToClassMap[type] || "";
	}
</script>

<svg
	xmlns="http://www.w3.org/2000/svg"
	width="20"
	height="20"
	viewBox="0 0 24 24"
	fill="none"
>
	<path
		d="M11.9997 16.0402C11.6797 16.0402 11.3597 15.9202 11.1147 15.6752L5.21973 9.78021L6.27973 8.72021L11.9997 14.4402L17.7197 8.72021L18.7797 9.78021L12.8847 15.6752C12.6397 15.9202 12.3197 16.0402 11.9997 16.0402Z"
		fill="{setbackground(type)}"
	></path>
</svg>
