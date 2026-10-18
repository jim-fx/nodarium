/// <reference types="vite-plugin-comlink/client" />

export function createGardenWorker() {
  return new ComlinkWorker<typeof import('./garden-worker-backend.ts')>(
    new URL(`./garden-worker-backend.ts`, import.meta.url)
  );
}
