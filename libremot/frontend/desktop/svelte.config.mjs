import { sveltePreprocess } from "svelte-preprocess";

const config = {
  preprocess: sveltePreprocess({
    typescript: {
      tsconfigFile: './tsconfig.json',
      reportDiagnostics: true
    },
    scss: {
      prependData: '@use "sass:math";'
    },
    postcss: true // Enable PostCSS processing for vanilla CSS in <style> blocks
  }),
  compilerOptions: {
    runes: true
  }
};

export default config;
