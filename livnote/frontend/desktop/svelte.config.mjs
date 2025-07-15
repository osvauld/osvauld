import { sveltePreprocess } from "svelte-preprocess";

const config = {
  preprocess: sveltePreprocess({
    typescript: {
      tsconfigFile: './tsconfig.json',
      reportDiagnostics: true
    },
    scss: {
      prependData: '@use "sass:math";'
    }
  }),
  compilerOptions: {
    runes: true
  }
};

export default config;
