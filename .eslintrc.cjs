module.exports = {
  env: {
    browser: true,
    es2021: true,
  },
  extends: ["eslint:recommended", "plugin:svelte/recommended"],
  parser: "@typescript-eslint/parser",
  plugins: ["@typescript-eslint"],
  parserOptions: {
    ecmaVersion: "latest",
    sourceType: "module",
    extraFileExtensions: [".svelte"],
  },
  overrides: [
    {
      files: ["*.svelte"],
      parser: "svelte-eslint-parser",
      parserOptions: {
        parser: "@typescript-eslint/parser",
      },
      rules: {
        "indent": ["error", "tab", { "SwitchCase": 1 }],
        "no-tabs": "off"
      }
    },
    {
      files: ["**/*.ts", "**/*.js"],
      rules: {
        "indent": ["error", "tab", { "SwitchCase": 1 }],
        "no-tabs": "off"
      }
    },
  ],
  rules: {
    "indent": ["error", "tab", { "SwitchCase": 1 }],
    "no-tabs": "off"
  },
};
