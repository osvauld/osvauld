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
    root: ".",

    plugins: [
      tailwindcss(),
      svelte({
        preprocess: sveltePreprocess({
          typescript: true,
        }),
        compilerOptions: {
          dev: isDev,
          hmr: isDev,
          runes: true,
        },
      }),
    ],


    build: {
      target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
      minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
      sourcemap: true,
      outDir: "dist",
      emptyOutDir: true,
    },
    css: {
      devSourcemap: true,
    },

    server: {
      strictPort: true,
      port: 1421,
      host: process.env.TAURI_DEV_HOST || "localhost",
      hmr: {
        protocol: "ws",
        host: process.env.TAURI_DEV_HOST || "localhost",
        port: 1421,
      },
      watch: {
        usePolling: true,
        interval: 1000,
      },
    },
  };
});
