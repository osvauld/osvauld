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
		root: ".",  // Root is current directory since we're in mobile package

		plugins: [
			tailwindcss(),
			svelte({
				preprocess: sveltePreprocess({
					typescript: true,
				}),
				compilerOptions: {
					dev: isDev,
					hmr: isDev,
				},
			}),
		],

		resolve: {
			alias: {
				"@osvauld/password-manager-common": path.resolve(__dirname, "../common"),
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
			host: process.env.TAURI_DEV_HOST || "localhost",
			hmr: {
				protocol: 'ws',
				host: process.env.TAURI_DEV_HOST || 'localhost',
				port: 1420,
			},
			watch: {
				usePolling: true,
				interval: 1000,
			}
		},
	};
});