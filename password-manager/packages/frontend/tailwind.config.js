export default {
	content: [
		"./common/**/*.{html,js,svelte,ts}",
		"./desktop/**/*.{html,js,svelte,ts}",
		"./mobile/**/*.{html,js,svelte,ts}",
		"./tailwind.css",
	],
	theme: {
		screens: {
			sm: "640px",
			md: "768px",
			lg: "1024px",
			xl: "1280px",
			"2xl": "1536px",
			"3xl": "1920px",
		},
		extend: {
			fontFamily: {
				sans: [
					'"Inter"',
					"system-ui",
					"-apple-system",
					"BlinkMacSystemFont",
					'"Segoe UI"',
					"Roboto",
					'"Helvetica Neue"',
					"Arial",
					'"Noto Sans"',
					"sans-serif",
					'"Apple Color Emoji"',
					'"Segoe UI Emoji"',
					'"Segoe UI Symbol"',
					'"Noto Color Emoji"',
				],
				Jakarta: ['"Plus Jakarta Sans"', "sans-serif"],
			},
			fontSize: {
				base: "16px",
				lg: "18px",
				xl: "20px",
				"2xl": "24px",
			},
		},
	},
};
