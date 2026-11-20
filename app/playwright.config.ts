import { defineConfig } from '@playwright/test';

export default defineConfig({
  // CI builds the app in an earlier step
  webServer: {
    command: process.env.CI ? 'pnpm preview' : 'pnpm build && pnpm preview',
    port: 4173
  },
  testDir: 'e2e',
  // each worker is a Firefox with software WebGL; more of them starve the CI runner
  workers: 2,
  use: {
    browserName: 'firefox',
    launchOptions: {
      firefoxUserPrefs: {
        // Force WebGL even without a GPU
        'webgl.force-enabled': true,
        'webgl.disabled': false,
        // Use software rendering (Mesa) instead of hardware
        'layers.acceleration.disabled': true,
        'gfx.webrender.software': true,
        'webgl.enable-webgl2': true
      }
    }
  }
});
