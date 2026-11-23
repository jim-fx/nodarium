import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { playwright } from '@vitest/browser-playwright';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [sveltekit({ preprocess: vitePreprocess(), adapter: adapter() })],
  ssr: {
    noExternal: ['three']
  },
  // found late, a test that is the first to import them is reloaded and fails
  optimizeDeps: {
    include: ['file-saver', 'three/src/math/Vector3.js']
  },
  test: {
    expect: { requireAssertions: true },
    // CI runs the tests of every package at once, a worker per core here got the job killed for memory
    maxWorkers: 2,
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
          include: ['src/**/*.svelte.{test,spec}.{js,ts}']
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
