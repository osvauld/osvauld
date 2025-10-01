<script lang="ts">
	interface Props {
		color?: string;
		size?: number;
		className?: string;
		bgColor?: string;
		width?: number;
		height?: number;
	}

	let {
		color = "#2F303E",
		size = 20,
		className = "",
		bgColor = "black",
		width = 44,
		height = 59,
	}: Props = $props();
</script>

<svg
	{width}
	{height}
	viewBox="0 0 44 59"
	fill="none"
	xmlns="http://www.w3.org/2000/svg"
	class={className}
>
	<rect x="0.5" y="0.5" width="43" height="58" fill={bgColor}></rect>
	<rect x="0.5" y="0.5" width="43" height="58" stroke={bgColor}></rect>
	<path
		class="transition-colors duration-300"
		d="M32.4111 7.2207L18.5068 29.7373L18.3447 30L18.5068 30.2627L32.4111 52.7783L30.3926 56.0479L16.3203 33.2617C15.7839 32.3932 15.5001 31.2156 15.5 30.0078C15.5 28.8009 15.7833 27.6178 16.3203 26.7383L30.3926 3.95117L32.4111 7.2207Z"
		fill={color}
		stroke={bgColor}
	></path>
</svg>
