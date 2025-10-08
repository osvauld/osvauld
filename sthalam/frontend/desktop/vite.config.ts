import { vanillaExtractPlugin } from '@vanilla-extract/vite-plugin';
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import wasm from 'vite-plugin-wasm';
import checker from 'vite-plugin-checker';

export default defineConfig({
  plugins: [
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
      'yjs',
      'lit',
      '@lit/reactive-element',
      '@preact/signals-core'
    ],
    // Exclude ALL BlockSuite packages from pre-bundling
    exclude: [
      '@blocksuite/affine',
      '@blocksuite/integration-test'
    ],
    esbuildOptions: {
      target: 'esnext',
      supported: {
        'decorators': true
      }
    }
  },
  ssr: {
    noExternal: [
      '@blocksuite/affine',
      '@blocksuite/affine-block-note',
      '@blocksuite/affine-fragment-outline',
      '@blocksuite/integration-test'
    ]
  },
  server: {
    port: 5173,
    open: true,
    fs: {
      strict: false
    }
  },
  resolve: {
    extensions: ['.ts', '.js', '.svelte'],
    dedupe: ['lit', 'yjs', '@preact/signals-core']
  },
  esbuild: {
    target: 'esnext',
    supported: {
      'top-level-await': true,
      'decorators': true
    }
  }
});
