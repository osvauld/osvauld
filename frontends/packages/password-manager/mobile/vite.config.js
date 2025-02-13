import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import path from "path";
import autoprefixer from "autoprefixer";
import tailwindcss from "tailwindcss";
import { sveltePreprocess } from "svelte-preprocess";

export default defineConfig(({ mode }) => {
	const isDev = mode === "development";

	return {
		appType: "spa",
		clearScreen: false,
		root: ".",  // Root is current directory since we're in mobile package

		plugins: [
			svelte({
				preprocess: sveltePreprocess({
					typescript: true,
					postcss: {
						plugins: [tailwindcss(), autoprefixer()],
					},
				}),
				compilerOptions: {
					dev: isDev,
				},
				hot: isDev && {
					preserveState: true,
				},
			}),
		],

		resolve: {
			alias: {
				"@osvauld/password-manager-common": path.resolve(__dirname, "../common"),
			},
		},

		css: {
			postcss: {
				plugins: [tailwindcss(), autoprefixer()],
			},
		},

		build: {
			target: "safari13",
			minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
			sourcemap: !!process.env.TAURI_ENV_DEBUG,
			outDir: "dist",
			emptyOutDir: true,
		},

		server: {
			strictPort: true,
			port: 1420,
			host: process.env.TAURI_DEV_HOST || "0.0.0.0",
			hmr: {
				protocol: 'ws',
				host: process.env.TAURI_DEV_HOST || '0.0.0.0',
				port: 1420,
			},
			watch: {
				usePolling: true,
				interval: 1000,
			}
		},
	};
});