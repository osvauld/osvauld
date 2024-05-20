module.exports = {
  env: {
    browser: true,
    es2021: true,
  },
  extends: [
    "eslint:recommended",
    "plugin:svelte/recommended",
    "plugin:@typescript-eslint/recommended",
    "plugin:prettier/recommended", // Add this line
  ],
  parser: "@typescript-eslint/parser",
  plugins: ["@typescript-eslint", "svelte3", "prettier"], // Add "prettier"
  parserOptions: {
    project: "./tsconfig.app.json",
    ecmaVersion: "latest",
    sourceType: "module",
    extraFileExtensions: [".svelte"],
  },
  overrides: [
    {
      files: ["*.svelte"],
      processor: "svelte3/svelte3",
      parser: "svelte-eslint-parser",
      parserOptions: {
        parser: "@typescript-eslint/parser",
        project: "./tsconfig.app.json",
      },
    },
    {
      files: ["src/**/*.ts"],
      parserOptions: {
        project: "./tsconfig.app.json",
      },
    },
  ],
  settings: {
    "svelte3/typescript": require("typescript"), // Add this line
  },
  rules: {
    "prettier/prettier": ["error"], // Add this line to enforce Prettier formatting
  },
};