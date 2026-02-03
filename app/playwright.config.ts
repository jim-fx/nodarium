import { defineConfig } from '@playwright/test';

export default defineConfig({
  webServer: { command: 'pnpm build && pnpm preview', port: 4173 },
  testDir: 'e2e',
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
