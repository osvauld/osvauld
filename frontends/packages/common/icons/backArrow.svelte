<script lang="ts">
	interface Props {
		color?: string;
		size?: number;
	}

	let { color = "#85889C", size = 24 }: Props = $props();
</script>

<svg
	width="{size}"
	height="{size}"
	viewBox="0 0 24 24"
	fill="none"
	xmlns="http://www.w3.org/2000/svg">
	<path
		d="M20.9997 11.2502H5.30969L10.5297 6.03022L9.46969 4.97021L3.32469 11.1152C2.83969 11.6002 2.83969 12.3952 3.32469 12.8852L9.46969 19.0302L10.5297 17.9702L5.30969 12.7502H20.9997V11.2502Z"
		fill="{color}"></path>
</svg>
