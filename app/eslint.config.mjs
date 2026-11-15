import { includeIgnoreFile } from '@eslint/compat';
import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import { defineConfig } from 'eslint/config';
import globals from 'globals';
import path from 'node:path';
import ts from 'typescript-eslint';

const gitignorePath = path.resolve(import.meta.dirname, '.gitignore');

// The node editor is meant to become a package: it imports nothing from the app, and the app
// reaches it through its index.ts only. Tests are held to this like any other file.
const graphInterface = 'src/lib/graph-interface';

/**
 * @param {string} regex
 * @param {string} message
 * @returns {import('eslint').Linter.RulesRecord}
 */
const restrictImports = (regex, message) => ({
  'no-restricted-imports': ['error', { patterns: [{ regex, message }] }]
});

const graphInterfaceBoundary = [
  {
    files: ['src/**'],
    ignores: [`${graphInterface}/**`],
    rules: restrictImports(
      'graph-interface/(?!index\\.ts$)',
      'Import from #lib/graph-interface/index.ts, it exports what the app may use.'
    )
  },
  // a relative import leaves the folder with one `../` more than the file is deep
  ...[0, 1, 2, 3, 4].map((depth) => ({
    files: [`${graphInterface}/${'*/'.repeat(depth)}*`],
    rules: restrictImports(
      `^[#$]lib/|^${'\\.\\./'.repeat(depth + 1)}`,
      'graph-interface must not import from the app, pass it in as an option or a prop.'
    )
  }))
];

export default defineConfig(
  includeIgnoreFile(gitignorePath),
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs.recommended,
  {
    languageOptions: { globals: { ...globals.browser, ...globals.node } },
    rules: {
      // typescript-eslint strongly recommend that you do not use the no-undef lint rule on TypeScript projects.
      // see: https://typescript-eslint.io/troubleshooting/faqs/eslint/#i-get-errors-from-the-no-undef-rule-about-global-variables-not-being-defined-even-though-there-are-no-typescript-errors
      'no-undef': 'off'
    }
  },
  {
    files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
    languageOptions: {
      parserOptions: {
        projectService: true,
        extraFileExtensions: ['.svelte'],
        parser: ts.parser,
        tsconfigRootDir: import.meta.dirname
      }
    }
  },
  ...graphInterfaceBoundary
);
