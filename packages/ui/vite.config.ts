import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { playwright } from '@vitest/browser-playwright';
import { exec } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { defineConfig } from 'vitest/config';

const postDevPackagePlugin = () => {
  const packageContent = JSON.parse(readFileSync('./package.json', { encoding: 'utf8' }));
  let timeout: ReturnType<typeof setTimeout> | undefined;
  return {
    name: 'run-command-on-change',
    handleHotUpdate: () => {
      clearTimeout(timeout);
      timeout = setTimeout(function() {
        exec(packageContent.scripts.package);
      }, 200);
    }
  } as const;
};

export default defineConfig({
  plugins: [tailwindcss(), sveltekit(), postDevPackagePlugin()],
  test: {
    expect: { requireAssertions: true },
    projects: [
      {
        extends: './vite.config.ts',
        test: {
          name: 'client',
          browser: {
            enabled: true,
            provider: playwright(),
            instances: [{ browser: 'firefox', headless: true }]
          },
          include: ['src/**/*.svelte.ts'],
          exclude: ['src/lib/server/**']
        }
      },

      {
        extends: './vite.config.ts',
        test: {
          name: 'server',
          environment: 'node',
          include: ['src/**/*.{test,spec}.{js,ts}'],
          exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
        }
      }
    ]
  }
});
