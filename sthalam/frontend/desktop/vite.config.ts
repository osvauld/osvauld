import { vanillaExtractPlugin } from '@vanilla-extract/vite-plugin';
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import wasm from 'vite-plugin-wasm';
import checker from 'vite-plugin-checker';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [
    tailwindcss(),
    svelte(),
    wasm(),
    vanillaExtractPlugin({
      identifiers: 'short'
    }),
    checker({
      typescript: true,
      svelte: true,
      overlay: {
        initialIsOpen: false,
      },
    })
  ],
  build: {
    commonjsOptions: {
      include: [/lodash\.ismatch/, /node_modules/],
      transformMixedEsModules: true
    },
    target: 'esnext',
    minify: false,
    sourcemap: true
  },
  optimizeDeps: {
    include: [
      'yjs'
    ],
    esbuildOptions: {
      target: 'esnext'
    }
  },
  server: {
    port: 1422,
    open: true,
    fs: {
      strict: false
    }
  },
  resolve: {
    extensions: ['.ts', '.js', '.svelte'],
    dedupe: ['yjs']
  },
  esbuild: {
    target: 'esnext',
    supported: {
      'top-level-await': true
    }
  }
});
