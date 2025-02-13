import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import path from "path";
import { sveltePreprocess } from "svelte-preprocess";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig(({ mode }) => {
	const isDev = mode === "development";

	return {
		appType: "spa",
		clearScreen: false,
		root: ".", // Root is current directory since we're in desktop package

		plugins: [
			svelte({
				preprocess: sveltePreprocess({
					typescript: true,
				}),
				compilerOptions: {
					dev: isDev,
					// Use this to enable HMR in Svelte 5
					hmr: isDev,
				},
			}),
			tailwindcss(), // Use the Tailwind CSS Vite plugin
		],

		resolve: {
			alias: {
				"@osvauld/password-manager-common": path.resolve(__dirname, "../common"),
			},
		},

		build: {
			target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
			minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
			sourcemap: !!process.env.TAURI_ENV_DEBUG,
			outDir: "dist",
			emptyOutDir: true,
		},

		server: {
			strictPort: true,
			port: 1421,
			host: process.env.TAURI_DEV_HOST || "0.0.0.0",
			hmr: {
				protocol: "ws",
				host: process.env.TAURI_DEV_HOST || "0.0.0.0",
				port: 1421,
			},
			watch: {
				usePolling: true,
				interval: 1000,
			},
		},
	};
});