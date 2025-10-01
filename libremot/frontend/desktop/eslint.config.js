import baseConfig from '@osvauld/eslint-config';

export default [
  ...baseConfig,
  {
    files: ['**/*.{ts,js}'],
    languageOptions: {
      parserOptions: {
        project: './tsconfig.json',
      },
    },
  },
];
