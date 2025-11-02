import { vanillaExtractPlugin } from '@vanilla-extract/vite-plugin';
import { defineConfig, Plugin } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import wasm from 'vite-plugin-wasm';
import checker from 'vite-plugin-checker';
import tailwindcss from '@tailwindcss/vite';
import fs from 'fs';
import path from 'path';

// Custom plugin to serve .wasm files with correct MIME type
function wasmMimePlugin(): Plugin {
  return {
    name: 'wasm-mime',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (req.url && req.url.endsWith('.wasm')) {
          console.log('[WASM Middleware] Request for:', req.url);

          // Get the file path - Vite serves from public directory
          const filePath = req.url.startsWith('/') ? req.url.substring(1) : req.url;
          const wasmPath = path.resolve(process.cwd(), 'public', filePath);

          console.log('[WASM Middleware] Looking for file at:', wasmPath);

          if (fs.existsSync(wasmPath)) {
            console.log('[WASM Middleware] File found! Serving with correct MIME type');
            const wasmFile = fs.readFileSync(wasmPath);
            res.setHeader('Content-Type', 'application/wasm');
            res.end(wasmFile);
            return;
          } else {
            console.log('[WASM Middleware] File not found at:', wasmPath);
          }
        }
        next();
      });
    }
  };
}

export default defineConfig({
  plugins: [
    tailwindcss(),
    svelte(),
    wasm(),
    wasmMimePlugin(),
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
  publicDir: 'public',
  build: {
    commonjsOptions: {
      include: [/lodash\.ismatch/, /node_modules/],
      transformMixedEsModules: true
    },
    target: 'esnext',
    minify: 'esbuild',
    sourcemap: true,
    assetsInlineLimit: 0, // Don't inline WASM files
  },
  optimizeDeps: {
    include: [
      'yjs'
    ],
    exclude: [
      'src/lib/huml-eval.js'
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
