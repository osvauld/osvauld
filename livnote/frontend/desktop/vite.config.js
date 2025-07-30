import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { sveltePreprocess } from "svelte-preprocess";
import tailwindcss from "@tailwindcss/vite";
import legacy from "@vitejs/plugin-legacy";

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
      legacy({
        targets: ['chrome >= 58', 'firefox >= 54', 'safari >= 11', 'edge >= 79'],
        modernPolyfills: true,
        renderLegacyChunks: false,
      }),
    ],


    build: {
      target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome58" : "safari11",
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
